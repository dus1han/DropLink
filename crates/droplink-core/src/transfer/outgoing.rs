use std::path::PathBuf;
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

use super::{files, ActiveCtl, Outcome, Progress};
use crate::db::{DeliveryRow, ItemRow, NewSession};
use crate::engine::Engine;
use crate::models::{DeliveryStatus, Direction, TrustState};
use crate::net::{Conn, Writer};
use crate::protocol::{read_message, write_data, write_message, Message, Offer, OfferItem, Purpose, CHUNK_SIZE};
use crate::{Error, Result};

/// How long the receiving user has to accept a request.
const APPROVAL_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const SUMMARY_TIMEOUT: Duration = Duration::from_secs(120);
const RETRY_BACKOFF_SECS: [u64; 3] = [2, 5, 12];

fn read_err(item: &ItemRow, e: std::io::Error) -> Error {
    Error::Other(format!("Cannot read {}: {e}", item.rel_path))
}

async fn read_full<R: AsyncRead + Unpin>(r: &mut R, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        let m = r.read(&mut buf[n..]).await?;
        if m == 0 {
            break;
        }
        n += m;
    }
    Ok(n)
}

impl Engine {
    /// Queues `paths` (files and/or folders) for every recipient. Each
    /// recipient gets an independent delivery. Only trusted devices can be
    /// recipients.
    pub async fn send_files(&self, paths: Vec<PathBuf>, recipients: Vec<String>) -> Result<String> {
        let mut recipients = recipients;
        recipients.sort();
        recipients.dedup();
        if recipients.is_empty() {
            return Err(Error::Invalid("choose at least one device".into()));
        }
        for r in &recipients {
            if r == self.device_id() {
                return Err(Error::Invalid("cannot send to this device".into()));
            }
            if self.trust_of(r) != TrustState::Trusted {
                return Err(Error::NotTrusted);
            }
        }
        let collected = tokio::task::spawn_blocking(move || files::collect(&paths))
            .await
            .map_err(|e| Error::Other(e.to_string()))??;
        if collected.items.is_empty() {
            return Err(Error::Invalid("nothing to send".into()));
        }

        let session = NewSession {
            id: uuid::Uuid::new_v4().to_string(),
            direction: Direction::Outgoing,
            sender_device_id: self.device_id().into(),
            label: collected.label,
            dest_dir: None,
            conflict_policy: None,
            items: collected
                .items
                .into_iter()
                .enumerate()
                .map(|(i, it)| ItemRow {
                    idx: i as u32,
                    rel_path: it.rel_path,
                    source_path: Some(it.source.to_string_lossy().into_owned()),
                    final_path: None,
                    size: it.size,
                    mtime: it.mtime,
                    is_dir: it.is_dir,
                    sha256: None,
                    status: "pending".into(),
                })
                .collect(),
        };
        let deliveries: Vec<(String, String)> =
            recipients.iter().map(|r| (uuid::Uuid::new_v4().to_string(), r.clone())).collect();
        self.db.create_session(&session, &deliveries, DeliveryStatus::Queued)?;
        for (id, _) in &deliveries {
            self.emit_delivery(id);
            self.spawn_outgoing(id);
        }
        Ok(session.id)
    }

    pub async fn send_to_group(&self, paths: Vec<PathBuf>, group_id: &str) -> Result<String> {
        let recipients = self.group_recipients(group_id)?;
        if recipients.is_empty() {
            return Err(Error::Invalid("this group has no trusted devices".into()));
        }
        self.send_files(paths, recipients).await
    }

    pub(crate) fn spawn_outgoing(&self, id: &str) {
        let Ok(Some(row)) = self.db.get_delivery(id) else { return };
        let Some(ctl) = self.transfers.register(id, &row.device_id) else { return };
        let engine = self.clone();
        let id = id.to_string();
        self.spawn(async move {
            let result = engine.run_outgoing(&id, &ctl).await;
            engine.finalize(&id, result);
            engine.transfers.unregister(&id);
        });
    }

