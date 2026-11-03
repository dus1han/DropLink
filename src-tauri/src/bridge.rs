//! Forwards engine events to the webview and raises native notifications.

use std::collections::HashSet;
use std::sync::OnceLock;

use droplink_core::events::PairingRole;
use droplink_core::models::{DeliveryStatus, Direction};
use droplink_core::{Engine, Event, EventSink};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

pub const EVENT_CHANNEL: &str = "droplink://event";

pub struct TauriSink {
    app: AppHandle,
    engine: OnceLock<Engine>,
    notified: Mutex<HashSet<String>>,
}

impl TauriSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app, engine: OnceLock::new(), notified: Mutex::new(HashSet::new()) }
    }

    pub fn attach(&self, engine: Engine) {
        let _ = self.engine.set(engine);
    }

    fn window_focused(&self) -> bool {
        self.app
            .get_webview_window("main")
            .and_then(|w| Some(w.is_focused().ok()? && w.is_visible().ok()?))
            .unwrap_or(false)
    }

    fn notify(&self, title: &str, body: &str) {
        if let Err(e) = self.app.notification().builder().title(title).body(body).show() {
            tracing::warn!("notification failed: {e}");
        }
    }

    fn maybe_notify(&self, event: &Event) {
        let Some(engine) = self.engine.get() else { return };
        let prefs = engine.settings().notifications;
        if !prefs.enabled {
            return;
        }
        // Prompts that need an answer are always shown; status updates only
        // when the user is not already looking at the app.
        let focused = self.window_focused();
        match event {
            Event::IncomingRequest { request } if prefs.incoming_requests && !focused => {
                self.notify(
                    "Incoming transfer",
                    &format!("{} wants to send you {} ({})", request.device_name, request.label, format_bytes(request.total_bytes)),
                );
            }
            Event::PairingCode { role: PairingRole::Responder, device_name, code, .. } if prefs.pairing_requests && !focused => {
                self.notify("Pairing request", &format!("{device_name} wants to pair. Code: {code}"));
            }
            Event::TextReceived { message } if prefs.incoming_requests && !focused => {
                let preview: String = message.content.chars().take(120).collect();
                self.notify(&format!("Text from {}", message.device_name), &preview);
            }
            Event::DeliveryUpdated { delivery } if !focused => {
                let key = format!("{}:{:?}", delivery.id, delivery.status);
                let wanted = match delivery.status {
                    DeliveryStatus::Completed => prefs.transfer_completed,
                    DeliveryStatus::Failed => prefs.transfer_failed,
                    _ => false,
                };
                if !wanted || !self.notified.lock().insert(key) {
                    return;
                }
                let (title, body) = match (delivery.status, delivery.direction) {
                    (DeliveryStatus::Completed, Direction::Outgoing) => {
                        ("Transfer complete", format!("{} sent to {}", delivery.label, delivery.device_name))
                    }
                    (DeliveryStatus::Completed, Direction::Incoming) => {
                        ("Transfer received", format!("{} from {}", delivery.label, delivery.device_name))
                    }
                    _ => (
                        "Transfer failed",
                        format!(
                            "{} — {}",
                            delivery.label,
                            delivery.error.clone().unwrap_or_else(|| "unknown error".into())
                        ),
                    ),
                };
                self.notify(title, &body);
            }
            Event::DeviceDiscovered { device } if prefs.device_discovered && !focused => {
                self.notify("New device nearby", &format!("{} is available", device.name));
            }
            Event::DeviceDisconnected { name, device_id } if prefs.device_disconnected && !focused => {
                let trusted = engine
                    .device_view(device_id)
                    .ok()
                    .flatten()
                    .map(|d| d.trust == droplink_core::models::TrustState::Trusted)
                    .unwrap_or(false);
                if trusted {
                    self.notify("Device disconnected", &format!("{name} went offline"));
                }
            }
            _ => {}
        }
    }
}

impl EventSink for TauriSink {
    fn emit(&self, event: Event) {
        if let Err(e) = self.app.emit(EVENT_CHANNEL, &event) {
            tracing::warn!("failed to forward event: {e}");
        }
        self.maybe_notify(&event);
    }
}

pub fn format_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1000.0 && i < UNITS.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}
