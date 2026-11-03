use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::time::Duration;

use filetime::FileTime;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufWriter};
use tokio::sync::oneshot;

use super::{files::MAX_ITEMS, partial_dir, ActiveCtl, Outcome, PendingIncoming, Progress};
use crate::db::{DeliveryRow, ItemRow, NewSession};
use crate::engine::Engine;
use crate::events::Event;
use crate::models::*;
use crate::net::{Conn, Peer, Reader, Writer};
use crate::protocol::{read_frame, read_message, write_message, Frame, Message, Offer, CHUNK_SIZE};
use crate::settings::ConflictPolicy;
use crate::util::{now_ms, safe_relative_path, unique_path};
use crate::{Error, Result};

const APPROVAL_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub enum IncomingDecision {
    Accept { dest_dir: Option<PathBuf>, overwrite: Option<bool> },
    Reject,
}

enum Verdict {
    Accept { dest_dir: PathBuf, overwrite: bool },
    Reject(String),
    /// The sender cancelled or disconnected while we were waiting.
    Withdrawn,
}

fn disk_err(what: &Path, e: std::io::Error) -> Error {
    Error::Other(format!("Could not write {}: {e}", what.display()))
}

struct Current {
    index: u32,
    file: BufWriter<tokio::fs::File>,
    hasher: Sha256,
    written: u64,
    size: u64,
    part: PathBuf,
}

impl Engine {
    pub(crate) async fn handle_transfer(&self, conn: &mut Conn) -> Result<()> {
        let peer = conn.peer.clone();
        let (r, w) = (&mut conn.r, &mut conn.w);
        if self.trust_of(&peer.id) != TrustState::Trusted {
            write_message(w, &Message::error("not_trusted", "pair with this device before sending files")).await?;
            self.record_security("rejected_untrusted", Some(&peer.id), "Refused files from a device that is not trusted");
            return Ok(());
        }
        let offer = match tokio::time::timeout(Duration::from_secs(30), read_message(r)).await?? {
            Message::Offer(o) => o,
            _ => return Err(Error::Protocol("expected an offer".into())),
        };
        if let Err(e) = validate_offer(&offer) {
            write_message(w, &Message::Reject { reason: e.to_string() }).await?;
            self.record_security("invalid_offer", Some(&peer.id), &e.to_string());
            return Ok(());
        }
        let id = offer.session_id.clone();
        if self.transfers.is_active(&id) {
            write_message(w, &Message::error("busy", "this transfer is already running")).await?;
            return Ok(());
        }

        let resume = match self.db.get_delivery(&id)? {
            Some(row) if row.direction == Direction::Incoming && row.device_id == peer.id => {
                if matches!(row.status, DeliveryStatus::Rejected | DeliveryStatus::Cancelled) {
                    // Sent again after a refusal: treat as a brand new request.
                    self.db.delete_session(&row.session_id)?;
                    false
                } else {
                    true
                }
            }
            Some(_) => {
                write_message(w, &Message::error("rejected", "invalid session")).await?;
                self.record_security("session_hijack", Some(&peer.id), "Offer reused another device's session id");
                return Ok(());
            }
            None => false,
        };

        if resume {
            let items = self.db.session_items(&id)?;
            if items.len() != offer.items.len()
                || items.iter().zip(&offer.items).any(|(a, b)| a.rel_path != b.path || a.size != b.size)
            {
                write_message(w, &Message::Reject { reason: "the files changed since the transfer started".into() })
                    .await?;
                return Ok(());
            }
        } else {
            match self.decide_offer(&peer, &offer, r).await? {
                Verdict::Accept { dest_dir, overwrite } => {
                    self.create_incoming_session(&peer, &offer, &dest_dir, overwrite)?;
                }
                Verdict::Reject(reason) => {
                    self.create_incoming_session(&peer, &offer, &self.settings().download_dir, false)?;
                    self.finalize(&id, Ok(Outcome::Rejected(reason.clone())));
                    write_message(w, &Message::Reject { reason }).await?;
                    return Ok(());
                }
                Verdict::Withdrawn => {
                    self.create_incoming_session(&peer, &offer, &self.settings().download_dir, false)?;
                    self.finalize(&id, Ok(Outcome::CancelledByPeer));
                    return Ok(());
                }
            }
        }

        let Some(ctl) = self.transfers.register(&id, &peer.id) else {
            write_message(w, &Message::error("busy", "this transfer is already running")).await?;
            return Ok(());
        };
        let result = self.run_incoming(&id, &ctl, r, w).await;
        if let Err(e) = &result {
            if !e.is_transient() {
                // Let the sender show the real reason instead of a dropped
                // connection (e.g. the disk is full).
                let _ = write_message(w, &Message::error("failed", e.to_string())).await;
            }
        }
        self.finalize(&id, result);
        self.transfers.unregister(&id);
        Ok(())
    }

