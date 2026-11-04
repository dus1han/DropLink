//! Device pairing with a short authentication string (SAS).
//!
//! 1. Initiator → responder: `PairCommit { sha256(nonce_i) }`
//! 2. Responder → initiator: `PairNonce { nonce_r }`
//! 3. Initiator → responder: `PairReveal { nonce_i }` (responder checks it)
//! 4. Both derive a 6-digit code from both certificate fingerprints and both
//!    nonces and show it to their user.
//! 5. Each side sends `PairDecision`; trust is stored only if both accept.
//!
//! Because the TLS session binds both fingerprints, a man-in-the-middle ends
//! up with two different codes. The commitment stops him from choosing his
//! nonce after seeing the initiator's, so he cannot search for a collision:
//! his chance of success is one in a million per attempt.

use std::time::Duration;

use rand::RngCore;
use sha2::{Digest, Sha256};
use tokio::sync::oneshot;
use tokio::time::timeout;

use crate::engine::Engine;
use crate::events::{Event, PairingRole};
use crate::models::TrustState;
use crate::net::Conn;
use crate::protocol::{read_message, write_message, Message, Purpose};
use crate::{Error, Result};

const DECISION_TIMEOUT: Duration = Duration::from_secs(120);
const STEP_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_PENDING: usize = 4;

pub struct PendingPairing {
    pub device_id: String,
    decision: Option<oneshot::Sender<bool>>,
}

pub fn sas_code(initiator: &str, responder: &str, nonce_i: &[u8], nonce_r: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b"droplink-pair-v1");
    h.update(initiator.as_bytes());
    h.update(responder.as_bytes());
    h.update(nonce_i);
    h.update(nonce_r);
    let d = h.finalize();
    let n = u32::from_be_bytes([d[0], d[1], d[2], d[3]]) % 1_000_000;
    format!("{n:06}")
}

fn nonce() -> [u8; 32] {
    let mut n = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut n);
    n
}

fn decode_nonce(s: &str) -> Result<Vec<u8>> {
    let n = hex::decode(s).map_err(|_| Error::Protocol("bad nonce".into()))?;
    if n.len() != 32 {
        return Err(Error::Protocol("bad nonce length".into()));
    }
    Ok(n)
}

impl Engine {
    /// Starts pairing with a device. Progress is reported through
    /// `PairingCode` / `PairingFinished` events; returns the pairing id.
    pub fn pair(&self, device_id: &str) -> Result<String> {
        if self.trust_of(device_id) == TrustState::Blocked {
            return Err(Error::Blocked);
        }
        let pairing_id = uuid::Uuid::new_v4().to_string();
        let engine = self.clone();
        let (pid, did) = (pairing_id.clone(), device_id.to_string());
        self.spawn(async move {
            let result = engine.run_initiator(&pid, &did).await;
            engine.finish_pairing(&pid, &did, result);
        });
        Ok(pairing_id)
    }

    /// The local user's answer to a displayed pairing code.
    pub fn respond_pairing(&self, pairing_id: &str, accept: bool) -> Result<()> {
        let mut map = self.pairings.lock();
        let p = map.get_mut(pairing_id).ok_or_else(|| Error::Invalid("pairing is no longer active".into()))?;
        if let Some(tx) = p.decision.take() {
            let _ = tx.send(accept);
        }
        Ok(())
    }

    async fn run_initiator(&self, pairing_id: &str, device_id: &str) -> Result<bool> {
        let mut conn = self.connect_device(device_id, Purpose::Pair).await?;
        let na = nonce();
        conn.send(&Message::PairCommit { commitment: hex::encode(Sha256::digest(na)) }).await?;
        let nb = match timeout(STEP_TIMEOUT, conn.recv()).await?? {
            Message::PairNonce { nonce } => decode_nonce(&nonce)?,
            _ => return Err(Error::Protocol("expected pairing nonce".into())),
        };
        conn.send(&Message::PairReveal { nonce: hex::encode(na) }).await?;
        let code = sas_code(&self.identity.device_id, &conn.peer.id, &na, &nb);
        self.await_decisions(pairing_id, &mut conn, code, PairingRole::Initiator).await
    }

