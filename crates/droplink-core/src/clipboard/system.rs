//! Operating-system clipboard via `arboard`, plus platform extras.
//!
//! * Windows: `GetClipboardSequenceNumber` makes change detection a cheap
//!   counter read, and the formats password managers publish
//!   (`ExcludeClipboardContentFromMonitorProcessing`,
//!   `CanIncludeInClipboardHistory`, `Clipboard Viewer Ignore`) mark content
//!   as sensitive so it is never synchronised.
//! * macOS / Linux: no change counter is exposed through `arboard`, so the
//!   watcher compares content hashes. macOS's `org.nspasteboard.ConcealedType`
//!   marker is not visible through `arboard`; see docs/PLATFORM_NOTES.md.

use std::borrow::Cow;
use std::sync::Arc;

use parking_lot::Mutex;

use super::{ClipContent, ClipboardBackend};

pub struct SystemClipboard {
    cb: Mutex<Option<arboard::Clipboard>>,
}

pub fn system_backend() -> Option<Arc<dyn ClipboardBackend>> {
    match arboard::Clipboard::new() {
        Ok(cb) => Some(Arc::new(SystemClipboard { cb: Mutex::new(Some(cb)) })),
        Err(e) => {
            tracing::warn!("system clipboard unavailable: {e}");
            None
        }
    }
}

impl SystemClipboard {
    fn with<T>(&self, f: impl FnOnce(&mut arboard::Clipboard) -> Option<T>) -> Option<T> {
        let mut g = self.cb.lock();
        if g.is_none() {
            *g = arboard::Clipboard::new().ok();
        }
        g.as_mut().and_then(f)
    }
}

impl ClipboardBackend for SystemClipboard {
    fn change_counter(&self) -> Option<u64> {
        #[cfg(windows)]
        {
            Some(win::sequence_number())
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    fn read(&self, images: bool) -> Option<ClipContent> {
        self.with(|cb| {
            if let Ok(t) = cb.get_text() {
                if !t.is_empty() {
                    return Some(ClipContent::Text(t));
                }
            }
            if images {
                if let Ok(img) = cb.get_image() {
                    return Some(ClipContent::Image {
                        width: img.width as u32,
                        height: img.height as u32,
                        rgba: img.bytes.into_owned(),
                    });
                }
            }
            None
        })
    }

    fn write(&self, content: &ClipContent) -> Result<(), String> {
        let mut g = self.cb.lock();
        if g.is_none() {
            *g = arboard::Clipboard::new().ok();
        }
        let cb = g.as_mut().ok_or("clipboard unavailable")?;
        match content {
            ClipContent::Text(t) => cb.set_text(t.clone()).map_err(|e| e.to_string()),
            ClipContent::Image { width, height, rgba } => cb
                .set_image(arboard::ImageData {
                    width: *width as usize,
                    height: *height as usize,
                    bytes: Cow::Borrowed(rgba),
                })
                .map_err(|e| e.to_string()),
        }
    }

    fn is_sensitive(&self) -> bool {
        #[cfg(windows)]
        {
            win::has_sensitive_format()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::System::DataExchange::{
        GetClipboardSequenceNumber, IsClipboardFormatAvailable, RegisterClipboardFormatW,
    };

    pub fn sequence_number() -> u64 {
        // SAFETY: no arguments, no preconditions.
        unsafe { GetClipboardSequenceNumber() as u64 }
    }

    const SENSITIVE_FORMATS: &[&str] = &[
        "ExcludeClipboardContentFromMonitorProcessing",
        "CanIncludeInClipboardHistory",
        "Clipboard Viewer Ignore",
    ];

    pub fn has_sensitive_format() -> bool {
        SENSITIVE_FORMATS.iter().any(|name| {
            let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            // SAFETY: `wide` is a valid NUL-terminated UTF-16 string that
            // outlives the call; IsClipboardFormatAvailable does not require
            // the clipboard to be open.
            unsafe {
                let fmt = RegisterClipboardFormatW(wide.as_ptr());
                fmt != 0 && IsClipboardFormatAvailable(fmt) != 0
            }
        })
    }
}
