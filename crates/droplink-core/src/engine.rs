use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;

use parking_lot::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use crate::clipboard::{ClipboardBackend, ClipboardState};
use crate::db::Db;
use crate::discovery::Discovery;
use crate::events::{Event, EventSink};
use crate::identity::Identity;
use crate::models::*;
use crate::pairing::PendingPairing;
use crate::protocol::Purpose;
use crate::registry::{PresenceSource, Registry};
use crate::settings::Settings;
use crate::tls::{self, TlsConfigs};
use crate::transfer::{PendingIncoming, TransferState};
use crate::util::{fingerprint_display, now_ms};
use crate::{Error, Result};

pub enum ClipboardMode {
    /// The operating system clipboard.
    System,
    /// A caller-provided backend (tests use an in-memory clipboard).
    Custom(Arc<dyn ClipboardBackend>),
    Disabled,
}

pub struct EngineConfig {
    pub data_dir: PathBuf,
    pub app_version: String,
    /// Run mDNS advertising/browsing (disabled in tests).
    pub discovery: bool,
    pub clipboard: ClipboardMode,
    /// Overrides the port from settings; `Some(0)` picks a free port.
    pub port_override: Option<u16>,
    /// Bind to 127.0.0.1 only (tests).
    pub loopback_only: bool,
    /// Use an in-memory database and identity (tests).
    pub ephemeral: bool,
}

impl EngineConfig {
    pub fn new(data_dir: PathBuf, app_version: impl Into<String>) -> Self {
        Self {
            data_dir,
            app_version: app_version.into(),
            discovery: true,
            clipboard: ClipboardMode::System,
            port_override: None,
            loopback_only: false,
            ephemeral: false,
        }
    }
}

pub struct Inner {
    pub(crate) cfg: EngineConfig,
    pub(crate) identity: Identity,
    pub(crate) tls: TlsConfigs,
    pub(crate) db: Db,
    pub(crate) settings: RwLock<Settings>,
    pub(crate) sink: Arc<dyn EventSink>,
    pub(crate) registry: Registry,
    port: AtomicU16,
    listener_stop: Mutex<CancellationToken>,
    pub(crate) transfers: TransferState,
    pub(crate) pairings: Mutex<HashMap<String, PendingPairing>>,
    pub(crate) incoming: Mutex<HashMap<String, PendingIncoming>>,
    pub(crate) clipboard: ClipboardState,
    pub(crate) discovery: Mutex<Option<Discovery>>,
    /// Device IDs from mDNS whose DropLink handshake is being checked.
    pub(crate) verifying: Mutex<HashSet<String>>,
    pub(crate) shutdown: CancellationToken,
}

/// Cheaply clonable handle to the running engine.
#[derive(Clone)]
pub struct Engine(Arc<Inner>);

impl Deref for Engine {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}

impl Engine {
    pub async fn start(mut cfg: EngineConfig, sink: Arc<dyn EventSink>) -> Result<Engine> {
        let (identity, db) = if cfg.ephemeral {
            (Identity::ephemeral()?, Db::open_in_memory()?)
        } else {
            (
                Identity::load_or_create(&cfg.data_dir)?,
                Db::open(&cfg.data_dir.join("droplink.db"))?,
            )
        };
        let settings = db.load_settings()?;
        let tls = tls::build(&identity)?;
        let interrupted = db.interrupt_active_deliveries()?;
        if interrupted > 0 {
            tracing::info!(interrupted, "marked unfinished transfers as paused");
        }
        let _ = db.prune_history(settings.history_retention_days);

        let clipboard_backend = match std::mem::replace(&mut cfg.clipboard, ClipboardMode::Disabled) {
            ClipboardMode::System => crate::clipboard::system_backend(),
            ClipboardMode::Custom(b) => Some(b),
            ClipboardMode::Disabled => None,
        };

        let transfers = TransferState::new(&settings);
        let engine = Engine(Arc::new(Inner {
            identity,
            tls,
            db,
            settings: RwLock::new(settings.clone()),
            sink,
            registry: Registry::default(),
            port: AtomicU16::new(0),
            listener_stop: Mutex::new(CancellationToken::new()),
            transfers,
            pairings: Mutex::new(HashMap::new()),
            incoming: Mutex::new(HashMap::new()),
            clipboard: ClipboardState::new(clipboard_backend),
            discovery: Mutex::new(None),
            verifying: Mutex::new(HashSet::new()),
            shutdown: CancellationToken::new(),
            cfg,
        }));

        let port = engine.cfg.port_override.unwrap_or(settings.port);
        engine.start_listener(port).await?;
        if engine.cfg.discovery && settings.discovery_enabled {
            engine.start_discovery();
        }
        engine.start_clipboard_watcher();
        engine.spawn_housekeeping();
        tracing::info!(device_id = %engine.identity.device_id, port = engine.port(), "engine started");
        Ok(engine)
    }