    async fn run_outgoing(&self, id: &str, ctl: &ActiveCtl) -> Result<Outcome> {
        let mut attempt = 0;
        loop {
            match self.outgoing_attempt(id, ctl).await {
                _ if ctl.token.is_cancelled() => return Ok(ctl.stopped_outcome()),
                Err(e) if e.is_transient() && attempt < RETRY_BACKOFF_SECS.len() => {
                    tracing::info!(delivery = id, "attempt failed ({e}); retrying");
                    let msg = format!("Connection interrupted — retrying ({}/{})", attempt + 1, RETRY_BACKOFF_SECS.len());
                    self.set_status(id, DeliveryStatus::Connecting, Some(&msg));
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(RETRY_BACKOFF_SECS[attempt])) => {}
                        _ = ctl.token.cancelled() => return Ok(ctl.stopped_outcome()),
                    }
                    attempt += 1;
                }
                r => return r,
            }
        }
    }

    async fn outgoing_attempt(&self, id: &str, ctl: &ActiveCtl) -> Result<Outcome> {
        let row = self.db.get_delivery(id)?.ok_or_else(|| Error::Invalid("unknown transfer".into()))?;
        let items = self.db.session_items(&row.session_id)?;
        if row.status != DeliveryStatus::Connecting {
            self.set_status(id, DeliveryStatus::Queued, None);
        }
        let _permit = tokio::select! {
            p = self.transfers.gate.acquire() => p,
            _ = ctl.token.cancelled() => return Ok(ctl.stopped_outcome()),
        };
        if self.trust_of(&row.device_id) != TrustState::Trusted {
            return Err(Error::NotTrusted);
        }
        self.set_status(id, DeliveryStatus::Connecting, None);
        let conn = tokio::select! {
            c = self.connect_device(&row.device_id, Purpose::Transfer) => c?,
            _ = ctl.token.cancelled() => return Ok(ctl.stopped_outcome()),
        };
        let Conn { mut r, mut w, .. } = conn;

        let offer = Offer {
            session_id: id.into(),
            label: row.label.clone(),
            items: items
                .iter()
                .map(|i| OfferItem { path: i.rel_path.clone(), size: i.size, mtime: i.mtime, dir: i.is_dir })
                .collect(),
        };
        write_message(&mut w, &Message::Offer(offer)).await?;
        self.set_status(id, DeliveryStatus::Waiting, None);

        let reply = tokio::select! {
            m = tokio::time::timeout(APPROVAL_TIMEOUT, read_message(&mut r)) => match m {
                Ok(m) => m?,
                Err(_) => return Ok(Outcome::Rejected("The request was not answered in time".into())),
            },
            _ = ctl.token.cancelled() => return stop_stream(ctl, &mut w).await,
        };
        let offsets = match reply {
            Message::Accept { offsets } => offsets,
            Message::Reject { reason } => return Ok(Outcome::Rejected(reason)),
            other => return Err(Error::Protocol(format!("unexpected reply to offer: {other:?}"))),
        };
        if offsets.len() != items.len() {
            return Err(Error::Protocol("offset list does not match the offer".into()));
        }
        self.set_status(id, DeliveryStatus::Transferring, None);

        // The receiver may pause, cancel or report an error at any time, so
        // read its messages concurrently with streaming.
        let (tx, mut rx) = mpsc::channel::<Result<Message>>(8);
        let reader = tokio::spawn(async move {
            loop {
                let m = read_message(&mut r).await;
                let stop = m.is_err();
                if tx.send(m).await.is_err() || stop {
                    break;
                }
            }
        });
        let result = self.stream_items(&row, &items, &offsets, ctl, &mut w, &mut rx).await;
        reader.abort();
        let _ = w.shutdown().await;
        result
    }

    async fn stream_items(
        &self,
        row: &DeliveryRow,
        items: &[ItemRow],
        offsets: &[u64],
        ctl: &ActiveCtl,
        w: &mut Writer,
        rx: &mut mpsc::Receiver<Result<Message>>,
    ) -> Result<Outcome> {
        let (mut bytes, mut files) = (0u64, 0u32);
        for (item, &off) in items.iter().zip(offsets) {
            if item.is_dir {
                continue;
            }
            if off == u64::MAX {
                bytes += item.size;
                files += 1;
            } else if off > item.size {
                return Err(Error::Protocol("resume offset beyond end of file".into()));
            } else {
                bytes += off;
            }
        }
        let mut progress = Progress::new(row, bytes, files);
        progress.tick(self, true);
        let mut buf = vec![0u8; CHUNK_SIZE];

        for (i, item) in items.iter().enumerate() {
            let offset = offsets[i];
            if item.is_dir || offset == u64::MAX {
                continue;
            }
            progress.current_file = Some(item.rel_path.clone());
            let source = item
                .source_path
                .as_deref()
                .ok_or_else(|| Error::Other(format!("{} has no source path", item.rel_path)))?;
            let mut file = tokio::fs::File::open(source).await.map_err(|e| read_err(item, e))?;
            let len = file.metadata().await.map_err(|e| read_err(item, e))?.len();
            if len != item.size {
                return Err(Error::Other(format!("{} changed after it was selected", item.rel_path)));
            }
            write_message(w, &Message::FileStart { index: i as u32, offset }).await?;

            // The digest covers the whole file, so re-hash the part the
            // receiver already has (a local read, no network traffic).
            let mut hasher = Sha256::new();
            let mut skip = offset;
            while skip > 0 {
                let want = skip.min(CHUNK_SIZE as u64) as usize;
                let n = read_full(&mut file, &mut buf[..want]).await.map_err(|e| read_err(item, e))?;
                if n == 0 {
                    return Err(Error::Other(format!("{} changed after it was selected", item.rel_path)));
                }
                hasher.update(&buf[..n]);
                skip -= n as u64;
            }

            let mut sent = offset;
            while sent < item.size {
                if ctl.token.is_cancelled() {
                    return stop_stream(ctl, w).await;
                }
                if let Some(o) = check_peer(rx)? {
                    return Ok(o);
                }
                let want = (item.size - sent).min(CHUNK_SIZE as u64) as usize;
                let n = read_full(&mut file, &mut buf[..want]).await.map_err(|e| read_err(item, e))?;
                if n == 0 {
                    return Err(Error::Other(format!("{} changed after it was selected", item.rel_path)));
                }
                hasher.update(&buf[..n]);
                tokio::select! {
                    _ = self.transfers.limiter.acquire(n) => {}
                    _ = ctl.token.cancelled() => return stop_stream(ctl, w).await,
                }
                write_data(w, &buf[..n]).await?;
                sent += n as u64;
                progress.add(n);
                progress.tick(self, false);
            }
            write_message(w, &Message::FileEnd { index: i as u32, sha256: hex::encode(hasher.finalize()) }).await?;
            progress.files += 1;
            progress.tick(self, false);
        }

        write_message(w, &Message::Complete).await?;
        progress.current_file = None;
        progress.tick(self, true);

        loop {
            let m = tokio::select! {
                m = tokio::time::timeout(SUMMARY_TIMEOUT, rx.recv()) => m.map_err(|_| Error::Timeout)?,
                _ = ctl.token.cancelled() => return stop_stream(ctl, w).await,
            };
            match m {
                None => return Err(Error::Io(std::io::ErrorKind::UnexpectedEof.into())),
                Some(Err(e)) => return Err(e),
                Some(Ok(Message::Summary { failed })) => {
                    return Ok(if failed.is_empty() {
                        Outcome::Completed
                    } else {
                        Outcome::IntegrityFailed(failed.len() as u32)
                    })
                }
                Some(Ok(Message::Pause)) => return Ok(Outcome::PausedByPeer),
                Some(Ok(Message::Cancel)) => return Ok(Outcome::CancelledByPeer),
                Some(Ok(_)) => continue,
            }
        }
    }
}

fn check_peer(rx: &mut mpsc::Receiver<Result<Message>>) -> Result<Option<Outcome>> {
    use tokio::sync::mpsc::error::TryRecvError;
    match rx.try_recv() {
        Ok(Ok(Message::Pause)) => Ok(Some(Outcome::PausedByPeer)),
        Ok(Ok(Message::Cancel)) => Ok(Some(Outcome::CancelledByPeer)),
        Ok(Ok(_)) => Ok(None),
        Ok(Err(e)) => Err(e),
        Err(TryRecvError::Empty) => Ok(None),
        Err(TryRecvError::Disconnected) => Err(Error::Io(std::io::ErrorKind::UnexpectedEof.into())),
    }
}

async fn stop_stream(ctl: &ActiveCtl, w: &mut Writer) -> Result<Outcome> {
    if let Some(m) = ctl.stop_message() {
        let _ = write_message(w, &m).await;
    }
    Ok(ctl.stopped_outcome())
}
