//! Operating-system clipboard via `arboard`, plus platform extras.
//!
//! * Windows: `GetClipboardSequenceNumber` makes change detection a cheap
//!   counter read, and the formats password managers publish
//!   (`ExcludeClipboardContentFromMonitorProcessing`, `Clipboard Viewer
//!   Ignore`, and `CanIncludeInClipboardHistory` set to 0) mark content as
//!   sensitive so it is never synchronised.
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
        CloseClipboard, GetClipboardData, GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard,
        RegisterClipboardFormatW,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    pub fn sequence_number() -> u64 {
        // SAFETY: no arguments, no preconditions.
        unsafe { GetClipboardSequenceNumber() as u64 }
    }

    /// Formats whose mere presence marks content as confidential.
    const SENSITIVE_FORMATS: &[&str] = &["ExcludeClipboardContentFromMonitorProcessing", "Clipboard Viewer Ignore"];

    /// A DWORD flag: 0 keeps the content out of clipboard history (password
    /// managers), 1 explicitly allows it. Windows' own screenshot tools set
    /// it to 1, so only the value 0 counts as confidential.
    const HISTORY_FLAG: &str = "CanIncludeInClipboardHistory";

    fn format_id(name: &str) -> u32 {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: `wide` is a valid NUL-terminated UTF-16 string that
        // outlives the call.
        unsafe { RegisterClipboardFormatW(wide.as_ptr()) }
    }

    fn available(fmt: u32) -> bool {
        // SAFETY: IsClipboardFormatAvailable does not require the clipboard
        // to be open and has no other preconditions.
        fmt != 0 && unsafe { IsClipboardFormatAvailable(fmt) } != 0
    }

    pub fn has_sensitive_format() -> bool {
        if SENSITIVE_FORMATS.iter().any(|n| available(format_id(n))) {
            return true;
        }
        let flag = format_id(HISTORY_FLAG);
        if !available(flag) {
            return false;
        }
        // Unreadable flag: err on the side of not sharing.
        read_dword(flag).map(|v| v == 0).unwrap_or(true)
    }

    fn read_dword(fmt: u32) -> Option<u32> {
        // Another process may hold the clipboard for a moment; retry briefly.
        let mut opened = false;
        for _ in 0..10 {
            // SAFETY: a null owner window is allowed; the clipboard is closed
            // below on every path once opened.
            if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if !opened {
            return None;
        }
        // SAFETY: the clipboard is open. The handle returned by
        // GetClipboardData is owned by the clipboard and stays valid until it
        // is closed; it is only read while locked, and only when it holds at
        // least four bytes.
        let value = unsafe {
            let h = GetClipboardData(fmt);
            if h.is_null() {
                None
            } else {
                let p = GlobalLock(h) as *const u8;
                let v = if !p.is_null() && GlobalSize(h) >= 4 {
                    Some(u32::from_le_bytes(std::ptr::read_unaligned(p as *const [u8; 4])))
                } else {
                    None
                };
                if !p.is_null() {
                    GlobalUnlock(h);
                }
                v
            }
        };
        // SAFETY: we opened the clipboard above.
        unsafe { CloseClipboard() };
        value
    }
}
