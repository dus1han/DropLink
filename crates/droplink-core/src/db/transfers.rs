use rusqlite::{params, OptionalExtension, Row};

use super::Db;
use crate::models::*;
use crate::util::now_ms;
use crate::Result;

#[derive(Debug, Clone)]
pub struct ItemRow {
    pub idx: u32,
    pub rel_path: String,
    pub source_path: Option<String>,
    pub final_path: Option<String>,
    pub size: u64,
    pub mtime: Option<i64>,
    pub is_dir: bool,
    pub sha256: Option<String>,
    pub status: String,
}

pub struct NewSession {
    pub id: String,
    pub direction: Direction,
    pub sender_device_id: String,
    pub label: String,
    pub dest_dir: Option<String>,
    pub conflict_policy: Option<String>,
    pub items: Vec<ItemRow>,
}

impl NewSession {
    pub fn total_bytes(&self) -> u64 {
        self.items.iter().filter(|i| !i.is_dir).map(|i| i.size).sum()
    }
    pub fn total_files(&self) -> u32 {
        self.items.iter().filter(|i| !i.is_dir).count() as u32
    }
}

#[derive(Debug, Clone)]
pub struct DeliveryRow {
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
    pub auto_resume: bool,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub dest_dir: Option<String>,
    pub conflict_policy: Option<String>,
    pub sender_device_id: String,
    pub first_file: Option<String>,
}

impl DeliveryRow {
    pub fn to_view(&self) -> DeliveryView {
        DeliveryView {
            id: self.id.clone(),
            session_id: self.session_id.clone(),
            direction: self.direction,
            label: self.label.clone(),
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
            device_os: self.device_os.clone(),
            status: self.status,
            bytes_done: self.bytes_done,
            total_bytes: self.total_bytes,
            files_done: self.files_done,
            total_files: self.total_files,
            error: self.error.clone(),
            verified: self.verified,
            created_at: self.created_at,
            started_at: self.started_at,
            finished_at: self.finished_at,
            dest_dir: self.dest_dir.clone(),
            first_file: self.first_file.clone(),
        }
    }
}

const DELIVERY_SELECT: &str = "SELECT r.id, r.session_id, s.direction, s.label, r.device_id,
        COALESCE(d.alias, d.name, 'Unknown device'), COALESCE(d.os, 'unknown'),
        r.status, r.bytes_done, s.total_bytes, r.files_done, s.total_files, r.error, r.verified,
        r.auto_resume, s.created_at, r.started_at, r.finished_at, s.dest_dir, s.conflict_policy,
        s.sender_device_id,
        (SELECT rel_path FROM transfer_items i WHERE i.session_id = s.id AND i.is_dir = 0 ORDER BY i.idx LIMIT 1)
    FROM transfer_recipients r
    JOIN transfer_sessions s ON s.id = r.session_id
    LEFT JOIN devices d ON d.id = r.device_id";

fn delivery_from_row(r: &Row<'_>) -> rusqlite::Result<DeliveryRow> {
    Ok(DeliveryRow {
        id: r.get(0)?,
        session_id: r.get(1)?,
        direction: Direction::parse(&r.get::<_, String>(2)?),
        label: r.get(3)?,
        device_id: r.get(4)?,
        device_name: r.get(5)?,
        device_os: r.get(6)?,
        status: DeliveryStatus::parse(&r.get::<_, String>(7)?),
        bytes_done: r.get::<_, i64>(8)? as u64,
        total_bytes: r.get::<_, i64>(9)? as u64,
        files_done: r.get::<_, i64>(10)? as u32,
        total_files: r.get::<_, i64>(11)? as u32,
        error: r.get(12)?,
        verified: r.get::<_, Option<i64>>(13)?.map(|v| v != 0),
        auto_resume: r.get::<_, i64>(14)? != 0,
        created_at: r.get(15)?,
        started_at: r.get(16)?,
        finished_at: r.get(17)?,
        dest_dir: r.get(18)?,
        conflict_policy: r.get(19)?,
        sender_device_id: r.get(20)?,
        first_file: r.get(21)?,
    })
}

