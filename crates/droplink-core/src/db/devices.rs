use rusqlite::{params, OptionalExtension, Row};

use super::Db;
use crate::models::{ConnectionType, GroupView, TrustState};
use crate::util::now_ms;
use crate::Result;

#[derive(Debug, Clone)]
pub struct DeviceRow {
    pub id: String,
    pub name: String,
    pub alias: Option<String>,
    pub os: String,
    pub app_version: Option<String>,
    pub last_address: Option<String>,
    pub last_port: Option<u16>,
    pub connection_type: ConnectionType,
    pub manual_address: Option<String>,
    pub first_seen: i64,
    pub last_seen: i64,
    pub trust: TrustState,
    pub paired_at: Option<i64>,
    pub auto_accept: bool,
    pub clipboard_sync: bool,
}

impl DeviceRow {
    pub fn display_name(&self) -> String {
        self.alias.clone().unwrap_or_else(|| self.name.clone())
    }
}

const DEVICE_SELECT: &str = "SELECT d.id, d.name, d.alias, d.os, d.app_version, d.last_address, d.last_port,
        d.connection_type, d.manual_address, d.first_seen, d.last_seen,
        t.status, t.paired_at, COALESCE(t.auto_accept, 0), COALESCE(t.clipboard_sync, 1)
    FROM devices d LEFT JOIN trusted_devices t ON t.device_id = d.id";

fn device_from_row(r: &Row<'_>) -> rusqlite::Result<DeviceRow> {
    let status: Option<String> = r.get(11)?;
    Ok(DeviceRow {
        id: r.get(0)?,
        name: r.get(1)?,
        alias: r.get(2)?,
        os: r.get(3)?,
        app_version: r.get(4)?,
        last_address: r.get(5)?,
        last_port: r.get::<_, Option<i64>>(6)?.map(|p| p as u16),
        connection_type: ConnectionType::parse(&r.get::<_, String>(7)?),
        manual_address: r.get(8)?,
        first_seen: r.get(9)?,
        last_seen: r.get(10)?,
        trust: match status.as_deref() {
            Some("trusted") => TrustState::Trusted,
            Some("blocked") => TrustState::Blocked,
            _ => TrustState::Unknown,
        },
        paired_at: r.get(12)?,
        auto_accept: r.get::<_, i64>(13)? != 0,
        clipboard_sync: r.get::<_, i64>(14)? != 0,
    })
}

pub struct SeenDevice<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub os: &'a str,
    pub app_version: Option<&'a str>,
    pub address: Option<&'a str>,
    pub port: Option<u16>,
}

impl Db {
    /// Records that a device was seen. Returns `true` if it was new.
    pub fn upsert_device_seen(&self, d: &SeenDevice<'_>) -> Result<bool> {
        let now = now_ms();
        self.with(|c| {
            let existed: bool = c
                .query_row("SELECT 1 FROM devices WHERE id = ?1", [d.id], |_| Ok(()))
                .optional()?
                .is_some();
            c.execute(
                "INSERT INTO devices (id, name, os, app_version, last_address, last_port, first_seen, last_seen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    os = excluded.os,
                    app_version = COALESCE(excluded.app_version, devices.app_version),
                    last_address = COALESCE(excluded.last_address, devices.last_address),
                    last_port = COALESCE(excluded.last_port, devices.last_port),
                    last_seen = excluded.last_seen",
                params![d.id, d.name, d.os, d.app_version, d.address, d.port.map(|p| p as i64), now],
            )?;
            Ok(!existed)
        })
    }