    async fn decide_offer(&self, peer: &Peer, offer: &Offer, r: &mut Reader) -> Result<Verdict> {
        let s = self.settings();
        let overwrite = s.conflict_policy == ConflictPolicy::Overwrite;
        if !s.require_approval || self.db.effective_auto_accept(&peer.id)? {
            return Ok(Verdict::Accept { dest_dir: s.download_dir, overwrite });
        }

        let files: Vec<_> = offer.items.iter().filter(|i| !i.dir).collect();
        let tops: std::collections::HashSet<&str> =
            offer.items.iter().map(|i| i.path.split('/').next().unwrap_or("")).collect();
        let conflicts = tops.iter().filter(|t| s.download_dir.join(t).exists()).count() as u32;
        let view = IncomingRequestView {
            id: offer.session_id.clone(),
            device_id: peer.id.clone(),
            device_name: self.device_name(&peer.id),
            device_os: peer.hello.os.clone(),
            label: offer.label.clone(),
            total_bytes: files.iter().map(|i| i.size).sum(),
            total_files: files.len() as u32,
            items: offer
                .items
                .iter()
                .take(100)
                .map(|i| OfferItemPreview { path: i.path.clone(), size: i.size, is_dir: i.dir })
                .collect(),
            conflicts,
            dest_dir: s.download_dir.to_string_lossy().into_owned(),
            created_at: now_ms(),
        };
        let (tx, rx) = oneshot::channel();
        self.incoming.lock().insert(view.id.clone(), PendingIncoming { view: view.clone(), tx: Some(tx) });
        self.emit(Event::IncomingRequest { request: view });

        let verdict = tokio::select! {
            d = tokio::time::timeout(APPROVAL_TIMEOUT, rx) => match d {
                Ok(Ok(IncomingDecision::Accept { dest_dir, overwrite: ow })) => Verdict::Accept {
                    dest_dir: dest_dir.unwrap_or(s.download_dir.clone()),
                    overwrite: ow.unwrap_or(overwrite),
                },
                Ok(Ok(IncomingDecision::Reject)) | Ok(Err(_)) => Verdict::Reject("Declined".into()),
                Err(_) => Verdict::Reject("The request was not answered in time".into()),
            },
            // Any message (Cancel) or EOF while waiting means the sender gave up.
            _ = read_message(r) => Verdict::Withdrawn,
            _ = self.shutdown.cancelled() => Verdict::Withdrawn,
        };
        self.incoming.lock().remove(&offer.session_id);
        self.emit(Event::IncomingRequestResolved { request_id: offer.session_id.clone() });
        Ok(verdict)
    }

