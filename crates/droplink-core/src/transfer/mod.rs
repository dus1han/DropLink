//! File and folder transfer engine.
//!
//! Each delivery (one session to one device) runs as its own task. Pause,
//! network interruption and app restarts all leave partial files on the
//! receiver; the next attempt reuses the same session id and the receiver
//! answers with per-file offsets, so only missing bytes are sent. Both sides
//! hash every file while streaming and compare SHA-256 digests at the end.

pub mod files;
mod incoming;
mod outgoing;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tokio::sync::{oneshot, Notify};
use tokio_util::sync::CancellationToken;

use crate::db::{DeliveryRow, HistoryRecord};
use crate::engine::Engine;
use crate::events::Event;
use crate::models::*;
use crate::net::Conn;
use crate::protocol::{Message, Purpose};
use crate::settings::Settings;
use crate::util::now_ms;
use crate::{Error, Result};

pub use incoming::IncomingDecision;

const PARTIAL_DIR: &str = ".droplink-partial";

pub(crate) fn partial_dir(dest: &Path, delivery_id: &str) -> PathBuf {
    dest.join(PARTIAL_DIR).join(delivery_id)
}

pub(crate) fn remove_partials(dest: &Path, delivery_id: &str) {
    let dir = partial_dir(dest, delivery_id);
    if dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            tracing::warn!("could not remove {}: {e}", dir.display());
        }
    }
    // Remove the parent too once no other transfer is using it.
    let _ = std::fs::remove_dir(dest.join(PARTIAL_DIR));
}

fn dir_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum())
        .unwrap_or(0)
}

// ---- control ----------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Intent {
    None = 0,
    Pause = 1,
    Cancel = 2,
    Shutdown = 3,
    /// Remote side cancelled out-of-band (via `Abort`).
    PeerCancel = 4,
}

#[derive(Clone)]
pub(crate) struct ActiveCtl {
    pub token: CancellationToken,
    intent: Arc<AtomicU8>,
    pub device_id: String,
}

impl ActiveCtl {
    pub fn stop(&self, intent: Intent) {
        self.intent.store(intent as u8, Ordering::SeqCst);
        self.token.cancel();
    }
    pub fn intent(&self) -> Intent {
        match self.intent.load(Ordering::SeqCst) {
            1 => Intent::Pause,
            2 => Intent::Cancel,
            3 => Intent::Shutdown,
            4 => Intent::PeerCancel,
            _ => Intent::None,
        }
    }
    /// Outcome to report when the token fired.
    pub fn stopped_outcome(&self) -> Outcome {
        match self.intent() {
            Intent::Pause => Outcome::Paused,
            Intent::Cancel => Outcome::Cancelled,
            Intent::PeerCancel => Outcome::CancelledByPeer,
            Intent::Shutdown | Intent::None => Outcome::Shutdown,
        }
    }
    /// The message that tells the peer why we stopped.
    pub fn stop_message(&self) -> Option<Message> {
        match self.intent() {
            Intent::Pause => Some(Message::Pause),
            Intent::Cancel => Some(Message::Cancel),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum Outcome {
    Completed,
    Paused,
    PausedByPeer,
    Cancelled,
    CancelledByPeer,
    Rejected(String),
    IntegrityFailed(u32),
    Shutdown,
}

/// Limits how many deliveries stream at once. Unlike a semaphore the limit
/// can shrink while permits are held.
pub(crate) struct Gate {
    limit: AtomicUsize,
    running: Mutex<usize>,
    notify: Notify,
}

pub(crate) struct GatePermit<'a>(&'a Gate);

impl Drop for GatePermit<'_> {
    fn drop(&mut self) {
        *self.0.running.lock() -= 1;
        self.0.notify.notify_waiters();
    }
}

impl Gate {
    fn new(limit: usize) -> Self {
        Self { limit: AtomicUsize::new(limit.max(1)), running: Mutex::new(0), notify: Notify::new() }
    }

    pub async fn acquire(&self) -> GatePermit<'_> {
        loop {
            let notified = self.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            {
                let mut running = self.running.lock();
                if *running < self.limit.load(Ordering::SeqCst) {
                    *running += 1;
                    return GatePermit(self);
                }
            }
            notified.await;
        }
    }

    fn set_limit(&self, limit: usize) {
        self.limit.store(limit.max(1), Ordering::SeqCst);
        self.notify.notify_waiters();
    }
}

