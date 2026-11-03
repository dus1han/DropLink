use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::util;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Dark,
    Light,
}

/// What to do when an incoming file already exists at its destination.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConflictPolicy {
    /// Keep both: the incoming file is saved as `name (2).ext`.
    Rename,
    /// Replace the existing file.
    Overwrite,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct NotificationSettings {
    pub enabled: bool,
    pub incoming_requests: bool,
    pub transfer_completed: bool,
    pub transfer_failed: bool,
    pub device_discovered: bool,
    pub pairing_requests: bool,
    pub device_disconnected: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            incoming_requests: true,
            transfer_completed: true,
            transfer_failed: true,
            device_discovered: false,
            pairing_requests: true,
            device_disconnected: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    // General
    pub device_name: String,
    pub download_dir: PathBuf,
    pub theme: Theme,
    pub language: String,
    pub launch_at_login: bool,
    /// Closing the window keeps DropLink running in the system tray so it
    /// can still receive transfers.
    pub close_to_tray: bool,

    // Network
    pub discovery_enabled: bool,
    pub port: u16,
    /// Name of the network interface to advertise on; `None` = all.
    pub preferred_interface: Option<String>,
    /// Accept connections whose source address is outside private/LAN ranges
    /// (needed for transfers across networks via port forwarding or VPN).
    pub allow_remote_connections: bool,

    // Transfers
    /// Ask before accepting transfers from trusted devices that do not have
    /// auto-accept enabled. Untrusted devices can never send files.
    pub require_approval: bool,
    pub conflict_policy: ConflictPolicy,
    pub max_concurrent_transfers: u32,
    /// Upload limit in megabytes per second. 0 = unlimited.
    pub bandwidth_limit_mbps: f64,
    pub preserve_timestamps: bool,

    // Clipboard
    pub clipboard_auto_sync: bool,
    pub clipboard_images: bool,
    pub clipboard_history_enabled: bool,
    pub clipboard_history_limit: u32,

    // Storage
    pub history_retention_days: u32,

    pub notifications: NotificationSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            device_name: util::default_device_name(),
            download_dir: util::default_download_dir(),
            theme: Theme::System,
            language: "en".into(),
            launch_at_login: false,
            close_to_tray: true,
            discovery_enabled: true,
            port: crate::DEFAULT_PORT,
            preferred_interface: None,
            allow_remote_connections: false,
            require_approval: true,
            conflict_policy: ConflictPolicy::Rename,
            max_concurrent_transfers: 3,
            bandwidth_limit_mbps: 0.0,
            preserve_timestamps: true,
            clipboard_auto_sync: false,
            clipboard_images: true,
            clipboard_history_enabled: true,
            clipboard_history_limit: 50,
            history_retention_days: 90,
            notifications: NotificationSettings::default(),
        }
    }
}

impl Settings {
    /// Clamps values that came from the UI or an older database to sane ranges.
    pub fn normalized(mut self) -> Self {
        self.device_name = self.device_name.trim().chars().take(64).collect();
        if self.device_name.is_empty() {
            self.device_name = util::default_device_name();
        }
        if self.port < 1024 {
            self.port = crate::DEFAULT_PORT;
        }
        self.max_concurrent_transfers = self.max_concurrent_transfers.clamp(1, 16);
        if !self.bandwidth_limit_mbps.is_finite() || self.bandwidth_limit_mbps < 0.0 {
            self.bandwidth_limit_mbps = 0.0;
        }
        self.clipboard_history_limit = self.clipboard_history_limit.clamp(5, 500);
        self.history_retention_days = self.history_retention_days.clamp(1, 3650);
        if self.language.is_empty() {
            self.language = "en".into();
        }
        self
    }
}