    fn create_incoming_session(&self, peer: &Peer, offer: &Offer, dest: &Path, overwrite: bool) -> Result<()> {
        std::fs::create_dir_all(dest).map_err(|e| disk_err(dest, e))?;
        // Resolve every top-level name once, so a renamed folder ("Photos
        // (2)") stays consistent for all files inside it and across resumes.
        let mut tops: HashMap<String, String> = HashMap::new();
        let mut items = Vec::with_capacity(offer.items.len());
        for (i, it) in offer.items.iter().enumerate() {
            let rel = safe_relative_path(&it.path)?;
            let mut comps = rel.components();
            let top = comps.next().map(|c| c.as_os_str().to_string_lossy().into_owned()).unwrap_or_default();
            let rest: PathBuf = comps.collect();
            let top_is_dir = it.dir || !rest.as_os_str().is_empty();
            let mapped = tops
                .entry(top.clone())
                .or_insert_with(|| {
                    let existing = dest.join(&top);
                    let compatible = existing.exists() && existing.is_dir() == top_is_dir;
                    if !existing.exists() || (overwrite && compatible) {
                        top.clone()
                    } else {
                        unique_path(dest, &top).file_name().unwrap().to_string_lossy().into_owned()
                    }
                })
                .clone();
            let mut final_path = dest.join(mapped);
            if !rest.as_os_str().is_empty() {
                // Joining an empty path would append a trailing separator.
                final_path.push(rest);
            }
            items.push(ItemRow {
                idx: i as u32,
                rel_path: it.path.clone(),
                source_path: None,
                final_path: Some(final_path.to_string_lossy().into_owned()),
                size: it.size,
                mtime: it.mtime,
                is_dir: it.dir,
                sha256: None,
                status: "pending".into(),
            });
        }
        let label: String = offer.label.chars().take(200).collect();
        let session = NewSession {
            id: offer.session_id.clone(),
            direction: Direction::Incoming,
            sender_device_id: peer.id.clone(),
            label,
            dest_dir: Some(dest.to_string_lossy().into_owned()),
            conflict_policy: Some(if overwrite { "overwrite" } else { "rename" }.into()),
            items,
        };
        self.db.create_session(&session, &[(session.id.clone(), peer.id.clone())], DeliveryStatus::Waiting)?;
        Ok(())
    }

