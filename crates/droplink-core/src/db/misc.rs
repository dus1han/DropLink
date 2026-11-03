use rusqlite::{params, OptionalExtension};

use super::Db;
use crate::models::*;
use crate::settings::Settings;
use crate::util::now_ms;
use crate::Result;

pub struct NewClip<'a> {
    pub origin: &'a str,
    pub device_id: Option<&'a str>,
    pub device_name: Option<&'a str>,
    pub kind: &'a str,
    pub text: Option<&'a str>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub byte_size: u64,
    pub hash: &'a str,
}

impl Db {
    // ---- settings -----------------------------------------------------

    /// Settings are stored one JSON value per top-level key so that new
    /// fields pick up defaults and unknown keys from newer versions survive.
    pub fn load_settings(&self) -> Result<Settings> {
        let pairs: Vec<(String, String)> = self.with(|c| {
            let mut stmt = c.prepare("SELECT key, value FROM application_settings")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect()
        })?;
        let mut base = serde_json::to_value(Settings::default())?;
        if let serde_json::Value::Object(map) = &mut base {
            for (k, v) in pairs {
                if let Ok(value) = serde_json::from_str(&v) {
                    map.insert(k, value);
                }
            }
        }
        Ok(serde_json::from_value::<Settings>(base).unwrap_or_default().normalized())
    }

    pub fn save_settings(&self, s: &Settings) -> Result<()> {
        let value = serde_json::to_value(s)?;
        let now = now_ms();
        self.with_tx(|tx| {
            let mut stmt = tx.prepare(
                "INSERT INTO application_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
                 WHERE application_settings.value <> excluded.value",
            )?;
            if let serde_json::Value::Object(map) = value {
                for (k, v) in map {
                    stmt.execute(params![k, v.to_string(), now])?;
                }
            }
            Ok(())
        })
    }

    // ---- text messages ------------------------------------------------

