//! LAN discovery over mDNS / DNS-SD (`_droplink._tcp.local.`).
//!
//! The TXT record carries the device ID, name and OS. Those claims are only
//! used for display: a device's identity is proven by its TLS certificate
//! when we connect, and `connect_device` rejects a peer whose certificate
//! does not match the advertised ID.

use std::net::IpAddr;

use mdns_sd::{IfKind, ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::db::SeenDevice;
use crate::engine::Engine;
use crate::protocol::Purpose;
use crate::registry::PresenceSource;
use crate::util::short_id;
use crate::{Error, Result, PROTOCOL_VERSION, SERVICE_TYPE};

pub struct Discovery {
    daemon: ServiceDaemon,
    fullname: String,
}

fn mdns_err(e: mdns_sd::Error) -> Error {
    Error::Other(format!("mDNS: {e}"))
}

impl Discovery {
    pub fn start(engine: Engine) -> Result<Self> {
        let daemon = ServiceDaemon::new().map_err(mdns_err)?;
        let settings = engine.settings();
        if let Some(iface) = settings.preferred_interface.as_deref().filter(|s| !s.is_empty()) {
            daemon.disable_interface(IfKind::All).map_err(mdns_err)?;
            daemon.enable_interface(IfKind::Name(iface.to_string())).map_err(mdns_err)?;
        }

        let id = engine.device_id().to_string();
        let instance = format!("droplink-{}", short_id(&id));
        let host = format!("{instance}.local.");
        let version = PROTOCOL_VERSION.to_string();
        let props = [
            ("id", id.as_str()),
            ("name", settings.device_name.as_str()),
            ("os", crate::util::os_name()),
            ("pv", version.as_str()),
        ];
        let info = ServiceInfo::new(SERVICE_TYPE, &instance, &host, "", engine.port(), &props[..])
            .map_err(mdns_err)?
            .enable_addr_auto();
        let fullname = info.get_fullname().to_string();
        daemon.register(info).map_err(mdns_err)?;

        let receiver = daemon.browse(SERVICE_TYPE).map_err(mdns_err)?;
        let stop = engine.shutdown.clone();
        tokio::spawn(async move {
            loop {
                let event = tokio::select! {
                    e = receiver.recv_async() => match e {
                        Ok(e) => e,
                        Err(_) => break,
                    },
                    _ = stop.cancelled() => break,
                };
                match event {
                    ServiceEvent::ServiceResolved(info) => engine.on_mdns_resolved(&info),
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        if let Some(id) = engine.registry.id_for_mdns_name(&fullname) {
                            engine.on_device_gone(&id);
                        }
                    }
                    _ => {}
                }
            }
            tracing::debug!("mDNS browse loop ended");
        });
        tracing::info!(%fullname, "advertising on mDNS");
        Ok(Self { daemon, fullname })
    }

    pub fn stop(self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

impl Engine {
    fn on_mdns_resolved(&self, info: &ServiceInfo) {
        let Some(id) = info.get_property_val_str("id").map(str::to_string) else { return };
        if id == self.identity.device_id || id.len() != 64 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
            return;
        }
        // An mDNS record is only a claim. A device is listed only after it has
        // proven it runs DropLink: a TLS handshake with the certificate whose
        // fingerprint is the advertised ID, followed by a DropLink hello.
        if !self.registry.is_online(&id) {
            if !self.verifying.lock().insert(id.clone()) {
                return;
            }
            let engine = self.clone();
            let info = info.clone();
            tokio::spawn(async move {
                let port = info.get_port();
                let mut verified = false;
                for ip in info.get_addresses().iter().copied() {
                    let addr = std::net::SocketAddr::new(ip, port);
                    match engine.connect_addr(addr, Some(&id), Purpose::Probe).await {
                        Ok(_) => {
                            verified = true;
                            break;
                        }
                        Err(e) => tracing::debug!(%addr, "not listing mDNS record: {e}"),
                    }
                }
                engine.verifying.lock().remove(&id);
                if verified {
                    engine.record_mdns_presence(&id, &info);
                }
            });
            return;
        }
        self.record_mdns_presence(&id, info);
    }

    fn record_mdns_presence(&self, id: &str, info: &ServiceInfo) {
        let id = id.to_string();
        let name = info.get_property_val_str("name").unwrap_or("DropLink device");
        let name: String = name.chars().take(64).collect();
        let os = info.get_property_val_str("os").unwrap_or("unknown");
        let mut addrs: Vec<IpAddr> = info.get_addresses().iter().copied().collect();
        addrs.sort_by_key(|a| (!a.is_ipv4(), a.to_string()));
        let best = addrs.iter().find(|a| a.is_ipv4()).or(addrs.first()).map(|a| a.to_string());
        let is_new = self
            .db
            .upsert_device_seen(&SeenDevice {
                id: &id,
                name: &name,
                os,
                app_version: None,
                address: best.as_deref(),
                port: Some(info.get_port()),
            })
            .unwrap_or(false);
        self.registry.mark_online(
            &id,
            addrs,
            info.get_port(),
            PresenceSource::Mdns,
            Some(info.get_fullname().to_string()),
        );
        self.on_device_seen(&id, is_new);
    }
}