/// Token bucket shared by every outgoing stream. Tokens may go negative
/// (debt), which keeps the average rate exact without splitting chunks.
pub(crate) struct RateLimiter {
    bytes_per_sec: AtomicU64,
    state: Mutex<(f64, Instant)>,
}

impl RateLimiter {
    fn new(mbps: f64) -> Self {
        let r = Self { bytes_per_sec: AtomicU64::new(0), state: Mutex::new((0.0, Instant::now())) };
        r.set_mbps(mbps);
        r
    }

    fn set_mbps(&self, mbps: f64) {
        self.bytes_per_sec.store((mbps * 1_000_000.0) as u64, Ordering::SeqCst);
    }

    pub async fn acquire(&self, n: usize) {
        let rate = self.bytes_per_sec.load(Ordering::SeqCst) as f64;
        if rate <= 0.0 {
            return;
        }
        let wait = {
            let mut st = self.state.lock();
            let now = Instant::now();
            let elapsed = now.duration_since(st.1).as_secs_f64();
            st.0 = (st.0 + elapsed * rate).min(rate * 0.5);
            st.1 = now;
            st.0 -= n as f64;
            if st.0 < 0.0 {
                -st.0 / rate
            } else {
                0.0
            }
        };
        if wait > 0.0 {
            tokio::time::sleep(Duration::from_secs_f64(wait)).await;
        }
    }
}

pub(crate) struct PendingIncoming {
    pub view: IncomingRequestView,
    pub tx: Option<oneshot::Sender<IncomingDecision>>,
}

pub struct TransferState {
    active: Mutex<HashMap<String, ActiveCtl>>,
    last_auto_attempt: Mutex<HashMap<String, Instant>>,
    pub(crate) gate: Gate,
    pub(crate) limiter: RateLimiter,
}

impl TransferState {
    pub fn new(s: &Settings) -> Self {
        Self {
            active: Mutex::new(HashMap::new()),
            last_auto_attempt: Mutex::new(HashMap::new()),
            gate: Gate::new(s.max_concurrent_transfers as usize),
            limiter: RateLimiter::new(s.bandwidth_limit_mbps),
        }
    }

    pub fn apply_settings(&self, s: &Settings) {
        self.gate.set_limit(s.max_concurrent_transfers as usize);
        self.limiter.set_mbps(s.bandwidth_limit_mbps);
    }

    /// Registers a running delivery; `None` if it is already running.
    pub(crate) fn register(&self, id: &str, device_id: &str) -> Option<ActiveCtl> {
        let mut map = self.active.lock();
        if map.contains_key(id) {
            return None;
        }
        let ctl = ActiveCtl {
            token: CancellationToken::new(),
            intent: Arc::new(AtomicU8::new(0)),
            device_id: device_id.into(),
        };
        map.insert(id.into(), ctl.clone());
        Some(ctl)
    }

    pub(crate) fn unregister(&self, id: &str) {
        self.active.lock().remove(id);
    }

    pub(crate) fn get(&self, id: &str) -> Option<ActiveCtl> {
        self.active.lock().get(id).cloned()
    }

    pub fn is_active(&self, id: &str) -> bool {
        self.active.lock().contains_key(id)
    }

    pub fn cancel_for_device(&self, device_id: &str) {
        for ctl in self.active.lock().values().filter(|c| c.device_id == device_id) {
            ctl.stop(Intent::Cancel);
        }
    }

    pub fn cancel_all_for_shutdown(&self) {
        for ctl in self.active.lock().values() {
            ctl.stop(Intent::Shutdown);
        }
    }

    fn may_auto_attempt(&self, id: &str) -> bool {
        let mut map = self.last_auto_attempt.lock();
        let now = Instant::now();
        match map.get(id) {
            Some(t) if now.duration_since(*t) < Duration::from_secs(30) => false,
            _ => {
                map.insert(id.into(), now);
                true
            }
        }
    }
}

// ---- progress ---------------------------------------------------------

pub(crate) struct Progress {
    delivery_id: String,
    pub total_bytes: u64,
    pub total_files: u32,
    pub bytes: u64,
    pub files: u32,
    pub current_file: Option<String>,
    speed: f64,
    window_start: Instant,
    window_bytes: u64,
    last_emit: Instant,
    last_db: Instant,
}