    pub fn insert_text(&self, m: &TextMessageView) -> Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO text_messages (id, direction, device_id, device_name, content, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![m.id, m.direction.as_str(), m.device_id, m.device_name, m.content, m.created_at],
            )
        })?;
        Ok(())
    }

    pub fn list_texts(&self, limit: u32) -> Result<Vec<TextMessageView>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT id, direction, device_id, device_name, content, created_at
                 FROM text_messages ORDER BY created_at DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map([limit as i64], |r| {
                Ok(TextMessageView {
                    id: r.get(0)?,
                    direction: Direction::parse(&r.get::<_, String>(1)?),
                    device_id: r.get(2)?,
                    device_name: r.get(3)?,
                    content: r.get(4)?,
                    created_at: r.get(5)?,
                })
            })?;
            rows.collect()
        })
    }

    pub fn delete_text(&self, id: &str) -> Result<()> {
        self.with(|c| c.execute("DELETE FROM text_messages WHERE id = ?1", [id]))?;
        Ok(())
    }

    pub fn clear_texts(&self) -> Result<()> {
        self.with(|c| c.execute("DELETE FROM text_messages", []))?;
        Ok(())
    }

    // ---- clipboard ----------------------------------------------------

    pub fn insert_clip(&self, e: &NewClip<'_>, keep: u32) -> Result<ClipboardEntryView> {
        let now = now_ms();
        let id = self.with_tx(|tx| {
            tx.execute(
                "INSERT INTO clipboard_history (origin, device_id, device_name, kind, content, width, height,
                    byte_size, hash, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    e.origin,
                    e.device_id,
                    e.device_name,
                    e.kind,
                    e.text,
                    e.width,
                    e.height,
                    e.byte_size as i64,
                    e.hash,
                    now
                ],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute(
                "DELETE FROM clipboard_history WHERE id NOT IN
                    (SELECT id FROM clipboard_history ORDER BY created_at DESC, id DESC LIMIT ?1)",
                [keep as i64],
            )?;
            Ok(id)
        })?;
        Ok(ClipboardEntryView {
            id,
            origin: e.origin.into(),
            device_id: e.device_id.map(Into::into),
            device_name: e.device_name.map(Into::into),
            kind: e.kind.into(),
            text: e.text.map(Into::into),
            width: e.width,
            height: e.height,
            byte_size: e.byte_size,
            created_at: now,
        })
    }

    pub fn list_clips(&self, limit: u32) -> Result<Vec<ClipboardEntryView>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT id, origin, device_id, device_name, kind, content, width, height, byte_size, created_at
                 FROM clipboard_history ORDER BY created_at DESC, id DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map([limit as i64], |r| {
                Ok(ClipboardEntryView {
                    id: r.get(0)?,
                    origin: r.get(1)?,
                    device_id: r.get(2)?,
                    device_name: r.get(3)?,
                    kind: r.get(4)?,
                    text: r.get(5)?,
                    width: r.get(6)?,
                    height: r.get(7)?,
                    byte_size: r.get::<_, i64>(8)? as u64,
                    created_at: r.get(9)?,
                })
            })?;
            rows.collect()
        })
    }

    pub fn get_clip_text(&self, id: i64) -> Result<Option<String>> {
        self.with(|c| {
            c.query_row("SELECT content FROM clipboard_history WHERE id = ?1", [id], |r| r.get(0))
                .optional()
                .map(Option::flatten)
        })
    }

    pub fn delete_clip(&self, id: i64) -> Result<()> {
        self.with(|c| c.execute("DELETE FROM clipboard_history WHERE id = ?1", [id]))?;
        Ok(())
    }

    pub fn clear_clips(&self) -> Result<()> {
        self.with(|c| c.execute("DELETE FROM clipboard_history", []))?;
        Ok(())
    }

    // ---- security events ----------------------------------------------

    pub fn insert_security_event(
        &self,
        kind: &str,
        device_id: Option<&str>,
        device_name: Option<&str>,
        detail: &str,
    ) -> Result<SecurityEventView> {
        let now = now_ms();
        let id = self.with(|c| {
            c.execute(
                "INSERT INTO security_events (kind, device_id, device_name, detail, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![kind, device_id, device_name, detail, now],
            )?;
            // Keep the log bounded.
            c.execute(
                "DELETE FROM security_events WHERE id <= (SELECT MAX(id) - 2000 FROM security_events)",
                [],
            )?;
            Ok(c.last_insert_rowid())
        })?;
        Ok(SecurityEventView {
            id,
            kind: kind.into(),
            device_id: device_id.map(Into::into),
            device_name: device_name.map(Into::into),
            detail: detail.into(),
            created_at: now,
        })
    }

    pub fn list_security_events(&self, limit: u32) -> Result<Vec<SecurityEventView>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT id, kind, device_id, device_name, detail, created_at
                 FROM security_events ORDER BY id DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map([limit as i64], |r| {
                Ok(SecurityEventView {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    device_id: r.get(2)?,
                    device_name: r.get(3)?,
                    detail: r.get(4)?,
                    created_at: r.get(5)?,
                })
            })?;
            rows.collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_with_defaults() {
        let db = Db::open_in_memory().unwrap();
        let mut s = db.load_settings().unwrap();
        assert_eq!(s, Settings::default().normalized());
        s.device_name = "Studio Mac".into();
        s.max_concurrent_transfers = 99;
        db.save_settings(&s.clone().normalized()).unwrap();
        let loaded = db.load_settings().unwrap();
        assert_eq!(loaded.device_name, "Studio Mac");
        assert_eq!(loaded.max_concurrent_transfers, 16);
    }

    #[test]
    fn clipboard_history_is_bounded() {
        let db = Db::open_in_memory().unwrap();
        for i in 0..10 {
            let t = format!("item {i}");
            db.insert_clip(
                &NewClip {
                    origin: "local",
                    device_id: None,
                    device_name: None,
                    kind: "text",
                    text: Some(&t),
                    width: None,
                    height: None,
                    byte_size: t.len() as u64,
                    hash: &t,
                },
                5,
            )
            .unwrap();
        }
        let clips = db.list_clips(100).unwrap();
        assert_eq!(clips.len(), 5);
        assert_eq!(clips[0].text.as_deref(), Some("item 9"));
    }
}
