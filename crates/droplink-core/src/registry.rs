//! In-memory presence table: which devices are reachable right now and at
//! which address. Persistent facts (names, trust) live in SQLite; this only
//! tracks what changes from moment to moment. Keyed by device ID, so the
//! same device seen over mDNS, a manual address and an inbound connection is
//! always a single entry.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};

use parking_lot::RwLock;

use crate::util::now_ms;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceSource {
    Mdns,
    Manual,
    Inbound,
}

#[derive(Debug, Clone)]
pub struct Presence {
    pub addresses: Vec<IpAddr>,
    pub port: u16,
    pub online: bool,
    pub last_seen: i64,
    pub source: PresenceSource,
    /// mDNS full service name, used to match removal events.
    pub mdns_name: Option<String>,
}

impl Presence {
    pub fn best_addr(&self) -> Option<SocketAddr> {
        // Prefer IPv4: link-local IPv6 needs a scope id that mDNS records do
        // not carry reliably across platforms.
        self.addresses
            .iter()
            .find(|a| a.is_ipv4())
            .or_else(|| self.addresses.first())
            .map(|ip| SocketAddr::new(*ip, self.port))
    }
}

#[derive(Default)]
pub struct Registry {
    map: RwLock<HashMap<String, Presence>>,
}

/// What changed as a result of an update; drives event emission.
#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    /// Device was offline (or unknown) and is now online.
    CameOnline,
    /// Already online; details may have changed.
    Refreshed,
}

impl Registry {
    pub fn mark_online(
        &self,
        id: &str,
        addresses: Vec<IpAddr>,
        port: u16,
        source: PresenceSource,
        mdns_name: Option<String>,
    ) -> Change {
        let mut map = self.map.write();
        let now = now_ms();
        match map.get_mut(id) {
            Some(p) => {
                let was_online = p.online;
                if !addresses.is_empty() {
                    // mDNS knows every interface address; an inbound
                    // connection only knows one, so merge rather than replace.
                    if source == PresenceSource::Mdns || !p.online {
                        p.addresses = addresses;
                    } else {
                        for a in addresses {
                            if !p.addresses.contains(&a) {
                                p.addresses.push(a);
                            }
                        }
                    }
                }
                if port != 0 {
                    p.port = port;
                }
                p.online = true;
                p.last_seen = now;
                if source == PresenceSource::Mdns || !was_online {
                    p.source = source;
                }
                if mdns_name.is_some() {
                    p.mdns_name = mdns_name;
                }
                if was_online {
                    Change::Refreshed
                } else {
                    Change::CameOnline
                }
            }
            None => {
                map.insert(
                    id.to_string(),
                    Presence { addresses, port, online: true, last_seen: now, source, mdns_name },
                );
                Change::CameOnline
            }
        }
    }

    /// Returns `true` if the device was online before.
    pub fn mark_offline(&self, id: &str) -> bool {
        let mut map = self.map.write();
        match map.get_mut(id) {
            Some(p) if p.online => {
                p.online = false;
                true
            }
            _ => false,
        }
    }

    pub fn id_for_mdns_name(&self, name: &str) -> Option<String> {
        self.map
            .read()
            .iter()
            .find(|(_, p)| p.mdns_name.as_deref() == Some(name))
            .map(|(id, _)| id.clone())
    }

    pub fn get(&self, id: &str) -> Option<Presence> {
        self.map.read().get(id).cloned()
    }

    pub fn is_online(&self, id: &str) -> bool {
        self.map.read().get(id).map(|p| p.online).unwrap_or(false)
    }

    pub fn online_ids(&self) -> Vec<String> {
        self.map
            .read()
            .iter()
            .filter(|(_, p)| p.online)
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn remove(&self, id: &str) {
        self.map.write().remove(id);
    }

    /// Marks every mDNS-sourced device offline (discovery was turned off).
    pub fn drop_mdns(&self) -> Vec<String> {
        let mut map = self.map.write();
        let mut changed = Vec::new();
        for (id, p) in map.iter_mut() {
            if p.online && p.source == PresenceSource::Mdns {
                p.online = false;
                changed.push(id.clone());
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_announcements_collapse_to_one_entry() {
        let r = Registry::default();
        let ip: IpAddr = "192.168.1.10".parse().unwrap();
        assert_eq!(r.mark_online("a", vec![ip], 1, PresenceSource::Mdns, Some("x".into())), Change::CameOnline);
        assert_eq!(r.mark_online("a", vec![ip], 1, PresenceSource::Mdns, None), Change::Refreshed);
        assert_eq!(r.online_ids().len(), 1);
        assert_eq!(r.id_for_mdns_name("x").as_deref(), Some("a"));

        assert!(r.mark_offline("a"));
        assert!(!r.mark_offline("a"), "second offline is not a change");
        assert_eq!(r.mark_online("a", vec![], 0, PresenceSource::Inbound, None), Change::CameOnline);
        // Port and addresses survive an update that does not carry them.
        assert_eq!(r.get("a").unwrap().best_addr(), Some(SocketAddr::new(ip, 1)));
    }

    #[test]
    fn prefers_ipv4() {
        let p = Presence {
            addresses: vec!["fe80::1".parse().unwrap(), "10.0.0.5".parse().unwrap()],
            port: 9,
            online: true,
            last_seen: 0,
            source: PresenceSource::Mdns,
            mdns_name: None,
        };
        assert_eq!(p.best_addr().unwrap().ip().to_string(), "10.0.0.5");
    }
}
