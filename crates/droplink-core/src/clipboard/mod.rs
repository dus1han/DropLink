//! Universal clipboard.
//!
//! Loop prevention: every piece of content is identified by a SHA-256 hash.
//! When remote content is applied, its hash (and the platform's clipboard
//! change counter, read *after* writing) becomes the "last seen" state, so
//! the watcher does not mistake our own write for a new local copy. Remote
//! content is therefore never re-broadcast, and A → B → A cycles cannot
//! form. Only devices that are trusted *and* have clipboard sync permission
//! can send or receive clipboard data.

mod system;

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use sha2::{Digest, Sha256};

use crate::db::NewClip;
use crate::engine::Engine;
use crate::events::Event;
use crate::models::{ClipboardEntryView, TrustState};
use crate::net::Conn;
use crate::protocol::{read_frame, write_data, ClipboardPayload, Frame, Message, Purpose};
use crate::text::SendResult;
use crate::{Error, Result};

pub use system::system_backend;

pub const MAX_CLIP_TEXT: usize = 1_000_000;
pub const MAX_IMAGE_PIXELS: u64 = 40_000_000;
const HISTORY_TEXT_CHARS: usize = 20_000;
const POLL_INTERVAL: Duration = Duration::from_millis(700);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClipContent {
    Text(String),
    Image { width: u32, height: u32, rgba: Vec<u8> },
}

impl ClipContent {
    pub fn hash(&self) -> String {
        let mut h = Sha256::new();
        match self {
            ClipContent::Text(t) => {
                h.update(b"text\0");
                h.update(t.as_bytes());
            }
            ClipContent::Image { width, height, rgba } => {
                h.update(b"image\0");
                h.update(width.to_be_bytes());
                h.update(height.to_be_bytes());
                h.update(rgba);
            }
        }
        hex::encode(h.finalize())
    }

    pub fn kind(&self) -> &'static str {
        match self {
            ClipContent::Text(_) => "text",
            ClipContent::Image { .. } => "image",
        }
    }

    pub fn byte_size(&self) -> u64 {
        match self {
            ClipContent::Text(t) => t.len() as u64,
            ClipContent::Image { rgba, .. } => rgba.len() as u64,
        }
    }

    fn within_limits(&self) -> bool {
        match self {
            ClipContent::Text(t) => !t.is_empty() && t.len() <= MAX_CLIP_TEXT,
            ClipContent::Image { width, height, rgba } => {
                let px = *width as u64 * *height as u64;
                px > 0 && px <= MAX_IMAGE_PIXELS && rgba.len() as u64 == px * 4
            }
        }
    }
}

pub trait ClipboardBackend: Send + Sync + 'static {
    /// A cheap value that changes whenever the clipboard changes, where the
    /// platform provides one (Windows sequence number).
    fn change_counter(&self) -> Option<u64> {
        None
    }
    fn read(&self, images: bool) -> Option<ClipContent>;
    fn write(&self, content: &ClipContent) -> std::result::Result<(), String>;
    /// True if the current content was marked as confidential by its source
    /// (password managers set dedicated clipboard formats for this).
    fn is_sensitive(&self) -> bool {
        false
    }
}

/// In-process clipboard used by tests.
#[derive(Default)]
pub struct MemoryClipboard {
    inner: Mutex<(Option<ClipContent>, u64, bool)>,
}

impl MemoryClipboard {
    /// Simulates the user copying something.
    pub fn set(&self, c: ClipContent) {
        let mut g = self.inner.lock();
        g.0 = Some(c);
        g.1 += 1;
        g.2 = false;
    }
    /// Simulates a password manager copying a secret.
    pub fn set_sensitive(&self, c: ClipContent) {
        self.set(c);
        self.inner.lock().2 = true;
    }
    pub fn get(&self) -> Option<ClipContent> {
        self.inner.lock().0.clone()
    }
    pub fn writes(&self) -> u64 {
        self.inner.lock().1
    }
}

