use serde::Serialize;

use crate::models::*;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PairingRole {
    /// This device started the pairing.
    Initiator,
    /// Another device asked to pair with this one.
    Responder,
}

/// Everything the engine reports to the UI. The Tauri layer forwards each
/// event on the `droplink://event` channel and turns some of them into native
/// notifications.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Event {
    /// A device appeared for the first time.
    DeviceDiscovered { device: DeviceView },
    /// Presence, trust or naming of a device changed.
    DeviceUpdated { device: DeviceView },
    /// A known device went offline.
    DeviceDisconnected { device_id: String, name: String },
    /// A device was forgotten and should disappear from lists.
    DeviceRemoved { device_id: String },
    LocalDeviceUpdated { device: LocalDeviceView },

    PairingCode {
        pairing_id: String,
        device_id: String,
        device_name: String,
        device_os: String,
        code: String,
        role: PairingRole,
    },
    PairingFinished {
        pairing_id: String,
        device_id: String,
        device_name: String,
        success: bool,
        message: String,
    },

    IncomingRequest { request: IncomingRequestView },
    IncomingRequestResolved { request_id: String },

    DeliveryUpdated { delivery: DeliveryView },
    DeliveryProgress { progress: ProgressView },

    TextReceived { message: TextMessageView },
    ClipboardChanged { entry: ClipboardEntryView },

    /// A security-relevant event was recorded.
    SecurityEvent { event: SecurityEventView },
    GroupsChanged,
}

pub trait EventSink: Send + Sync + 'static {
    fn emit(&self, event: Event);
}

/// Sink that drops everything; handy for tools and tests that do not care.
pub struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _event: Event) {}
}

/// Sink backed by an unbounded channel, used by integration tests.
pub struct ChannelSink(pub tokio::sync::mpsc::UnboundedSender<Event>);

impl EventSink for ChannelSink {
    fn emit(&self, event: Event) {
        let _ = self.0.send(event);
    }
}