    async fn run_incoming(&self, id: &str, ctl: &ActiveCtl, r: &mut Reader, w: &mut Writer) -> Result<Outcome> {
        let row = self.db.get_delivery(id)?.ok_or_else(|| Error::Other("session vanished".into()))?;
        let items = self.db.session_items(id)?;
        let dest = PathBuf::from(row.dest_dir.clone().unwrap_or_default());
        let pdir = partial_dir(&dest, id);
        let overwrite = row.conflict_policy.as_deref() == Some("overwrite");

        for it in items.iter().filter(|i| i.is_dir) {
            if let Some(p) = &it.final_path {
                tokio::fs::create_dir_all(p).await.map_err(|e| disk_err(Path::new(p), e))?;
            }
        }

        let mut offsets = Vec::with_capacity(items.len());
        let (mut bytes, mut files) = (0u64, 0u32);
        for it in &items {
            if it.is_dir {
                offsets.push(u64::MAX);
            } else if it.status == "completed" {
                offsets.push(u64::MAX);
                bytes += it.size;
                files += 1;
            } else {
                let part = pdir.join(format!("{}.part", it.idx));
                let have = match tokio::fs::metadata(&part).await {
                    Ok(m) if m.len() <= it.size => m.len(),
                    Ok(_) => {
                        let _ = tokio::fs::remove_file(&part).await;
                        0
                    }
                    Err(_) => 0,
                };
                bytes += have;
                offsets.push(have);
            }
        }
        write_message(w, &Message::Accept { offsets }).await?;
        self.set_status(id, DeliveryStatus::Transferring, None);

        let mut progress = Progress::new(&row, bytes, files);
        progress.tick(self, true);
        let mut cur: Option<Current> = None;
        let result = self.receive_loop(&row, &items, &pdir, overwrite, ctl, r, w, &mut progress, &mut cur).await;
        // Keep whatever arrived so the transfer can resume from here.
        if let Some(mut c) = cur {
            let _ = c.file.flush().await;
        }
        if matches!(result, Ok(Outcome::Completed)) && self.settings.read().preserve_timestamps {
            for it in items.iter().rev().filter(|i| i.is_dir) {
                if let (Some(p), Some(ms)) = (&it.final_path, it.mtime) {
                    let _ = filetime::set_file_mtime(p, to_filetime(ms));
                }
            }
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    async fn receive_loop(
        &self,
        row: &DeliveryRow,
        items: &[ItemRow],
        pdir: &Path,
        overwrite: bool,
        ctl: &ActiveCtl,
        r: &mut Reader,
        w: &mut Writer,
        progress: &mut Progress,
        cur: &mut Option<Current>,
    ) -> Result<Outcome> {
        let mut buf = Vec::with_capacity(CHUNK_SIZE + 64);
        let mut failed: Vec<u32> = Vec::new();
        let preserve = self.settings.read().preserve_timestamps;
        loop {
            let frame = tokio::select! {
                f = read_frame(r, &mut buf) => f?,
                _ = ctl.token.cancelled() => {
                    if let Some(m) = ctl.stop_message() {
                        let _ = write_message(w, &m).await;
                    }
                    return Ok(ctl.stopped_outcome());
                }
            };
            match frame {
                Frame::Data(n) => {
                    let c = cur.as_mut().ok_or_else(|| Error::Protocol("data outside of a file".into()))?;
                    if c.written + n as u64 > c.size {
                        return Err(Error::Protocol("more data than announced".into()));
                    }
                    c.file.write_all(&buf[..n]).await.map_err(|e| disk_err(&c.part, e))?;
                    c.hasher.update(&buf[..n]);
                    c.written += n as u64;
                    progress.add(n);
                    progress.tick(self, false);
                }
                Frame::Control(Message::FileStart { index, offset }) => {
                    if cur.is_some() {
                        return Err(Error::Protocol("file started before the previous one ended".into()));
                    }
                    let item = items
                        .get(index as usize)
                        .filter(|i| !i.is_dir && i.status != "completed")
                        .ok_or_else(|| Error::Protocol("invalid file index".into()))?;
                    *cur = Some(open_part(pdir, item, offset).await?);
                    progress.current_file = Some(item.rel_path.clone());
                }
                Frame::Control(Message::FileEnd { index, sha256 }) => {
                    let c = cur.take().filter(|c| c.index == index).ok_or_else(|| Error::Protocol("unexpected end of file".into()))?;
                    let Current { file, hasher, written, size, part, .. } = c;
                    let mut file = file;
                    file.flush().await.map_err(|e| disk_err(&part, e))?;
                    let file = file.into_inner();
                    file.sync_all().await.map_err(|e| disk_err(&part, e))?;
                    drop(file);
                    if written != size {
                        return Err(Error::Protocol("file ended early".into()));
                    }
                    let item = &items[index as usize];
                    let digest = hex::encode(hasher.finalize());
                    if digest.eq_ignore_ascii_case(&sha256) {
                        self.place_file(&row.id, &part, item, overwrite, preserve).await?;
                        self.db.set_item_status(&row.id, index, "completed", Some(&digest))?;
                        progress.files += 1;
                    } else {
                        let _ = tokio::fs::remove_file(&part).await;
                        self.db.set_item_status(&row.id, index, "failed", None)?;
                        progress.bytes = progress.bytes.saturating_sub(size);
                        failed.push(index);
                        self.record_security(
                            "integrity_failure",
                            Some(&row.device_id),
                            &format!("{} failed SHA-256 verification", item.rel_path),
                        );
                    }
                    progress.tick(self, false);
                }
                Frame::Control(Message::Complete) => {
                    if cur.is_some() {
                        return Err(Error::Protocol("transfer completed mid-file".into()));
                    }
                    write_message(w, &Message::Summary { failed: failed.clone() }).await?;
                    progress.current_file = None;
                    progress.tick(self, true);
                    return Ok(if failed.is_empty() {
                        Outcome::Completed
                    } else {
                        Outcome::IntegrityFailed(failed.len() as u32)
                    });
                }
                Frame::Control(Message::Pause) => return Ok(Outcome::PausedByPeer),
                Frame::Control(Message::Cancel) => return Ok(Outcome::CancelledByPeer),
                Frame::Control(other) => return Err(Error::Protocol(format!("unexpected message {other:?}"))),
            }
        }
    }

    async fn place_file(&self, delivery_id: &str, part: &Path, item: &ItemRow, overwrite: bool, preserve: bool) -> Result<PathBuf> {
        let mut target = PathBuf::from(
            item.final_path.as_deref().ok_or_else(|| Error::Other("missing destination".into()))?,
        );
        let parent = target.parent().map(Path::to_path_buf).unwrap_or_default();
        tokio::fs::create_dir_all(&parent).await.map_err(|e| disk_err(&parent, e))?;
        if target.exists() {
            if overwrite && target.is_file() {
                tokio::fs::remove_file(&target).await.map_err(|e| disk_err(&target, e))?;
            } else {
                let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                target = unique_path(&parent, &name);
                self.db.set_item_final_path(delivery_id, item.idx, &target.to_string_lossy())?;
            }
        }
        tokio::fs::rename(part, &target).await.map_err(|e| disk_err(&target, e))?;
        if preserve {
            if let Some(ms) = item.mtime {
                if let Err(e) = filetime::set_file_mtime(&target, to_filetime(ms)) {
                    tracing::warn!("could not preserve modification time of {}: {e}", target.display());
                }
            }
        }
        Ok(target)
    }
}

fn to_filetime(ms: i64) -> FileTime {
    FileTime::from_unix_time(ms.div_euclid(1000), (ms.rem_euclid(1000) * 1_000_000) as u32)
}

async fn open_part(pdir: &Path, item: &ItemRow, offset: u64) -> Result<Current> {
    tokio::fs::create_dir_all(pdir).await.map_err(|e| disk_err(pdir, e))?;
    let part = pdir.join(format!("{}.part", item.idx));
    let have = tokio::fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);
    if have < offset || offset > item.size {
        return Err(Error::Protocol("resume offset beyond the data received".into()));
    }
    let mut hasher = Sha256::new();
    if offset > 0 {
        let mut f = tokio::fs::File::open(&part).await.map_err(|e| disk_err(&part, e))?;
        let mut tmp = vec![0u8; CHUNK_SIZE];
        let mut left = offset;
        while left > 0 {
            let want = left.min(CHUNK_SIZE as u64) as usize;
            let n = f.read(&mut tmp[..want]).await.map_err(|e| disk_err(&part, e))?;
            if n == 0 {
                return Err(Error::Other("partial file is shorter than expected".into()));
            }
            hasher.update(&tmp[..n]);
            left -= n as u64;
        }
    }
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&part)
        .await
        .map_err(|e| disk_err(&part, e))?;
    file.set_len(offset).await.map_err(|e| disk_err(&part, e))?;
    file.seek(SeekFrom::Start(offset)).await.map_err(|e| disk_err(&part, e))?;
    Ok(Current {
        index: item.idx,
        file: BufWriter::with_capacity(1024 * 1024, file),
        hasher,
        written: offset,
        size: item.size,
        part,
    })
}

fn validate_offer(o: &Offer) -> Result<()> {
    uuid::Uuid::parse_str(&o.session_id).map_err(|_| Error::Protocol("invalid session id".into()))?;
    if o.items.is_empty() || o.items.len() > MAX_ITEMS {
        return Err(Error::Invalid("invalid number of files".into()));
    }
    let mut seen = std::collections::HashSet::with_capacity(o.items.len());
    for it in &o.items {
        let p = safe_relative_path(&it.path)?;
        if it.dir && it.size != 0 {
            return Err(Error::Protocol("folder entries cannot have a size".into()));
        }
        if !seen.insert(p.to_string_lossy().to_lowercase()) {
            return Err(Error::Protocol(format!("duplicate path {}", it.path)));
        }
    }
    Ok(())
}