impl Progress {
    pub fn new(row: &DeliveryRow, bytes: u64, files: u32) -> Self {
        let now = Instant::now();
        Self {
            delivery_id: row.id.clone(),
            total_bytes: row.total_bytes,
            total_files: row.total_files,
            bytes,
            files,
            current_file: None,
            speed: 0.0,
            window_start: now,
            window_bytes: 0,
            last_emit: now - Duration::from_secs(1),
            last_db: now,
        }
    }

    pub fn add(&mut self, n: usize) {
        self.bytes += n as u64;
        self.window_bytes += n as u64;
    }

    pub fn tick(&mut self, engine: &Engine, force: bool) {
        let now = Instant::now();
        let window = now.duration_since(self.window_start).as_secs_f64();
        if window >= 0.5 {
            let inst = self.window_bytes as f64 / window;
            self.speed = if self.speed == 0.0 { inst } else { self.speed * 0.6 + inst * 0.4 };
            self.window_start = now;
            self.window_bytes = 0;
        }
        if force || now.duration_since(self.last_emit) >= Duration::from_millis(150) {
            self.last_emit = now;
            let remaining = self.total_bytes.saturating_sub(self.bytes) as f64;
            engine.emit(Event::DeliveryProgress {
                progress: ProgressView {
                    delivery_id: self.delivery_id.clone(),
                    bytes_done: self.bytes,
                    total_bytes: self.total_bytes,
                    files_done: self.files,
                    total_files: self.total_files,
                    speed_bps: self.speed,
                    eta_seconds: (self.speed > 1.0).then(|| remaining / self.speed),
                    current_file: self.current_file.clone(),
                },
            });
        }
        if force || now.duration_since(self.last_db) >= Duration::from_secs(1) {
            self.last_db = now;
            let _ = engine.db.set_delivery_progress(&self.delivery_id, self.bytes, self.files);
        }
    }
}

// ---- engine API -------------------------------------------------------

impl Engine {
    pub fn list_deliveries(&self, limit: u32) -> Result<Vec<DeliveryView>> {
        Ok(self.db.list_deliveries(limit)?.iter().map(DeliveryRow::to_view).collect())
    }

    pub(crate) fn emit_delivery(&self, id: &str) {
        if let Ok(Some(row)) = self.db.get_delivery(id) {
            self.emit(Event::DeliveryUpdated { delivery: row.to_view() });
        }
    }

    pub(crate) fn set_status(&self, id: &str, status: DeliveryStatus, error: Option<&str>) {
        if let Err(e) = self.db.set_delivery_status(id, status, error) {
            tracing::warn!("failed to update delivery {id}: {e}");
        }
        self.emit_delivery(id);
    }

    /// Writes the final state of a delivery attempt, its history record and
    /// the matching event.
    pub(crate) fn finalize(&self, id: &str, result: Result<Outcome>) {
        let Ok(Some(row)) = self.db.get_delivery(id) else { return };
        let outgoing = row.direction == Direction::Outgoing;
        let peer = row.device_name.clone();
        let (status, error, verified, auto_resume): (DeliveryStatus, Option<String>, Option<bool>, bool) =
            match result {
                Ok(Outcome::Completed) => (DeliveryStatus::Completed, None, Some(true), false),
                Ok(Outcome::Paused) => (DeliveryStatus::Paused, None, None, false),
                Ok(Outcome::PausedByPeer) => {
                    (DeliveryStatus::Paused, Some(format!("Paused by {peer}")), None, false)
                }
                Ok(Outcome::Cancelled) => (DeliveryStatus::Cancelled, None, None, false),
                Ok(Outcome::CancelledByPeer) => {
                    (DeliveryStatus::Cancelled, Some(format!("Cancelled by {peer}")), None, false)
                }
                Ok(Outcome::Rejected(reason)) => (DeliveryStatus::Rejected, Some(reason), None, false),
                Ok(Outcome::IntegrityFailed(n)) => (
                    DeliveryStatus::Failed,
                    Some(format!("{n} file(s) failed SHA-256 verification and were discarded")),
                    Some(false),
                    false,
                ),
                Ok(Outcome::Shutdown) => (
                    DeliveryStatus::Paused,
                    Some("Interrupted — DropLink was closed".into()),
                    None,
                    outgoing,
                ),
                Err(e) if e.is_transient() => {
                    let msg = if outgoing {
                        format!("Connection lost ({e}). Resumes when {peer} is back online.")
                    } else {
                        format!("Connection lost ({e}). Waiting for {peer} to resume.")
                    };
                    (DeliveryStatus::Paused, Some(msg), None, outgoing)
                }
                Err(e) => (DeliveryStatus::Failed, Some(e.to_string()), None, false),
            };

        let _ = self.db.set_delivery_status(id, status, error.as_deref());
        if let Some(v) = verified {
            let _ = self.db.set_delivery_verified(id, v);
        }
        if auto_resume {
            let _ = self.db.set_auto_resume(id, true);
        }
        if !outgoing && matches!(status, DeliveryStatus::Completed | DeliveryStatus::Cancelled) {
            if let Some(dest) = row.dest_dir.as_deref() {
                remove_partials(Path::new(dest), id);
            }
        }
        let Ok(Some(row)) = self.db.get_delivery(id) else { return };
        if status.is_terminal() {
            self.record_delivery_history(&row);
        }
        self.emit(Event::DeliveryUpdated { delivery: row.to_view() });
    }