    async fn start_listener(&self, port: u16) -> Result<()> {
        let listener = self.bind_listener(port).await?;
        let actual = listener.local_addr()?.port();
        self.port.store(actual, Ordering::SeqCst);
        let stop = CancellationToken::new();
        let old = std::mem::replace(&mut *self.listener_stop.lock(), stop.clone());
        old.cancel();
        self.spawn_listener(listener, stop);
        Ok(())
    }

    pub fn shutdown(&self) {
        self.shutdown.cancel();
        self.listener_stop.lock().cancel();
        self.transfers.cancel_all_for_shutdown();
        if let Some(d) = self.discovery.lock().take() {
            d.stop();
        }
    }

    pub fn port(&self) -> u16 {
        self.port.load(Ordering::SeqCst)
    }

    pub fn device_id(&self) -> &str {
        &self.identity.device_id
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().clone()
    }

    pub(crate) fn emit(&self, e: Event) {
        self.sink.emit(e);
    }

    // ---- devices ------------------------------------------------------

    pub fn local_device(&self) -> LocalDeviceView {
        let s = self.settings.read();
        LocalDeviceView {
            id: self.identity.device_id.clone(),
            name: s.device_name.clone(),
            os: crate::util::os_name().into(),
            port: self.port(),
            fingerprint: fingerprint_display(&self.identity.device_id),
            addresses: local_addresses().into_iter().map(|i| i.address).collect(),
            app_version: self.cfg.app_version.clone(),
            discovery_active: self.discovery.lock().is_some(),
        }
    }

    pub fn network_interfaces(&self) -> Vec<NetworkInterfaceView> {
        local_addresses()
    }

    pub(crate) fn trust_of(&self, id: &str) -> TrustState {
        self.db
            .get_device(id)
            .ok()
            .flatten()
            .map(|d| d.trust)
            .unwrap_or(TrustState::Unknown)
    }

    pub(crate) fn device_name(&self, id: &str) -> String {
        self.db
            .get_device(id)
            .ok()
            .flatten()
            .map(|d| d.display_name())
            .unwrap_or_else(|| format!("Device {}", crate::util::short_id(id)))
    }

    pub fn device_view(&self, id: &str) -> Result<Option<DeviceView>> {
        let Some(row) = self.db.get_device(id)? else { return Ok(None) };
        let presence = self.registry.get(id);
        let online = presence.as_ref().map(|p| p.online).unwrap_or(false);
        let addr = presence.as_ref().and_then(|p| p.best_addr());
        let connection_type = match presence.as_ref().map(|p| p.source) {
            Some(PresenceSource::Mdns) => ConnectionType::Lan,
            Some(_) if row.connection_type == ConnectionType::Manual => ConnectionType::Manual,
            Some(_) => {
                if addr.map(|a| crate::util::is_local_ip(a.ip())).unwrap_or(true) {
                    ConnectionType::Lan
                } else {
                    ConnectionType::Manual
                }
            }
            None => row.connection_type,
        };
        Ok(Some(DeviceView {
            name: row.display_name(),
            reported_name: row.name.clone(),
            alias: row.alias.clone(),
            os: row.os.clone(),
            app_version: row.app_version.clone(),
            online,
            address: addr.map(|a| a.ip().to_string()).or(row.last_address.clone()),
            port: addr.map(|a| a.port()).or(row.last_port),
            last_seen: presence.map(|p| p.last_seen).unwrap_or(row.last_seen).max(row.last_seen),
            connection_type,
            trust: row.trust,
            paired_at: row.paired_at,
            auto_accept: row.auto_accept,
            clipboard_sync: row.clipboard_sync,
            group_ids: self.db.group_ids_for_device(id)?,
            fingerprint: fingerprint_display(&row.id),
            id: row.id,
        }))
    }

