//! Types shared with the frontend. Every struct serialises to camelCase and is
//! mirrored by hand in `src/lib/types.ts`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TrustState {
    Unknown,
    Trusted,
    Blocked,
}

impl TrustState {
    pub fn as_str(self) -> &'static str {
        match self {
            TrustState::Unknown => "unknown",
            TrustState::Trusted => "trusted",
            TrustState::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionType {
    /// Found through mDNS on the local network.
    Lan,
    /// Reached through an address the user typed in.
    Manual,
}

impl ConnectionType {
    pub fn as_str(self) -> &'static str {
        match self {
            ConnectionType::Lan => "lan",
            ConnectionType::Manual => "manual",
        }
    }
    pub fn parse(s: &str) -> Self {
        if s == "manual" {
            ConnectionType::Manual
        } else {
            ConnectionType::Lan
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub id: String,
    /// Display name: the local alias if set, otherwise the advertised name.
    pub name: String,
    pub reported_name: String,
    pub alias: Option<String>,
    pub os: String,
    pub app_version: Option<String>,
    pub online: bool,
    pub address: Option<String>,
    pub port: Option<u16>,
    pub last_seen: i64,
    pub connection_type: ConnectionType,
    pub trust: TrustState,
    pub paired_at: Option<i64>,
    pub auto_accept: bool,
    pub clipboard_sync: bool,
    pub group_ids: Vec<String>,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDeviceView {
    pub id: String,
    pub name: String,
    pub os: String,
    pub port: u16,
    pub fingerprint: String,
    pub addresses: Vec<String>,
    pub app_version: String,
    pub discovery_active: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Outgoing,
    Incoming,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Outgoing => "outgoing",
            Direction::Incoming => "incoming",
        }
    }
    pub fn parse(s: &str) -> Self {
        if s == "incoming" {
            Direction::Incoming
        } else {
            Direction::Outgoing
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryStatus {
    Queued,
    Connecting,
    /// Waiting for the receiving user to accept.
    Waiting,
    Transferring,
    Paused,
    Completed,
    Failed,
    Cancelled,
    Rejected,
}

impl DeliveryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            DeliveryStatus::Queued => "queued",
            DeliveryStatus::Connecting => "connecting",
            DeliveryStatus::Waiting => "waiting",
            DeliveryStatus::Transferring => "transferring",
            DeliveryStatus::Paused => "paused",
            DeliveryStatus::Completed => "completed",
            DeliveryStatus::Failed => "failed",
            DeliveryStatus::Cancelled => "cancelled",
            DeliveryStatus::Rejected => "rejected",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "queued" => DeliveryStatus::Queued,
            "connecting" => DeliveryStatus::Connecting,
            "waiting" => DeliveryStatus::Waiting,
            "transferring" => DeliveryStatus::Transferring,
            "paused" => DeliveryStatus::Paused,
            "completed" => DeliveryStatus::Completed,
            "cancelled" => DeliveryStatus::Cancelled,
            "rejected" => DeliveryStatus::Rejected,
            _ => DeliveryStatus::Failed,
        }
    }
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            DeliveryStatus::Completed
                | DeliveryStatus::Failed
                | DeliveryStatus::Cancelled
                | DeliveryStatus::Rejected
        )
    }
    pub fn is_active(self) -> bool {
        matches!(
            self,
            DeliveryStatus::Queued
                | DeliveryStatus::Connecting
                | DeliveryStatus::Waiting
                | DeliveryStatus::Transferring
        )
    }
}

/// One transfer to (or from) one device. A send to three devices creates one
/// session with three deliveries, each with independent progress and retry.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryView {
    pub id: String,
    pub session_id: String,
    pub direction: Direction,
    pub label: String,
    pub device_id: String,
    pub device_name: String,
    pub device_os: String,
    pub status: DeliveryStatus,
    pub bytes_done: u64,
    pub total_bytes: u64,
    pub files_done: u32,
    pub total_files: u32,
    pub error: Option<String>,
    pub verified: Option<bool>,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub dest_dir: Option<String>,
    pub first_file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressView {
    pub delivery_id: String,
    pub bytes_done: u64,
    pub total_bytes: u64,
    pub files_done: u32,
    pub total_files: u32,
    pub speed_bps: f64,
    pub eta_seconds: Option<f64>,
    pub current_file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfferItemPreview {
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingRequestView {
    pub id: String,
    pub device_id: String,
    pub device_name: String,
    pub device_os: String,
    pub label: String,
    pub total_bytes: u64,
    pub total_files: u32,
    pub items: Vec<OfferItemPreview>,
    /// Number of top-level entries that already exist in the destination.
    pub conflicts: u32,
    pub dest_dir: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub delivery_id: String,
    pub direction: Direction,
    pub kind: String,
    pub name: String,
    pub total_bytes: u64,
    pub file_count: u32,
    pub sender_id: String,
    pub sender_name: String,
    pub recipient_id: String,
    pub recipient_name: String,
    pub status: DeliveryStatus,
    pub error: Option<String>,
    pub verified: Option<bool>,
    pub started_at: Option<i64>,
    pub finished_at: i64,
    pub duration_ms: Option<i64>,
    pub retryable: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HistoryQuery {
    pub search: Option<String>,
    pub status: Option<String>,
    pub device_id: Option<String>,
    pub direction: Option<String>,
    pub from_ms: Option<i64>,
    pub to_ms: Option<i64>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextMessageView {
    pub id: String,
    pub direction: Direction,
    pub device_id: String,
    pub device_name: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardEntryView {
    pub id: i64,
    /// `local` = copied on this machine, `remote` = received from a device.
    pub origin: String,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub kind: String,
    pub text: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub byte_size: u64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupView {
    pub id: String,
    pub name: String,
    pub color: String,
    pub auto_accept: bool,
    pub clipboard_sync: bool,
    pub member_ids: Vec<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityEventView {
    pub id: i64,
    pub kind: String,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub detail: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StatsView {
    pub known_devices: u32,
    pub trusted_devices: u32,
    pub online_devices: u32,
    pub active_transfers: u32,
    pub files_today: u64,
    pub bytes_today: u64,
    pub bytes_total: u64,
    pub transfers_total: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInterfaceView {
    pub name: String,
    pub address: String,
}