impl ClipboardBackend for MemoryClipboard {
    fn change_counter(&self) -> Option<u64> {
        Some(self.inner.lock().1)
    }
    fn read(&self, images: bool) -> Option<ClipContent> {
        match self.inner.lock().0.clone() {
            Some(ClipContent::Image { .. }) if !images => None,
            c => c,
        }
    }
    fn write(&self, content: &ClipContent) -> std::result::Result<(), String> {
        let mut g = self.inner.lock();
        g.0 = Some(content.clone());
        g.1 += 1;
        g.2 = false;
        Ok(())
    }
    fn is_sensitive(&self) -> bool {
        self.inner.lock().2
    }
}

pub struct ClipboardState {
    pub(crate) backend: Option<Arc<dyn ClipboardBackend>>,
    last_hash: Mutex<Option<String>>,
    last_counter: Mutex<Option<u64>>,
}

impl ClipboardState {
    pub fn new(backend: Option<Arc<dyn ClipboardBackend>>) -> Self {
        Self { backend, last_hash: Mutex::new(None), last_counter: Mutex::new(None) }
    }

    /// Returns new local content, or `None` if nothing changed since the
    /// last check (including changes we caused ourselves).
    fn poll(&self, images: bool) -> Option<(ClipContent, String, bool)> {
        let backend = self.backend.as_ref()?;
        let counter = backend.change_counter();
        if counter.is_some() {
            let mut last = self.last_counter.lock();
            if *last == counter {
                return None;
            }
            *last = counter;
        }
        let content = backend.read(images)?;
        let hash = content.hash();
        {
            let mut last = self.last_hash.lock();
            if last.as_deref() == Some(hash.as_str()) {
                return None;
            }
            *last = Some(hash.clone());
        }
        Some((content, hash, backend.is_sensitive()))
    }

    /// Writes remote content and records the resulting state as already seen.
    fn apply_remote(&self, content: &ClipContent) -> std::result::Result<(), String> {
        let backend = self.backend.as_ref().ok_or("clipboard unavailable")?;
        *self.last_hash.lock() = Some(content.hash());
        backend.write(content)?;
        *self.last_counter.lock() = backend.change_counter();
        // Some platforms convert content on write (e.g. image formats); use
        // whatever reads back so the watcher sees no change.
        if let Some(back) = backend.read(matches!(content, ClipContent::Image { .. })) {
            *self.last_hash.lock() = Some(back.hash());
        }
        Ok(())
    }

    /// Initialise "last seen" with the current clipboard so whatever was
    /// copied before sync was enabled is not broadcast.
    fn prime(&self, images: bool) {
        if let Some(b) = &self.backend {
            *self.last_counter.lock() = b.change_counter();
            *self.last_hash.lock() = b.read(images).map(|c| c.hash());
        }
    }
}

pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        let mut writer = enc.write_header().map_err(|e| Error::Other(e.to_string()))?;
        writer.write_image_data(rgba).map_err(|e| Error::Other(e.to_string()))?;
    }
    Ok(out)
}

pub fn decode_png(bytes: &[u8]) -> Result<ClipContent> {
    let mut decoder = png::Decoder::new_with_limits(
        std::io::Cursor::new(bytes),
        png::Limits { bytes: (MAX_IMAGE_PIXELS * 4) as usize + 1024 },
    );
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().map_err(|e| Error::Protocol(format!("bad image: {e}")))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| Error::Protocol(format!("bad image: {e}")))?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(Error::Protocol("unsupported image format".into()));
    }
    buf.truncate(info.buffer_size());
    Ok(ClipContent::Image { width: info.width, height: info.height, rgba: buf })
}

impl Engine {
    pub fn clipboard_available(&self) -> bool {
        self.clipboard.backend.is_some()
    }