    pub fn list_devices(&self) -> Result<Vec<DeviceView>> {
        let rows = self.db.list_devices()?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            if let Some(v) = self.device_view(&r.id)? {
                out.push(v);
            }
        }
        // Online first, then trusted, then most recently seen.
        out.sort_by(|a, b| {
            b.online
                .cmp(&a.online)
                .then((b.trust == TrustState::Trusted).cmp(&(a.trust == TrustState::Trusted)))
                .then(b.last_seen.cmp(&a.last_seen))
        });
        Ok(out)
    }

    pub(crate) fn emit_device(&self, id: &str) {
        if let Ok(Some(device)) = self.device_view(id) {
            self.emit(Event::DeviceUpdated { device });
        }
    }

    /// Called whenever a device is authenticated or (re)discovered.
    pub(crate) fn on_device_seen(&self, id: &str, is_new: bool) {
        match self.device_view(id) {
            Ok(Some(device)) => {
                if is_new {
                    self.emit(Event::DeviceDiscovered { device: device.clone() });
                }
                self.emit(Event::DeviceUpdated { device });
            }
            _ => return,
        }
        self.resume_waiting_for(id);
    }

    pub(crate) fn on_device_gone(&self, id: &str) {
        if self.registry.mark_offline(id) {
            let name = self.device_name(id);
            self.emit(Event::DeviceDisconnected { device_id: id.into(), name });
            self.emit_device(id);
        }
    }

    pub async fn connect_manual(&self, address: &str) -> Result<DeviceView> {
        let address = address.trim();
        let with_port = if address.contains(':') && !address.starts_with('[') && address.matches(':').count() > 1 {
            // Bare IPv6 address without a port.
            format!("[{address}]:{}", crate::DEFAULT_PORT)
        } else if address.rsplit_once(':').map(|(_, p)| p.parse::<u16>().is_ok()).unwrap_or(false) {
            address.to_string()
        } else {
            format!("{address}:{}", crate::DEFAULT_PORT)
        };
        let addrs: Vec<_> = tokio::net::lookup_host(&with_port)
            .await
            .map_err(|_| Error::Invalid(format!("could not resolve {address}")))?
            .collect();
        let mut last = Error::Unreachable;
        for addr in addrs {
            match self.connect_addr(addr, None, Purpose::Probe).await {
                Ok(conn) => {
                    let id = conn.peer.id.clone();
                    self.db.set_manual_address(&id, &with_port)?;
                    self.registry.mark_online(&id, vec![addr.ip()], addr.port(), PresenceSource::Manual, None);
                    self.emit_device(&id);
                    return self.device_view(&id)?.ok_or(Error::DeviceNotFound);
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// Checks whether a device is reachable right now.
    pub async fn probe(&self, id: &str) -> Result<bool> {
        let ok = self.connect_device(id, Purpose::Probe).await.is_ok();
        if !ok {
            self.on_device_gone(id);
        }
        Ok(ok)
    }

    pub fn rename_device(&self, id: &str, alias: Option<String>) -> Result<()> {
        let alias = alias.map(|a| a.trim().chars().take(64).collect::<String>()).filter(|a| !a.is_empty());
        self.db.set_alias(id, alias.as_deref())?;
        self.emit_device(id);
        Ok(())
    }

    pub fn set_device_permissions(
        &self,
        id: &str,
        auto_accept: Option<bool>,
        clipboard_sync: Option<bool>,
    ) -> Result<()> {
        if self.trust_of(id) != TrustState::Trusted {
            return Err(Error::NotTrusted);
        }
        self.db.set_device_permissions(id, auto_accept, clipboard_sync)?;
        let name = self.device_name(id);
        if let Some(v) = auto_accept {
            self.record_security(
                "permission_changed",
                Some(id),
                &format!("Auto-accept {} for {name}", if v { "enabled" } else { "disabled" }),
            );
        }
        self.emit_device(id);
        Ok(())
    }

    /// Revokes trust. The device can be paired again later.
    pub fn revoke_device(&self, id: &str) -> Result<()> {
        self.db.clear_trust(id)?;
        self.transfers.cancel_for_device(id);
        self.record_security("revoked", Some(id), "Trust revoked");
        self.emit_device(id);
        Ok(())
    }

    pub fn block_device(&self, id: &str) -> Result<()> {
        if self.db.get_device(id)?.is_none() {
            return Err(Error::DeviceNotFound);
        }
        self.db.set_blocked(id)?;
        self.transfers.cancel_for_device(id);
        self.record_security("blocked", Some(id), "Device blocked");
        self.emit_device(id);
        Ok(())
    }

    pub fn unblock_device(&self, id: &str) -> Result<()> {
        self.db.clear_trust(id)?;
        self.record_security("unblocked", Some(id), "Device unblocked");
        self.emit_device(id);
        Ok(())
    }

    /// Removes every trace of a device except history.
    pub fn forget_device(&self, id: &str) -> Result<()> {
        let name = self.device_name(id);
        self.transfers.cancel_for_device(id);
        self.db.delete_device(id)?;
        self.registry.remove(id);
        let _ = self.db.insert_security_event("forgotten", Some(id), Some(&name), "Device forgotten");
        self.emit(Event::DeviceRemoved { device_id: id.into() });
        Ok(())
    }

    // ---- groups -------------------------------------------------------

    pub fn list_groups(&self) -> Result<Vec<GroupView>> {
        self.db.list_groups()
    }

    pub fn create_group(&self, name: &str, color: &str, members: &[String]) -> Result<GroupView> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("group name is required".into()));
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.db.create_group(&id, name, color).map_err(|e| match e {
            Error::Db(rusqlite::Error::SqliteFailure(f, _)) if f.code == rusqlite::ErrorCode::ConstraintViolation => {
                Error::Invalid(format!("a group named \"{name}\" already exists"))
            }
            e => e,
        })?;
        self.db.set_group_members(&id, members)?;
        self.emit(Event::GroupsChanged);
        self.db
            .list_groups()?
            .into_iter()
            .find(|g| g.id == id)
            .ok_or(Error::Other("group vanished".into()))
    }

    pub fn update_group(
        &self,
        id: &str,
        name: Option<String>,
        color: Option<String>,
        auto_accept: Option<bool>,
        clipboard_sync: Option<bool>,
        members: Option<Vec<String>>,
    ) -> Result<()> {
        let name = name.map(|n| n.trim().to_string());
        if name.as_deref() == Some("") {
            return Err(Error::Invalid("group name is required".into()));
        }
        self.db.update_group(id, name.as_deref(), color.as_deref(), auto_accept, clipboard_sync)?;
        if let Some(m) = members {
            self.db.set_group_members(id, &m)?;
        }
        if auto_accept.is_some() || clipboard_sync.is_some() {
            self.record_security("group_permissions", None, &format!("Permissions changed for group {id}"));
        }
        self.emit(Event::GroupsChanged);
        Ok(())
    }

    pub fn delete_group(&self, id: &str) -> Result<()> {
        self.db.delete_group(id)?;
        self.emit(Event::GroupsChanged);
        Ok(())
    }

    /// Trusted members of a group — the only devices a group send reaches.
    pub fn group_recipients(&self, group_id: &str) -> Result<Vec<String>> {
        Ok(self
            .db
            .group_member_ids(group_id)?
            .into_iter()
            .filter(|id| self.trust_of(id) == TrustState::Trusted)
            .collect())
    }

    // ---- settings -----------------------------------------------------

    pub async fn update_settings(&self, new: Settings) -> Result<Settings> {
        let new = new.normalized();
        let old = self.settings();
        self.db.save_settings(&new)?;
        *self.settings.write() = new.clone();

        self.transfers.apply_settings(&new);
        if new.port != old.port && self.cfg.port_override.is_none() {
            self.start_listener(new.port).await?;
        }
        let discovery_changed = new.discovery_enabled != old.discovery_enabled
            || new.preferred_interface != old.preferred_interface
            || new.port != old.port
            || new.device_name != old.device_name;
        if self.cfg.discovery && discovery_changed {
            self.stop_discovery();
            if new.discovery_enabled {
                self.start_discovery();
            }
        }
        if new.device_name != old.device_name || new.port != old.port {
            self.emit(Event::LocalDeviceUpdated { device: self.local_device() });
        }
        if !new.clipboard_history_enabled && old.clipboard_history_enabled {
            self.db.clear_clips()?;
        }
        Ok(new)
    }

    pub(crate) fn start_discovery(&self) {
        let mut slot = self.discovery.lock();
        if slot.is_some() {
            return;
        }
        match Discovery::start(self.clone()) {
            Ok(d) => *slot = Some(d),
            Err(e) => tracing::warn!("mDNS discovery unavailable: {e}"),
        }
        drop(slot);
        self.emit(Event::LocalDeviceUpdated { device: self.local_device() });
    }

    pub(crate) fn stop_discovery(&self) {
        if let Some(d) = self.discovery.lock().take() {
            d.stop();
        }
        for id in self.registry.drop_mdns() {
            self.emit_device(&id);
        }
        self.emit(Event::LocalDeviceUpdated { device: self.local_device() });
    }

    // ---- security & stats ---------------------------------------------

    pub(crate) fn record_security(&self, kind: &str, device_id: Option<&str>, detail: &str) {
        let name = device_id.map(|id| self.device_name(id));
        match self.db.insert_security_event(kind, device_id, name.as_deref(), detail) {
            Ok(event) => self.emit(Event::SecurityEvent { event }),
            Err(e) => tracing::warn!("failed to record security event: {e}"),
        }
    }

    pub fn security_events(&self, limit: u32) -> Result<Vec<SecurityEventView>> {
        self.db.list_security_events(limit)
    }

    pub fn history(&self, q: &HistoryQuery) -> Result<Vec<HistoryEntry>> {
        self.db.query_history(q)
    }

    pub fn clear_history(&self) -> Result<usize> {
        self.db.clear_history()
    }

    pub fn stats(&self, start_of_day_ms: i64) -> Result<StatsView> {
        let mut s = self.db.stats(start_of_day_ms)?;
        s.online_devices = self.registry.online_ids().len() as u32;
        s.active_transfers = self.db.active_delivery_count()?;
        Ok(s)
    }

    fn spawn_housekeeping(&self) {
        let engine = self.clone();
        tokio::spawn(async move {
            // Give startup a moment before touching the disk.
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            if let Err(e) = engine.expire_stale_partials(7 * 86_400_000) {
                tracing::warn!("partial cleanup failed: {e}");
            }
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(6 * 3600));
            loop {
                tokio::select! {
                    _ = tick.tick() => {
                        let days = engine.settings.read().history_retention_days;
                        let _ = engine.db.prune_history(days);
                    }
                    _ = engine.shutdown.cancelled() => break,
                }
            }
        });
    }
}

pub(crate) fn local_addresses() -> Vec<NetworkInterfaceView> {
    if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_loopback() && i.ip().is_ipv4())
        .map(|i| NetworkInterfaceView { address: i.ip().to_string(), name: i.name })
        .collect()
}

pub fn start_of_day_ms(offset_minutes: i32) -> i64 {
    let now = now_ms() + offset_minutes as i64 * 60_000;
    now - now.rem_euclid(86_400_000) - offset_minutes as i64 * 60_000
}