    pub(crate) async fn handle_pair_request(&self, conn: &mut Conn) -> Result<()> {
        let peer = conn.peer.clone();
        if self.pairings.lock().len() >= MAX_PENDING {
            conn.send(&Message::error("busy", "too many pairing requests")).await?;
            return Ok(());
        }
        let commitment = match timeout(STEP_TIMEOUT, conn.recv()).await?? {
            Message::PairCommit { commitment } => commitment,
            _ => return Err(Error::Protocol("expected pairing commitment".into())),
        };
        let nb = nonce();
        conn.send(&Message::PairNonce { nonce: hex::encode(nb) }).await?;
        let na = match timeout(STEP_TIMEOUT, conn.recv()).await?? {
            Message::PairReveal { nonce } => decode_nonce(&nonce)?,
            _ => return Err(Error::Protocol("expected pairing reveal".into())),
        };
        if hex::encode(Sha256::digest(&na)) != commitment {
            self.record_security("pairing_tampered", Some(&peer.id), "Pairing nonce did not match its commitment");
            return Err(Error::Protocol("pairing commitment mismatch".into()));
        }
        let code = sas_code(&peer.id, &self.identity.device_id, &na, &nb);
        let pairing_id = uuid::Uuid::new_v4().to_string();
        let result = self.await_decisions(&pairing_id, conn, code, PairingRole::Responder).await;
        self.finish_pairing(&pairing_id, &peer.id, result);
        Ok(())
    }

    /// Shows the code locally and exchanges both users' decisions.
    async fn await_decisions(
        &self,
        pairing_id: &str,
        conn: &mut Conn,
        code: String,
        role: PairingRole,
    ) -> Result<bool> {
        let peer = conn.peer.clone();
        let (r, w) = (&mut conn.r, &mut conn.w);
        let (tx, rx) = oneshot::channel();
        self.pairings.lock().insert(
            pairing_id.to_string(),
            PendingPairing { device_id: peer.id.clone(), decision: Some(tx) },
        );
        self.emit(Event::PairingCode {
            pairing_id: pairing_id.into(),
            device_id: peer.id.clone(),
            device_name: self.device_name(&peer.id),
            device_os: peer.hello.os.clone(),
            code,
            role,
        });

        let local = async {
            let accepted = rx.await.unwrap_or(false);
            write_message(w, &Message::PairDecision { accepted }).await.map(|_| accepted)
        };
        let remote = async {
            match read_message(r).await {
                Ok(Message::PairDecision { accepted }) => accepted,
                _ => false,
            }
        };
        tokio::pin!(local, remote);
        let (mut mine, mut theirs) = (None, None);
        let deadline = tokio::time::sleep(DECISION_TIMEOUT);
        tokio::pin!(deadline);
        while mine.is_none() || theirs.is_none() {
            tokio::select! {
                l = &mut local, if mine.is_none() => {
                    let accepted = l?;
                    mine = Some(accepted);
                    if !accepted { break; }
                }
                t = &mut remote, if theirs.is_none() => {
                    theirs = Some(t);
                    if !t { break; }
                }
                _ = &mut deadline => return Err(Error::Timeout),
                _ = self.shutdown.cancelled() => return Err(Error::Cancelled),
            }
        }
        match (mine, theirs) {
            (Some(true), Some(true)) => {
                self.db.set_trusted(&peer.id)?;
                Ok(true)
            }
            (Some(false), _) => Err(Error::Rejected("You declined the pairing".into())),
            _ => Err(Error::Rejected(format!("{} declined the pairing", self.device_name(&peer.id)))),
        }
    }

    fn finish_pairing(&self, pairing_id: &str, device_id: &str, result: Result<bool>) {
        self.pairings.lock().remove(pairing_id);
        let name = self.device_name(device_id);
        let (success, message) = match &result {
            Ok(_) => (true, format!("{name} is now a trusted device")),
            Err(Error::Timeout) => (false, "Pairing timed out".to_string()),
            Err(e) => (false, e.to_string()),
        };
        if success {
            self.record_security("paired", Some(device_id), "Paired after code verification");
        } else {
            tracing::info!(device_id, "pairing failed: {message}");
        }
        self.emit(Event::PairingFinished {
            pairing_id: pairing_id.into(),
            device_id: device_id.into(),
            device_name: name,
            success,
            message,
        });
        self.emit_device(device_id);
    }

    pub fn pending_pairing_device(&self, pairing_id: &str) -> Option<String> {
        self.pairings.lock().get(pairing_id).map(|p| p.device_id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sas_depends_on_every_input() {
        let base = sas_code("a", "b", &[1; 32], &[2; 32]);
        assert_eq!(base.len(), 6);
        assert_eq!(base, sas_code("a", "b", &[1; 32], &[2; 32]));
        assert_ne!(base, sas_code("b", "a", &[1; 32], &[2; 32]));
        assert_ne!(base, sas_code("a", "c", &[1; 32], &[2; 32]));
        assert_ne!(base, sas_code("a", "b", &[3; 32], &[2; 32]));
        assert_ne!(base, sas_code("a", "b", &[1; 32], &[3; 32]));
    }
}