    pub(crate) fn start_clipboard_watcher(&self) {
        if self.clipboard.backend.is_none() {
            return;
        }
        let engine = self.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(POLL_INTERVAL);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut was_enabled = false;
            let mut n: u64 = 0;
            loop {
                tokio::select! {
                    _ = tick.tick() => {}
                    _ = engine.shutdown.cancelled() => break,
                }
                n += 1;
                let (enabled, images) = {
                    let s = engine.settings.read();
                    (s.clipboard_auto_sync, s.clipboard_images)
                };
                if !enabled {
                    was_enabled = false;
                    continue;
                }
                let e2 = engine.clone();
                let has_counter = engine.clipboard.backend.as_ref().map(|b| b.change_counter().is_some()).unwrap_or(false);
                // Without a change counter every check reads the clipboard;
                // decoding images is costly, so check those less often.
                let read_images = images && (has_counter || n % 4 == 0);
                let first = !was_enabled;
                was_enabled = true;
                let polled = tokio::task::spawn_blocking(move || {
                    if first {
                        e2.clipboard.prime(read_images);
                        None
                    } else {
                        e2.clipboard.poll(read_images)
                    }
                })
                .await
                .ok()
                .flatten();
                if let Some((content, hash, sensitive)) = polled {
                    if sensitive {
                        tracing::debug!("skipping clipboard content marked as sensitive");
                        continue;
                    }
                    engine.on_local_clipboard(content, hash).await;
                }
            }
        });
    }

    async fn on_local_clipboard(&self, content: ClipContent, hash: String) {
        if !content.within_limits() {
            return;
        }
        self.record_clip("local", None, &content, &hash);
        let targets: Vec<String> = self
            .registry
            .online_ids()
            .into_iter()
            .filter(|id| self.trust_of(id) == TrustState::Trusted)
            .filter(|id| self.db.effective_clipboard_sync(id).unwrap_or(false))
            .collect();
        if targets.is_empty() {
            return;
        }
        let png = match &content {
            ClipContent::Image { width, height, rgba } => {
                let (w, h, px) = (*width, *height, rgba.clone());
                match tokio::task::spawn_blocking(move || encode_png(w, h, &px)).await {
                    Ok(Ok(p)) => Some(Arc::new(p)),
                    _ => return,
                }
            }
            _ => None,
        };
        for id in targets {
            let engine = self.clone();
            let (content, hash, png) = (content.clone(), hash.clone(), png.clone());
            tokio::spawn(async move {
                if let Err(e) = engine.push_clipboard(&id, &content, &hash, png.as_deref().map(|v| v.as_slice()), false).await {
                    tracing::debug!(device = %id, "clipboard sync failed: {e}");
                }
            });
        }
    }

    fn record_clip(&self, origin: &str, device_id: Option<&str>, content: &ClipContent, hash: &str) {
        let (enabled, keep) = {
            let s = self.settings.read();
            (s.clipboard_history_enabled, s.clipboard_history_limit)
        };
        let device_name = device_id.map(|d| self.device_name(d));
        let (text, width, height) = match content {
            ClipContent::Text(t) => (Some(t.chars().take(HISTORY_TEXT_CHARS).collect::<String>()), None, None),
            ClipContent::Image { width, height, .. } => (None, Some(*width), Some(*height)),
        };
        let clip = NewClip {
            origin,
            device_id,
            device_name: device_name.as_deref(),
            kind: content.kind(),
            text: text.as_deref(),
            width,
            height,
            byte_size: content.byte_size(),
            hash,
        };
        let entry = if enabled {
            match self.db.insert_clip(&clip, keep) {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("clipboard history write failed: {e}");
                    return;
                }
            }
        } else {
            // Still tell the UI something happened, without persisting it.
            ClipboardEntryView {
                id: 0,
                origin: origin.into(),
                device_id: device_id.map(Into::into),
                device_name: device_name.clone(),
                kind: content.kind().into(),
                text: None,
                width,
                height,
                byte_size: content.byte_size(),
                created_at: crate::util::now_ms(),
            }
        };
        self.emit(Event::ClipboardChanged { entry });
    }

    async fn push_clipboard(
        &self,
        device_id: &str,
        content: &ClipContent,
        hash: &str,
        png: Option<&[u8]>,
        manual: bool,
    ) -> Result<()> {
        let mut conn = self.connect_device(device_id, Purpose::Clipboard).await?;
        let (text, width, height) = match content {
            ClipContent::Text(t) => (Some(t.clone()), None, None),
            ClipContent::Image { width, height, .. } => (None, Some(*width), Some(*height)),
        };
        conn.send(&Message::Clipboard(ClipboardPayload {
            hash: hash.into(),
            kind: content.kind().into(),
            text,
            width,
            height,
            manual,
        }))
        .await?;
        if let Some(png) = png {
            write_data(&mut conn.w, png).await?;
        }
        match tokio::time::timeout(Duration::from_secs(15), conn.recv()).await?? {
            Message::Ack => Ok(()),
            other => Err(Error::Protocol(format!("unexpected reply {other:?}"))),
        }
    }

    pub(crate) async fn handle_clipboard(&self, conn: &mut Conn) -> Result<()> {
        let peer = conn.peer.id.clone();
        if self.trust_of(&peer) != TrustState::Trusted {
            conn.send(&Message::error("not_trusted", "pair with this device first")).await?;
            self.record_security("rejected_untrusted", Some(&peer), "Ignored clipboard data from a device that is not trusted");
            return Ok(());
        }
        let payload = match tokio::time::timeout(Duration::from_secs(15), conn.recv()).await?? {
            Message::Clipboard(p) => p,
            _ => return Err(Error::Protocol("expected clipboard payload".into())),
        };
        let (auto, images) = {
            let s = self.settings.read();
            (s.clipboard_auto_sync, s.clipboard_images)
        };
        if !self.db.effective_clipboard_sync(&peer)? {
            conn.send(&Message::error("rejected", "clipboard sync is disabled for your device")).await?;
            return Ok(());
        }
        if !payload.manual && !auto {
            conn.send(&Message::error("rejected", "automatic clipboard sync is turned off")).await?;
            return Ok(());
        }
        let content = match payload.kind.as_str() {
            "text" => ClipContent::Text(payload.text.unwrap_or_default()),
            "image" => {
                let mut buf = Vec::new();
                let n = match tokio::time::timeout(Duration::from_secs(30), read_frame(&mut conn.r, &mut buf)).await?? {
                    Frame::Data(n) => n,
                    Frame::Control(_) => return Err(Error::Protocol("expected image data".into())),
                };
                if !images {
                    conn.send(&Message::error("rejected", "image clipboard sync is turned off")).await?;
                    return Ok(());
                }
                buf.truncate(n);
                tokio::task::spawn_blocking(move || decode_png(&buf))
                    .await
                    .map_err(|e| Error::Other(e.to_string()))??
            }
            _ => return Err(Error::Protocol("unknown clipboard kind".into())),
        };
        if !content.within_limits() {
            conn.send(&Message::error("rejected", "clipboard content too large")).await?;
            return Ok(());
        }
        let hash = content.hash();
        if hash != payload.hash {
            return Err(Error::Protocol("clipboard content does not match its hash".into()));
        }
        let engine = self.clone();
        let c2 = content.clone();
        tokio::task::spawn_blocking(move || engine.clipboard.apply_remote(&c2))
            .await
            .map_err(|e| Error::Other(e.to_string()))?
            .map_err(Error::Other)?;
        conn.send(&Message::Ack).await?;
        self.record_clip("remote", Some(&peer), &content, &hash);
        Ok(())
    }

    /// Sends whatever is on the clipboard now to specific devices, even if
    /// automatic sync is off.
    pub async fn send_clipboard(&self, recipients: Vec<String>) -> Result<Vec<SendResult>> {
        let images = self.settings.read().clipboard_images;
        let backend = self.clipboard.backend.clone().ok_or(Error::Other("clipboard unavailable".into()))?;
        let (content, sensitive) = tokio::task::spawn_blocking(move || (backend.read(images), backend.is_sensitive()))
            .await
            .map_err(|e| Error::Other(e.to_string()))?;
        let content = content.ok_or_else(|| Error::Invalid("the clipboard is empty".into()))?;
        if sensitive {
            return Err(Error::Invalid("the clipboard holds content marked as confidential".into()));
        }
        self.push_to(content, recipients).await
    }

    /// Sends a specific text (e.g. from clipboard history) to devices' clipboards.
    pub async fn send_clipboard_text(&self, text: String, recipients: Vec<String>) -> Result<Vec<SendResult>> {
        self.push_to(ClipContent::Text(text), recipients).await
    }

    async fn push_to(&self, content: ClipContent, recipients: Vec<String>) -> Result<Vec<SendResult>> {
        if !content.within_limits() {
            return Err(Error::Invalid("clipboard content is empty or too large".into()));
        }
        let hash = content.hash();
        let png = match &content {
            ClipContent::Image { width, height, rgba } => Some(encode_png(*width, *height, rgba)?),
            _ => None,
        };
        let mut out = Vec::new();
        for id in recipients {
            let r = if self.trust_of(&id) != TrustState::Trusted {
                Err(Error::NotTrusted)
            } else {
                self.push_clipboard(&id, &content, &hash, png.as_deref(), true).await
            };
            out.push(SendResult {
                device_name: self.device_name(&id),
                ok: r.is_ok(),
                error: r.err().map(|e| e.to_string()),
                device_id: id,
            });
        }
        Ok(out)
    }

    /// Puts text on the local clipboard (e.g. "Copy" on a received message).
    pub async fn copy_text(&self, text: String) -> Result<()> {
        let backend = self.clipboard.backend.clone().ok_or(Error::Other("clipboard unavailable".into()))?;
        tokio::task::spawn_blocking(move || backend.write(&ClipContent::Text(text)))
            .await
            .map_err(|e| Error::Other(e.to_string()))?
            .map_err(Error::Other)
    }

    pub async fn copy_clip(&self, id: i64) -> Result<()> {
        let text = self.db.get_clip_text(id)?.ok_or_else(|| Error::Invalid("only text entries can be copied".into()))?;
        self.copy_text(text).await
    }

    pub fn clipboard_history(&self, limit: u32) -> Result<Vec<ClipboardEntryView>> {
        self.db.list_clips(limit)
    }

    pub fn delete_clip(&self, id: i64) -> Result<()> {
        self.db.delete_clip(id)
    }

    pub fn clear_clipboard_history(&self) -> Result<()> {
        self.db.clear_clips()
    }

    pub fn clip_text(&self, id: i64) -> Result<Option<String>> {
        self.db.get_clip_text(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_round_trip() {
        let rgba: Vec<u8> = (0..4 * 3 * 2).map(|i| i as u8).collect();
        let png = encode_png(3, 2, &rgba).unwrap();
        assert_eq!(decode_png(&png).unwrap(), ClipContent::Image { width: 3, height: 2, rgba });
    }

    #[test]
    fn remote_writes_are_not_seen_as_local_changes() {
        let mem = Arc::new(MemoryClipboard::default());
        let state = ClipboardState::new(Some(mem.clone()));
        state.prime(true);
        assert!(state.poll(true).is_none());

        mem.set(ClipContent::Text("typed locally".into()));
        let (c, _, sensitive) = state.poll(true).expect("local copy detected");
        assert_eq!(c, ClipContent::Text("typed locally".into()));
        assert!(!sensitive);
        assert!(state.poll(true).is_none(), "same content is reported once");

        state.apply_remote(&ClipContent::Text("from peer".into())).unwrap();
        assert!(state.poll(true).is_none(), "applying remote content must not echo back");

        // Re-copying content identical to what we last had is not a change.
        mem.set(ClipContent::Text("from peer".into()));
        assert!(state.poll(true).is_none());

        mem.set_sensitive(ClipContent::Text("hunter2".into()));
        assert!(state.poll(true).unwrap().2, "sensitive flag surfaces");
    }
}
