//! Instant text messages between trusted devices.

use std::time::Duration;

use serde::Serialize;

use crate::db::{DeliveryRow, HistoryRecord};
use crate::engine::Engine;
use crate::events::Event;
use crate::models::*;
use crate::net::Conn;
use crate::protocol::{Message, Purpose};
use crate::util::now_ms;
use crate::{Error, Result};

pub const MAX_TEXT_BYTES: usize = 1_000_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendResult {
    pub device_id: String,
    pub device_name: String,
    pub ok: bool,
    pub error: Option<String>,
}

fn label_for(content: &str) -> String {
    let n = content.chars().count();
    format!("Text · {n} character{}", if n == 1 { "" } else { "s" })
}

impl Engine {
    pub async fn send_text(&self, content: String, recipients: Vec<String>) -> Result<Vec<SendResult>> {
        if content.trim().is_empty() {
            return Err(Error::Invalid("the message is empty".into()));
        }
        if content.len() > MAX_TEXT_BYTES {
            return Err(Error::Invalid("the message is larger than 1 MB — send it as a file".into()));
        }
        let mut recipients = recipients;
        recipients.sort();
        recipients.dedup();
        if recipients.is_empty() {
            return Err(Error::Invalid("choose at least one device".into()));
        }
        let mut set = tokio::task::JoinSet::new();
        for device_id in recipients {
            let engine = self.clone();
            let content = content.clone();
            set.spawn(async move {
                let started = now_ms();
                let result = engine.send_text_to(&device_id, &content).await;
                let name = engine.device_name(&device_id);
                engine.record_text_history(Direction::Outgoing, &device_id, &content, started, result.as_ref().err());
                SendResult {
                    ok: result.is_ok(),
                    error: result.err().map(|e| e.to_string()),
                    device_name: name,
                    device_id,
                }
            });
        }
        let mut out = Vec::new();
        while let Some(r) = set.join_next().await {
            out.push(r.map_err(|e| Error::Other(e.to_string()))?);
        }
        Ok(out)
    }

    async fn send_text_to(&self, device_id: &str, content: &str) -> Result<()> {
        if self.trust_of(device_id) != TrustState::Trusted {
            return Err(Error::NotTrusted);
        }
        let mut conn = self.connect_device(device_id, Purpose::Text).await?;
        let id = uuid::Uuid::new_v4().to_string();
        conn.send(&Message::Text { id: id.clone(), content: content.to_string() }).await?;
        match tokio::time::timeout(Duration::from_secs(15), conn.recv()).await?? {
            Message::Ack => {}
            other => return Err(Error::Protocol(format!("unexpected reply {other:?}"))),
        }
        self.db.insert_text(&TextMessageView {
            id,
            direction: Direction::Outgoing,
            device_id: device_id.into(),
            device_name: self.device_name(device_id),
            content: content.into(),
            created_at: now_ms(),
        })?;
        Ok(())
    }

    pub(crate) async fn handle_text(&self, conn: &mut Conn) -> Result<()> {
        let peer = conn.peer.id.clone();
        if self.trust_of(&peer) != TrustState::Trusted {
            conn.send(&Message::error("not_trusted", "pair with this device first")).await?;
            self.record_security("rejected_untrusted", Some(&peer), "Refused a text message from a device that is not trusted");
            return Ok(());
        }
        let (id, content) = match tokio::time::timeout(Duration::from_secs(30), conn.recv()).await?? {
            Message::Text { id, content } => (id, content),
            _ => return Err(Error::Protocol("expected text".into())),
        };
        if content.len() > MAX_TEXT_BYTES || uuid::Uuid::parse_str(&id).is_err() {
            conn.send(&Message::error("rejected", "message too large")).await?;
            return Ok(());
        }
        let started = now_ms();
        let message = TextMessageView {
            id: uuid::Uuid::new_v4().to_string(),
            direction: Direction::Incoming,
            device_id: peer.clone(),
            device_name: self.device_name(&peer),
            content,
            created_at: now_ms(),
        };
        self.db.insert_text(&message)?;
        conn.send(&Message::Ack).await?;
        self.record_text_history(Direction::Incoming, &peer, &message.content, started, None);
        self.emit(Event::TextReceived { message });
        Ok(())
    }

    fn record_text_history(&self, direction: Direction, device_id: &str, content: &str, started: i64, err: Option<&Error>) {
        let me = self.identity.device_id.clone();
        let my_name = self.settings.read().device_name.clone();
        let peer_name = self.device_name(device_id);
        let row = DeliveryRow {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: String::new(),
            direction,
            label: label_for(content),
            device_id: device_id.into(),
            device_name: peer_name.clone(),
            device_os: String::new(),
            status: if err.is_some() { DeliveryStatus::Failed } else { DeliveryStatus::Completed },
            bytes_done: content.len() as u64,
            total_bytes: content.len() as u64,
            files_done: 0,
            total_files: 0,
            error: err.map(|e| e.to_string()),
            verified: None,
            auto_resume: false,
            created_at: started,
            started_at: Some(started),
            finished_at: Some(now_ms()),
            dest_dir: None,
            conflict_policy: None,
            sender_device_id: String::new(),
            first_file: None,
        };
        let (sid, sname, rid, rname) = match direction {
            Direction::Outgoing => (me, my_name, device_id.to_string(), peer_name),
            Direction::Incoming => (device_id.to_string(), peer_name, me, my_name),
        };
        let _ = self.db.record_history(&HistoryRecord {
            delivery: &row,
            kind: "text",
            sender_id: &sid,
            sender_name: &sname,
            recipient_id: &rid,
            recipient_name: &rname,
        });
    }

    pub fn list_texts(&self, limit: u32) -> Result<Vec<TextMessageView>> {
        self.db.list_texts(limit)
    }

    pub fn delete_text(&self, id: &str) -> Result<()> {
        self.db.delete_text(id)
    }

    pub fn clear_texts(&self) -> Result<()> {
        self.db.clear_texts()
    }
}