    fn record_delivery_history(&self, row: &DeliveryRow) {
        let me = self.identity.device_id.clone();
        let my_name = self.settings.read().device_name.clone();
        let kind = if self.db.session_has_dirs(&row.session_id).unwrap_or(false) { "folder" } else { "files" };
        let (sender_id, sender_name, recipient_id, recipient_name) = match row.direction {
            Direction::Outgoing => (me, my_name, row.device_id.clone(), row.device_name.clone()),
            Direction::Incoming => (row.device_id.clone(), row.device_name.clone(), me, my_name),
        };
        let rec = HistoryRecord {
            delivery: row,
            kind,
            sender_id: &sender_id,
            sender_name: &sender_name,
            recipient_id: &recipient_id,
            recipient_name: &recipient_name,
        };
        if let Err(e) = self.db.record_history(&rec) {
            tracing::warn!("failed to record history: {e}");
        }
    }

    pub fn pause_delivery(&self, id: &str) -> Result<()> {
        let ctl = self.transfers.get(id).ok_or_else(|| Error::Invalid("transfer is not running".into()))?;
        ctl.stop(Intent::Pause);
        Ok(())
    }

    /// Resumes a paused delivery or retries a failed/cancelled one.
    pub async fn resume_delivery(&self, id: &str) -> Result<()> {
        let row = self.db.get_delivery(id)?.ok_or(Error::Invalid("unknown transfer".into()))?;
        if self.transfers.is_active(id) {
            return Ok(());
        }
        match row.direction {
            Direction::Outgoing => {
                if row.status == DeliveryStatus::Completed {
                    return Err(Error::Invalid("transfer already completed".into()));
                }
                if self.trust_of(&row.device_id) != TrustState::Trusted {
                    return Err(Error::NotTrusted);
                }
                self.spawn_outgoing(id);
                Ok(())
            }
            Direction::Incoming => {
                if row.status != DeliveryStatus::Paused {
                    return Err(Error::Invalid("only paused transfers can be resumed by the receiver".into()));
                }
                let mut conn = self.connect_device(&row.device_id, Purpose::Control).await?;
                conn.send(&Message::ResumeRequest { session_id: id.into() }).await?;
                match tokio::time::timeout(Duration::from_secs(10), conn.recv()).await?? {
                    Message::Ack => Ok(()),
                    _ => Err(Error::Protocol("unexpected reply".into())),
                }
            }
        }
    }

    pub async fn cancel_delivery(&self, id: &str) -> Result<()> {
        if let Some(ctl) = self.transfers.get(id) {
            ctl.stop(Intent::Cancel);
            return Ok(());
        }
        let row = self.db.get_delivery(id)?.ok_or(Error::Invalid("unknown transfer".into()))?;
        if row.status.is_terminal() {
            return Ok(());
        }
        self.finalize(id, Ok(Outcome::Cancelled));
        // Tell the other side so it can free partial data; best effort.
        let engine = self.clone();
        let (id, device) = (id.to_string(), row.device_id.clone());
        tokio::spawn(async move {
            if let Ok(mut conn) = engine.connect_device(&device, Purpose::Control).await {
                let _ = conn.send(&Message::Abort { session_id: id }).await;
                let _ = tokio::time::timeout(Duration::from_secs(5), conn.recv()).await;
            }
        });
        Ok(())
    }