fn item_from_row(r: &Row<'_>) -> rusqlite::Result<ItemRow> {
    Ok(ItemRow {
        idx: r.get::<_, i64>(0)? as u32,
        rel_path: r.get(1)?,
        source_path: r.get(2)?,
        final_path: r.get(3)?,
        size: r.get::<_, i64>(4)? as u64,
        mtime: r.get(5)?,
        is_dir: r.get::<_, i64>(6)? != 0,
        sha256: r.get(7)?,
        status: r.get(8)?,
    })
}

pub struct HistoryRecord<'a> {
    pub delivery: &'a DeliveryRow,
    pub kind: &'a str,
    pub sender_id: &'a str,
    pub sender_name: &'a str,
    pub recipient_id: &'a str,
    pub recipient_name: &'a str,
}

impl Db {
    /// Creates a session, its items and one delivery per remote device.
    pub fn create_session(
        &self,
        s: &NewSession,
        deliveries: &[(String, String)],
        status: DeliveryStatus,
    ) -> Result<()> {
        let now = now_ms();
        self.with_tx(|tx| {
            tx.execute(
                "INSERT INTO transfer_sessions (id, direction, sender_device_id, label, total_bytes, total_files,
                    dest_dir, conflict_policy, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    s.id,
                    s.direction.as_str(),
                    s.sender_device_id,
                    s.label,
                    s.total_bytes() as i64,
                    s.total_files() as i64,
                    s.dest_dir,
                    s.conflict_policy,
                    now
                ],
            )?;
            let mut ins = tx.prepare(
                "INSERT INTO transfer_items (session_id, idx, rel_path, source_path, final_path, size, mtime, is_dir, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending')",
            )?;
            for i in &s.items {
                ins.execute(params![
                    s.id,
                    i.idx as i64,
                    i.rel_path,
                    i.source_path,
                    i.final_path,
                    i.size as i64,
                    i.mtime,
                    i.is_dir as i64
                ])?;
            }
            let mut rec = tx.prepare(
                "INSERT INTO transfer_recipients (id, session_id, device_id, status, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for (delivery_id, device_id) in deliveries {
                rec.execute(params![delivery_id, s.id, device_id, status.as_str(), now])?;
            }
            Ok(())
        })
    }

    pub fn get_delivery(&self, id: &str) -> Result<Option<DeliveryRow>> {
        self.with(|c| {
            c.query_row(&format!("{DELIVERY_SELECT} WHERE r.id = ?1"), [id], delivery_from_row)
                .optional()
        })
    }

    /// Active deliveries first, then the most recently updated finished ones.
    pub fn list_deliveries(&self, limit: u32) -> Result<Vec<DeliveryRow>> {
        self.with(|c| {
            let mut stmt = c.prepare(&format!(
                "{DELIVERY_SELECT}
                 ORDER BY CASE WHEN r.status IN ('completed','failed','cancelled','rejected') THEN 1 ELSE 0 END,
                          r.updated_at DESC
                 LIMIT ?1"
            ))?;
            let rows = stmt.query_map([limit as i64], delivery_from_row)?;
            rows.collect()
        })
    }

    pub fn session_delivery_ids(&self, session_id: &str) -> Result<Vec<String>> {
        self.with(|c| {
            let mut stmt = c.prepare("SELECT id FROM transfer_recipients WHERE session_id = ?1")?;
            let rows = stmt.query_map([session_id], |r| r.get(0))?;
            rows.collect()
        })
    }

    pub fn set_delivery_status(
        &self,
        id: &str,
        status: DeliveryStatus,
        error: Option<&str>,
    ) -> Result<()> {
        let now = now_ms();
        self.with(|c| {
            c.execute(
                "UPDATE transfer_recipients SET
                    status = ?2,
                    error = ?3,
                    started_at = CASE WHEN ?2 = 'transferring' AND started_at IS NULL THEN ?4 ELSE started_at END,
                    finished_at = CASE WHEN ?2 IN ('completed','failed','cancelled','rejected') THEN ?4 ELSE NULL END,
                    auto_resume = CASE WHEN ?2 = 'paused' THEN auto_resume ELSE 0 END,
                    updated_at = ?4
                 WHERE id = ?1",
                params![id, status.as_str(), error, now],
            )
        })?;
        Ok(())
    }

    pub fn set_auto_resume(&self, id: &str, auto_resume: bool) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE transfer_recipients SET auto_resume = ?2 WHERE id = ?1",
                params![id, auto_resume as i64],
            )
        })?;
        Ok(())
    }

    pub fn set_delivery_progress(&self, id: &str, bytes: u64, files: u32) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE transfer_recipients SET bytes_done = ?2, files_done = ?3, updated_at = ?4 WHERE id = ?1",
                params![id, bytes as i64, files as i64, now_ms()],
            )
        })?;
        Ok(())
    }

    pub fn set_delivery_verified(&self, id: &str, verified: bool) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE transfer_recipients SET verified = ?2 WHERE id = ?1",
                params![id, verified as i64],
            )
        })?;
        Ok(())
    }

    pub fn session_items(&self, session_id: &str) -> Result<Vec<ItemRow>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT idx, rel_path, source_path, final_path, size, mtime, is_dir, sha256, status
                 FROM transfer_items WHERE session_id = ?1 ORDER BY idx",
            )?;
            let rows = stmt.query_map([session_id], item_from_row)?;
            rows.collect()
        })
    }

    pub fn set_item_status(
        &self,
        session_id: &str,
        idx: u32,
        status: &str,
        sha256: Option<&str>,
    ) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE transfer_items SET status = ?3, sha256 = COALESCE(?4, sha256) WHERE session_id = ?1 AND idx = ?2",
                params![session_id, idx as i64, status, sha256],
            )
        })?;
        Ok(())
    }

    pub fn set_item_final_path(&self, session_id: &str, idx: u32, path: &str) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE transfer_items SET final_path = ?3 WHERE session_id = ?1 AND idx = ?2",
                params![session_id, idx as i64, path],
            )
        })?;
        Ok(())
    }

    /// Called once at startup: anything that was running when the app closed
    /// is paused (and resumes automatically when the peer reappears).
    pub fn interrupt_active_deliveries(&self) -> Result<usize> {
        let now = now_ms();
        self.with(|c| {
            // Pending incoming approvals cannot survive a restart.
            let expired = c.execute(
                "UPDATE transfer_recipients SET status = 'failed', error = 'Request expired', finished_at = ?1, updated_at = ?1
                 WHERE status IN ('queued','connecting','waiting')
                   AND session_id IN (SELECT id FROM transfer_sessions WHERE direction = 'incoming')",
                [now],
            )?;
            let paused = c.execute(
                "UPDATE transfer_recipients SET status = 'paused', error = 'Interrupted — DropLink was closed',
                    auto_resume = CASE WHEN session_id IN (SELECT id FROM transfer_sessions WHERE direction = 'outgoing')
                                       THEN 1 ELSE 0 END,
                    updated_at = ?1
                 WHERE status IN ('queued','connecting','waiting','transferring')",
                [now],
            )?;
            Ok(expired + paused)
        })
    }

    pub fn auto_resume_candidates(&self, device_id: &str) -> Result<Vec<String>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT r.id FROM transfer_recipients r JOIN transfer_sessions s ON s.id = r.session_id
                 WHERE r.device_id = ?1 AND s.direction = 'outgoing' AND r.status = 'paused' AND r.auto_resume = 1",
            )?;
            let rows = stmt.query_map([device_id], |r| r.get(0))?;
            rows.collect()
        })
    }

    pub fn session_has_dirs(&self, session_id: &str) -> Result<bool> {
        self.with(|c| {
            c.query_row(
                "SELECT EXISTS(SELECT 1 FROM transfer_items WHERE session_id = ?1 AND is_dir = 1)",
                [session_id],
                |r| r.get(0),
            )
        })
    }

    /// Paused incoming deliveries (which keep partial files) last updated
    /// before `cutoff`, with their destination folder.
    pub fn resumable_incoming(&self, cutoff: i64) -> Result<Vec<(String, String)>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT r.id, s.dest_dir FROM transfer_recipients r JOIN transfer_sessions s ON s.id = r.session_id
                 WHERE s.direction = 'incoming' AND r.status = 'paused' AND s.dest_dir IS NOT NULL
                   AND r.updated_at < ?1",
            )?;
            let rows = stmt.query_map([cutoff], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect()
        })
    }

    /// Drops an incoming session so a rejected or cancelled offer can be
    /// offered again from scratch. History is kept.
    pub fn delete_session(&self, session_id: &str) -> Result<()> {
        self.with(|c| c.execute("DELETE FROM transfer_sessions WHERE id = ?1", [session_id]))?;
        Ok(())
    }

    /// Removes finished deliveries from the activity list (history is kept).
    pub fn clear_finished_deliveries(&self) -> Result<usize> {
        self.with_tx(|tx| {
            let n = tx.execute(
                "DELETE FROM transfer_recipients WHERE status IN ('completed','failed','cancelled','rejected')",
                [],
            )?;
            tx.execute(
                "DELETE FROM transfer_sessions WHERE id NOT IN (SELECT session_id FROM transfer_recipients)",
                [],
            )?;
            Ok(n)
        })
    }

    pub fn active_delivery_count(&self) -> Result<u32> {
        self.with(|c| {
            c.query_row(
                "SELECT COUNT(*) FROM transfer_recipients WHERE status IN ('queued','connecting','waiting','transferring')",
                [],
                |r| r.get(0),
            )
        })
    }

    // ---- history ------------------------------------------------------

    pub fn record_history(&self, h: &HistoryRecord<'_>) -> Result<()> {
        let d = h.delivery;
        let finished = d.finished_at.unwrap_or_else(now_ms);
        let duration = d.started_at.map(|s| (finished - s).max(0));
        self.with(|c| {
            c.execute(
                "INSERT INTO transfer_history (delivery_id, direction, kind, name, total_bytes, file_count,
                    sender_id, sender_name, recipient_id, recipient_name, status, error, verified,
                    started_at, finished_at, duration_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                 ON CONFLICT(delivery_id) DO UPDATE SET
                    status = excluded.status, error = excluded.error, verified = excluded.verified,
                    started_at = COALESCE(transfer_history.started_at, excluded.started_at),
                    finished_at = excluded.finished_at, duration_ms = excluded.duration_ms,
                    total_bytes = excluded.total_bytes",
                params![
                    d.id,
                    d.direction.as_str(),
                    h.kind,
                    d.label,
                    d.total_bytes as i64,
                    d.total_files as i64,
                    h.sender_id,
                    h.sender_name,
                    h.recipient_id,
                    h.recipient_name,
                    d.status.as_str(),
                    d.error,
                    d.verified.map(|v| v as i64),
                    d.started_at,
                    finished,
                    duration
                ],
            )
        })?;
        Ok(())
    }

    pub fn query_history(&self, q: &HistoryQuery) -> Result<Vec<HistoryEntry>> {
        let mut sql = String::from(
            "SELECT h.id, h.delivery_id, h.direction, h.kind, h.name, h.total_bytes, h.file_count,
                    h.sender_id, h.sender_name, h.recipient_id, h.recipient_name, h.status, h.error,
                    h.verified, h.started_at, h.finished_at, h.duration_ms,
                    EXISTS(SELECT 1 FROM transfer_recipients r WHERE r.id = h.delivery_id)
             FROM transfer_history h WHERE 1 = 1",
        );
        let mut args: Vec<rusqlite::types::Value> = Vec::new();
        if let Some(s) = q.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            let like = format!("%{}%", s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
            sql.push_str(&format!(
                " AND (h.name LIKE ?{0} ESCAPE '\\' OR h.sender_name LIKE ?{0} ESCAPE '\\' OR h.recipient_name LIKE ?{0} ESCAPE '\\')",
                args.len() + 1
            ));
            args.push(like.into());
        }
        if let Some(s) = q.status.as_deref().filter(|s| !s.is_empty() && *s != "all") {
            if s == "unsuccessful" {
                sql.push_str(" AND h.status <> 'completed'");
            } else {
                sql.push_str(&format!(" AND h.status = ?{}", args.len() + 1));
                args.push(s.to_string().into());
            }
        }
        if let Some(d) = q.device_id.as_deref().filter(|s| !s.is_empty()) {
            sql.push_str(&format!(" AND (h.sender_id = ?{0} OR h.recipient_id = ?{0})", args.len() + 1));
            args.push(d.to_string().into());
        }
        if let Some(d) = q.direction.as_deref().filter(|s| *s == "incoming" || *s == "outgoing") {
            sql.push_str(&format!(" AND h.direction = ?{}", args.len() + 1));
            args.push(d.to_string().into());
        }
        if let Some(from) = q.from_ms {
            sql.push_str(&format!(" AND h.finished_at >= ?{}", args.len() + 1));
            args.push(from.into());
        }
        if let Some(to) = q.to_ms {
            sql.push_str(&format!(" AND h.finished_at < ?{}", args.len() + 1));
            args.push(to.into());
        }
        sql.push_str(&format!(" ORDER BY h.finished_at DESC LIMIT {}", q.limit.unwrap_or(500).min(5000)));

        self.with(|c| {
            let mut stmt = c.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
                let direction = Direction::parse(&r.get::<_, String>(2)?);
                let status = DeliveryStatus::parse(&r.get::<_, String>(11)?);
                let kind: String = r.get(3)?;
                let delivery_exists: bool = r.get(17)?;
                Ok(HistoryEntry {
                    id: r.get(0)?,
                    delivery_id: r.get(1)?,
                    direction,
                    retryable: delivery_exists
                        && direction == Direction::Outgoing
                        && kind != "text"
                        && matches!(status, DeliveryStatus::Failed | DeliveryStatus::Cancelled),
                    kind,
                    name: r.get(4)?,
                    total_bytes: r.get::<_, i64>(5)? as u64,
                    file_count: r.get::<_, i64>(6)? as u32,
                    sender_id: r.get(7)?,
                    sender_name: r.get(8)?,
                    recipient_id: r.get(9)?,
                    recipient_name: r.get(10)?,
                    status,
                    error: r.get(12)?,
                    verified: r.get::<_, Option<i64>>(13)?.map(|v| v != 0),
                    started_at: r.get(14)?,
                    finished_at: r.get(15)?,
                    duration_ms: r.get(16)?,
                })
            })?;
            rows.collect()
        })
    }

    pub fn clear_history(&self) -> Result<usize> {
        self.with(|c| c.execute("DELETE FROM transfer_history", []))
    }

    pub fn prune_history(&self, retention_days: u32) -> Result<usize> {
        let cutoff = now_ms() - retention_days as i64 * 86_400_000;
        self.with(|c| c.execute("DELETE FROM transfer_history WHERE finished_at < ?1", [cutoff]))
    }

    pub fn stats(&self, start_of_day_ms: i64) -> Result<StatsView> {
        self.with(|c| {
            let (files_today, bytes_today): (i64, i64) = c.query_row(
                "SELECT COALESCE(SUM(file_count), 0), COALESCE(SUM(total_bytes), 0) FROM transfer_history
                 WHERE status = 'completed' AND kind <> 'text' AND finished_at >= ?1",
                [start_of_day_ms],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let (transfers_total, bytes_total): (i64, i64) = c.query_row(
                "SELECT COUNT(*), COALESCE(SUM(total_bytes), 0) FROM transfer_history WHERE status = 'completed'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let (known, trusted): (i64, i64) = c.query_row(
                "SELECT (SELECT COUNT(*) FROM devices),
                        (SELECT COUNT(*) FROM trusted_devices WHERE status = 'trusted')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            Ok(StatsView {
                known_devices: known as u32,
                trusted_devices: trusted as u32,
                online_devices: 0,
                active_transfers: 0,
                files_today: files_today as u64,
                bytes_today: bytes_today as u64,
                bytes_total: bytes_total as u64,
                transfers_total: transfers_total as u64,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::devices::SeenDevice;

    fn session(id: &str) -> NewSession {
        NewSession {
            id: id.into(),
            direction: Direction::Outgoing,
            sender_device_id: "me".into(),
            label: "report.pdf".into(),
            dest_dir: None,
            conflict_policy: None,
            items: vec![
                ItemRow {
                    idx: 0,
                    rel_path: "docs".into(),
                    source_path: None,
                    final_path: None,
                    size: 0,
                    mtime: None,
                    is_dir: true,
                    sha256: None,
                    status: "pending".into(),
                },
                ItemRow {
                    idx: 1,
                    rel_path: "docs/report.pdf".into(),
                    source_path: Some("/tmp/report.pdf".into()),
                    final_path: None,
                    size: 1000,
                    mtime: Some(1),
                    is_dir: false,
                    sha256: None,
                    status: "pending".into(),
                },
            ],
        }
    }

    #[test]
    fn delivery_lifecycle_and_history() {
        let db = Db::open_in_memory().unwrap();
        let peer = "c".repeat(64);
        db.upsert_device_seen(&SeenDevice { id: &peer, name: "Laptop", os: "windows", app_version: None, address: None, port: None })
            .unwrap();
        db.create_session(&session("s1"), &[("d1".into(), peer.clone())], DeliveryStatus::Queued).unwrap();

        let d = db.get_delivery("d1").unwrap().unwrap();
        assert_eq!((d.total_bytes, d.total_files), (1000, 1));
        assert_eq!(d.device_name, "Laptop");
        assert_eq!(d.first_file.as_deref(), Some("docs/report.pdf"));

        db.set_delivery_status("d1", DeliveryStatus::Transferring, None).unwrap();
        db.set_delivery_progress("d1", 500, 0).unwrap();
        db.interrupt_active_deliveries().unwrap();
        let d = db.get_delivery("d1").unwrap().unwrap();
        assert_eq!(d.status, DeliveryStatus::Paused);
        assert!(d.auto_resume);
        assert_eq!(db.auto_resume_candidates(&peer).unwrap(), vec!["d1".to_string()]);

        db.set_delivery_status("d1", DeliveryStatus::Failed, Some("boom")).unwrap();
        let d = db.get_delivery("d1").unwrap().unwrap();
        db.record_history(&HistoryRecord {
            delivery: &d,
            kind: "files",
            sender_id: "me",
            sender_name: "Me",
            recipient_id: &peer,
            recipient_name: "Laptop",
        })
        .unwrap();
        let h = db.query_history(&HistoryQuery { status: Some("failed".into()), ..Default::default() }).unwrap();
        assert_eq!(h.len(), 1);
        assert!(h[0].retryable);
        let h = db.query_history(&HistoryQuery { search: Some("100%_".into()), ..Default::default() }).unwrap();
        assert!(h.is_empty(), "LIKE wildcards in search must be escaped");
        let h = db.query_history(&HistoryQuery { search: Some("REPORT".into()), ..Default::default() }).unwrap();
        assert_eq!(h.len(), 1);

        assert_eq!(db.clear_finished_deliveries().unwrap(), 1);
        assert!(db.session_items("s1").unwrap().is_empty());
    }
}