    pub fn set_manual_address(&self, id: &str, address: &str) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE devices SET connection_type = 'manual', manual_address = ?2 WHERE id = ?1",
                params![id, address],
            )
        })?;
        Ok(())
    }

    pub fn touch_device(&self, id: &str) -> Result<()> {
        self.with(|c| c.execute("UPDATE devices SET last_seen = ?2 WHERE id = ?1", params![id, now_ms()]))?;
        Ok(())
    }

    pub fn get_device(&self, id: &str) -> Result<Option<DeviceRow>> {
        self.with(|c| {
            c.query_row(&format!("{DEVICE_SELECT} WHERE d.id = ?1"), [id], device_from_row)
                .optional()
        })
    }

    pub fn list_devices(&self) -> Result<Vec<DeviceRow>> {
        self.with(|c| {
            let mut stmt = c.prepare(&format!("{DEVICE_SELECT} ORDER BY d.last_seen DESC"))?;
            let rows = stmt.query_map([], device_from_row)?;
            rows.collect()
        })
    }

    pub fn set_alias(&self, id: &str, alias: Option<&str>) -> Result<()> {
        self.with(|c| c.execute("UPDATE devices SET alias = ?2 WHERE id = ?1", params![id, alias]))?;
        Ok(())
    }

    pub fn set_trusted(&self, id: &str) -> Result<()> {
        let now = now_ms();
        self.with(|c| {
            c.execute(
                "INSERT INTO trusted_devices (device_id, status, paired_at, updated_at)
                 VALUES (?1, 'trusted', ?2, ?2)
                 ON CONFLICT(device_id) DO UPDATE SET status = 'trusted', paired_at = ?2, updated_at = ?2",
                params![id, now],
            )
        })?;
        Ok(())
    }

    pub fn set_blocked(&self, id: &str) -> Result<()> {
        let now = now_ms();
        self.with(|c| {
            c.execute(
                "INSERT INTO trusted_devices (device_id, status, updated_at, auto_accept, clipboard_sync)
                 VALUES (?1, 'blocked', ?2, 0, 0)
                 ON CONFLICT(device_id) DO UPDATE SET status = 'blocked', auto_accept = 0,
                    clipboard_sync = 0, updated_at = ?2",
                params![id, now],
            )
        })?;
        Ok(())
    }

    /// Removes trust (or a block); the device becomes unknown again.
    pub fn clear_trust(&self, id: &str) -> Result<()> {
        self.with(|c| c.execute("DELETE FROM trusted_devices WHERE device_id = ?1", [id]))?;
        Ok(())
    }

    pub fn delete_device(&self, id: &str) -> Result<()> {
        self.with(|c| c.execute("DELETE FROM devices WHERE id = ?1", [id]))?;
        Ok(())
    }

    pub fn set_device_permissions(
        &self,
        id: &str,
        auto_accept: Option<bool>,
        clipboard_sync: Option<bool>,
    ) -> Result<()> {
        self.with(|c| {
            if let Some(v) = auto_accept {
                c.execute(
                    "UPDATE trusted_devices SET auto_accept = ?2, updated_at = ?3 WHERE device_id = ?1 AND status = 'trusted'",
                    params![id, v as i64, now_ms()],
                )?;
            }
            if let Some(v) = clipboard_sync {
                c.execute(
                    "UPDATE trusted_devices SET clipboard_sync = ?2, updated_at = ?3 WHERE device_id = ?1 AND status = 'trusted'",
                    params![id, v as i64, now_ms()],
                )?;
            }
            Ok(())
        })
    }

    /// Auto-accept granted directly or through any group the device is in.
    pub fn effective_auto_accept(&self, id: &str) -> Result<bool> {
        self.with(|c| {
            c.query_row(
                "SELECT EXISTS(SELECT 1 FROM trusted_devices WHERE device_id = ?1 AND status = 'trusted' AND auto_accept = 1)
                     OR EXISTS(SELECT 1 FROM device_group_members m JOIN device_groups g ON g.id = m.group_id
                               JOIN trusted_devices t ON t.device_id = m.device_id AND t.status = 'trusted'
                               WHERE m.device_id = ?1 AND g.auto_accept = 1)",
                [id],
                |r| r.get(0),
            )
        })
    }

    /// Clipboard sync allowed directly or through any group.
    pub fn effective_clipboard_sync(&self, id: &str) -> Result<bool> {
        self.with(|c| {
            c.query_row(
                "SELECT EXISTS(SELECT 1 FROM trusted_devices WHERE device_id = ?1 AND status = 'trusted' AND clipboard_sync = 1)
                     OR EXISTS(SELECT 1 FROM device_group_members m JOIN device_groups g ON g.id = m.group_id
                               JOIN trusted_devices t ON t.device_id = m.device_id AND t.status = 'trusted'
                               WHERE m.device_id = ?1 AND g.clipboard_sync = 1)",
                [id],
                |r| r.get(0),
            )
        })
    }

    // ---- groups -------------------------------------------------------

    pub fn list_groups(&self) -> Result<Vec<GroupView>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT id, name, color, auto_accept, clipboard_sync, created_at FROM device_groups ORDER BY name",
            )?;
            let mut groups = stmt
                .query_map([], |r| {
                    Ok(GroupView {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        color: r.get(2)?,
                        auto_accept: r.get::<_, i64>(3)? != 0,
                        clipboard_sync: r.get::<_, i64>(4)? != 0,
                        member_ids: Vec::new(),
                        created_at: r.get(5)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let mut members =
                c.prepare("SELECT device_id FROM device_group_members WHERE group_id = ?1 ORDER BY added_at")?;
            for g in &mut groups {
                g.member_ids = members
                    .query_map([&g.id], |r| r.get(0))?
                    .collect::<rusqlite::Result<Vec<String>>>()?;
            }
            Ok(groups)
        })
    }

    pub fn create_group(&self, id: &str, name: &str, color: &str) -> Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO device_groups (id, name, color, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![id, name, color, now_ms()],
            )
        })?;
        Ok(())
    }

    pub fn update_group(
        &self,
        id: &str,
        name: Option<&str>,
        color: Option<&str>,
        auto_accept: Option<bool>,
        clipboard_sync: Option<bool>,
    ) -> Result<()> {
        self.with(|c| {
            if let Some(v) = name {
                c.execute("UPDATE device_groups SET name = ?2 WHERE id = ?1", params![id, v])?;
            }
            if let Some(v) = color {
                c.execute("UPDATE device_groups SET color = ?2 WHERE id = ?1", params![id, v])?;
            }
            if let Some(v) = auto_accept {
                c.execute("UPDATE device_groups SET auto_accept = ?2 WHERE id = ?1", params![id, v as i64])?;
            }
            if let Some(v) = clipboard_sync {
                c.execute("UPDATE device_groups SET clipboard_sync = ?2 WHERE id = ?1", params![id, v as i64])?;
            }
            Ok(())
        })
    }

    pub fn delete_group(&self, id: &str) -> Result<()> {
        self.with(|c| c.execute("DELETE FROM device_groups WHERE id = ?1", [id]))?;
        Ok(())
    }

    pub fn set_group_members(&self, id: &str, device_ids: &[String]) -> Result<()> {
        let now = now_ms();
        self.with_tx(|tx| {
            tx.execute("DELETE FROM device_group_members WHERE group_id = ?1", [id])?;
            let mut ins = tx.prepare(
                "INSERT OR IGNORE INTO device_group_members (group_id, device_id, added_at) VALUES (?1, ?2, ?3)",
            )?;
            for d in device_ids {
                ins.execute(params![id, d, now])?;
            }
            Ok(())
        })
    }

    pub fn group_ids_for_device(&self, device_id: &str) -> Result<Vec<String>> {
        self.with(|c| {
            let mut stmt = c.prepare("SELECT group_id FROM device_group_members WHERE device_id = ?1")?;
            let rows = stmt.query_map([device_id], |r| r.get(0))?;
            rows.collect()
        })
    }

    pub fn group_member_ids(&self, group_id: &str) -> Result<Vec<String>> {
        self.with(|c| {
            let mut stmt = c.prepare("SELECT device_id FROM device_group_members WHERE group_id = ?1")?;
            let rows = stmt.query_map([group_id], |r| r.get(0))?;
            rows.collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen<'a>(id: &'a str, name: &'a str) -> SeenDevice<'a> {
        SeenDevice { id, name, os: "macos", app_version: Some("1.0.0"), address: Some("10.0.0.2"), port: Some(47100) }
    }

    #[test]
    fn upsert_deduplicates_and_reports_new() {
        let db = Db::open_in_memory().unwrap();
        let id = "a".repeat(64);
        assert!(db.upsert_device_seen(&seen(&id, "Mac")).unwrap());
        assert!(!db.upsert_device_seen(&seen(&id, "Mac renamed")).unwrap());
        let all = db.list_devices().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "Mac renamed");
        assert_eq!(all[0].trust, TrustState::Unknown);
    }

    #[test]
    fn trust_block_and_group_permissions() {
        let db = Db::open_in_memory().unwrap();
        let id = "b".repeat(64);
        db.upsert_device_seen(&seen(&id, "PC")).unwrap();
        assert!(!db.effective_auto_accept(&id).unwrap());

        db.set_trusted(&id).unwrap();
        assert_eq!(db.get_device(&id).unwrap().unwrap().trust, TrustState::Trusted);
        assert!(db.effective_clipboard_sync(&id).unwrap());

        db.create_group("g1", "Office", "blue").unwrap();
        db.set_group_members("g1", &[id.clone()]).unwrap();
        db.update_group("g1", None, None, Some(true), None).unwrap();
        assert!(db.effective_auto_accept(&id).unwrap());

        // Blocking overrides every grant, including group ones.
        db.set_blocked(&id).unwrap();
        assert!(!db.effective_auto_accept(&id).unwrap());
        assert!(!db.effective_clipboard_sync(&id).unwrap());
        assert_eq!(db.list_groups().unwrap()[0].member_ids, vec![id.clone()]);

        db.delete_device(&id).unwrap();
        assert!(db.list_groups().unwrap()[0].member_ids.is_empty());
    }
}