    pub fn clear_finished_deliveries(&self) -> Result<usize> {
        self.db.clear_finished_deliveries()
    }

    pub fn incoming_requests(&self) -> Vec<IncomingRequestView> {
        let mut v: Vec<_> = self.incoming.lock().values().map(|p| p.view.clone()).collect();
        v.sort_by_key(|r| r.created_at);
        v
    }

    pub fn respond_incoming(
        &self,
        request_id: &str,
        accept: bool,
        dest_dir: Option<PathBuf>,
        overwrite: Option<bool>,
    ) -> Result<()> {
        if let Some(d) = &dest_dir {
            if !d.is_dir() {
                return Err(Error::Invalid(format!("{} is not a folder", d.display())));
            }
        }
        let tx = {
            let mut map = self.incoming.lock();
            let p = map.get_mut(request_id).ok_or_else(|| Error::Invalid("request has expired".into()))?;
            p.tx.take()
        };
        if let Some(tx) = tx {
            let decision = if accept {
                IncomingDecision::Accept { dest_dir, overwrite }
            } else {
                IncomingDecision::Reject
            };
            let _ = tx.send(decision);
        }
        Ok(())
    }

    /// Small out-of-band requests about an existing delivery.
    pub(crate) async fn handle_control(&self, conn: &mut Conn) -> Result<()> {
        let peer = conn.peer.id.clone();
        let msg = tokio::time::timeout(Duration::from_secs(10), conn.recv()).await??;
        let reply = match msg {
            Message::ResumeRequest { session_id } => match self.db.get_delivery(&session_id)? {
                Some(row)
                    if row.direction == Direction::Outgoing
                        && row.device_id == peer
                        && matches!(row.status, DeliveryStatus::Paused | DeliveryStatus::Failed) =>
                {
                    if self.trust_of(&peer) != TrustState::Trusted {
                        Message::error("not_trusted", "device is not trusted")
                    } else {
                        self.spawn_outgoing(&session_id);
                        Message::Ack
                    }
                }
                _ => Message::error("rejected", "no paused transfer with that id"),
            },
            Message::Abort { session_id } => match self.db.get_delivery(&session_id)? {
                Some(row) if row.device_id == peer => {
                    if let Some(ctl) = self.transfers.get(&session_id) {
                        ctl.stop(Intent::PeerCancel);
                    } else if !row.status.is_terminal() {
                        self.finalize(&session_id, Ok(Outcome::CancelledByPeer));
                    }
                    Message::Ack
                }
                _ => Message::Ack,
            },
            _ => Message::error("protocol", "unexpected control message"),
        };
        conn.send(&reply).await
    }

    /// Restarts deliveries that were interrupted while `device_id` was away.
    pub(crate) fn resume_waiting_for(&self, device_id: &str) {
        let Ok(ids) = self.db.auto_resume_candidates(device_id) else { return };
        for id in ids {
            if !self.transfers.is_active(&id) && self.transfers.may_auto_attempt(&id) {
                tracing::info!(delivery = %id, "auto-resuming interrupted transfer");
                self.spawn_outgoing(&id);
            }
        }
    }

    /// Partial files kept for paused incoming transfers.
    pub fn partial_cache(&self) -> Result<(u32, u64)> {
        let mut count = 0;
        let mut bytes = 0;
        for (id, dest) in self.db.resumable_incoming(i64::MAX)? {
            let dir = partial_dir(Path::new(&dest), &id);
            if dir.exists() {
                count += 1;
                bytes += dir_size(&dir);
            }
        }
        Ok((count, bytes))
    }

    /// Discards partial data of paused incoming transfers older than `max_age_ms`
    /// (0 = all of them). Those transfers become failed.
    pub fn expire_stale_partials(&self, max_age_ms: i64) -> Result<u32> {
        let cutoff = now_ms() - max_age_ms;
        let mut n = 0;
        for (id, dest) in self.db.resumable_incoming(cutoff)? {
            if self.transfers.is_active(&id) {
                continue;
            }
            remove_partials(Path::new(&dest), &id);
            let _ = self.db.set_delivery_status(&id, DeliveryStatus::Failed, Some("Partial data discarded"));
            if let Ok(Some(row)) = self.db.get_delivery(&id) {
                self.record_delivery_history(&row);
                self.emit(Event::DeliveryUpdated { delivery: row.to_view() });
            }
            n += 1;
        }
        Ok(n)
    }
}
