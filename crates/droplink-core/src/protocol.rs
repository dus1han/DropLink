//! Wire protocol spoken over the mutually-authenticated TLS stream.
//!
//! Every frame is `[kind: u8][len: u32 BE][payload]`. Control frames carry a
//! JSON `Message`; data frames carry raw file bytes so large transfers never
//! pay JSON/base64 overhead.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{Error, Result};

const KIND_CONTROL: u8 = 0;
const KIND_DATA: u8 = 1;
/// Offers for folders with many thousands of files can be large.
pub const MAX_CONTROL_FRAME: usize = 32 * 1024 * 1024;
pub const MAX_DATA_FRAME: usize = 64 * 1024 * 1024;
/// Size of the chunks the sender streams. Large enough to amortise framing
/// and TLS record overhead, small enough for smooth progress and pausing.
pub const CHUNK_SIZE: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    /// Identity exchange only (manual connection, reachability check).
    Probe,
    Pair,
    Transfer,
    Text,
    Clipboard,
    /// Small control requests about an existing transfer.
    Control,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hello {
    pub protocol: u32,
    pub device_id: String,
    pub name: String,
    pub os: String,
    pub app_version: String,
    /// Port the sender listens on, so the receiver can connect back.
    pub port: u16,
    pub purpose: Purpose,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferItem {
    /// `/`-separated path relative to the transfer root.
    pub path: String,
    pub size: u64,
    /// Modification time in milliseconds since the Unix epoch.
    pub mtime: Option<i64>,
    pub dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Offer {
    /// Stable across reconnects, so the receiver can resume partial files.
    pub session_id: String,
    pub label: String,
    pub items: Vec<OfferItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardPayload {
    /// SHA-256 of the content, used to break synchronisation loops.
    pub hash: String,
    pub kind: String,
    pub text: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Sent explicitly by the user rather than by automatic sync.
    pub manual: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Message {
    Hello(Hello),
    Error { code: String, message: String },

    PairCommit { commitment: String },
    PairNonce { nonce: String },
    PairReveal { nonce: String },
    PairDecision { accepted: bool },

    Offer(Offer),
    /// Per-item byte offset to resume from (equal to the size = already done).
    Accept { offsets: Vec<u64> },
    Reject { reason: String },
    FileStart { index: u32, offset: u64 },
    FileEnd { index: u32, sha256: String },
    Complete,
    /// Sent by the receiver after `Complete`: items whose checksum failed.
    Summary { failed: Vec<u32> },
    Pause,
    Cancel,
    /// Tell the receiver to discard partial data for a session.
    Abort { session_id: String },
    /// Receiver asks the sender to continue a paused session.
    ResumeRequest { session_id: String },

    Text { id: String, content: String },
    /// For images, the PNG bytes follow in a single data frame.
    Clipboard(ClipboardPayload),
    Ack,
}

impl Message {
    pub fn error(code: &str, message: impl Into<String>) -> Self {
        Message::Error { code: code.into(), message: message.into() }
    }
}

/// Turns a peer's `Error` message into the matching local error.
pub fn remote_error(code: &str, message: &str) -> Error {
    match code {
        "not_trusted" => Error::NotTrusted,
        "blocked" => Error::Blocked,
        "identity" => Error::IdentityMismatch,
        "busy" | "rejected" => Error::Rejected(message.to_string()),
        "failed" => Error::Other(message.to_string()),
        _ => Error::Protocol(format!("{code}: {message}")),
    }
}

pub enum Frame {
    Control(Message),
    /// Payload is in the caller's buffer.
    Data(usize),
}

pub async fn read_frame<R: AsyncRead + Unpin>(r: &mut R, buf: &mut Vec<u8>) -> Result<Frame> {
    let kind = r.read_u8().await?;
    let len = r.read_u32().await? as usize;
    match kind {
        KIND_CONTROL => {
            if len > MAX_CONTROL_FRAME {
                return Err(Error::Protocol("control frame too large".into()));
            }
            buf.resize(len, 0);
            r.read_exact(&mut buf[..len]).await?;
            Ok(Frame::Control(serde_json::from_slice(&buf[..len])?))
        }
        KIND_DATA => {
            if len > MAX_DATA_FRAME {
                return Err(Error::Protocol("data frame too large".into()));
            }
            buf.resize(len, 0);
            r.read_exact(&mut buf[..len]).await?;
            Ok(Frame::Data(len))
        }
        other => Err(Error::Protocol(format!("unknown frame kind {other}"))),
    }
}

/// Reads the next frame and requires it to be a control message. A peer
/// `Error` message is surfaced as an `Err`.
pub async fn read_message<R: AsyncRead + Unpin>(r: &mut R) -> Result<Message> {
    let mut buf = Vec::new();
    match read_frame(r, &mut buf).await? {
        Frame::Control(Message::Error { code, message }) => Err(remote_error(&code, &message)),
        Frame::Control(m) => Ok(m),
        Frame::Data(_) => Err(Error::Protocol("unexpected data frame".into())),
    }
}

pub async fn write_message<W: AsyncWrite + Unpin>(w: &mut W, msg: &Message) -> Result<()> {
    let body = serde_json::to_vec(msg)?;
    write_raw(w, KIND_CONTROL, &body).await
}

pub async fn write_data<W: AsyncWrite + Unpin>(w: &mut W, data: &[u8]) -> Result<()> {
    write_raw(w, KIND_DATA, data).await
}

async fn write_raw<W: AsyncWrite + Unpin>(w: &mut W, kind: u8, body: &[u8]) -> Result<()> {
    let mut header = [0u8; 5];
    header[0] = kind;
    header[1..].copy_from_slice(&(body.len() as u32).to_be_bytes());
    w.write_all(&header).await?;
    w.write_all(body).await?;
    w.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn frames_round_trip() {
        let (mut a, mut b) = tokio::io::duplex(1024 * 1024);
        write_message(&mut a, &Message::FileStart { index: 3, offset: 42 }).await.unwrap();
        write_data(&mut a, b"hello").await.unwrap();
        write_message(&mut a, &Message::error("not_trusted", "nope")).await.unwrap();

        let mut buf = Vec::new();
        match read_frame(&mut b, &mut buf).await.unwrap() {
            Frame::Control(Message::FileStart { index, offset }) => {
                assert_eq!((index, offset), (3, 42))
            }
            _ => panic!("expected FileStart"),
        }
        match read_frame(&mut b, &mut buf).await.unwrap() {
            Frame::Data(n) => assert_eq!(&buf[..n], b"hello"),
            _ => panic!("expected data"),
        }
        assert!(matches!(read_message(&mut b).await, Err(Error::NotTrusted)));
    }

    #[tokio::test]
    async fn rejects_oversized_frames() {
        let (mut a, mut b) = tokio::io::duplex(64);
        tokio::spawn(async move {
            let _ = a.write_all(&[KIND_CONTROL, 0xff, 0xff, 0xff, 0xff]).await;
        });
        let mut buf = Vec::new();
        assert!(read_frame(&mut b, &mut buf).await.is_err());
    }
}
