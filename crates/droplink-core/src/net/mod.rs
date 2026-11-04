//! Connection setup. Every connection is TLS 1.3 with client certificates and
//! begins with a `Hello` exchange whose claimed device ID must equal the
//! fingerprint of the certificate used in the handshake.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tokio_util::sync::CancellationToken;

use crate::engine::Engine;
use crate::identity::fingerprint;
use crate::models::TrustState;
use crate::protocol::{read_message, write_message, Hello, Message, Purpose};
use crate::registry::PresenceSource;
use crate::tls::SERVER_NAME;
use crate::util::is_local_ip;
use crate::{Error, Result, PROTOCOL_VERSION};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(6);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

pub trait Io: AsyncRead + AsyncWrite + Unpin + Send + 'static {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send + 'static> Io for T {}

pub type Reader = ReadHalf<Box<dyn Io>>;
pub type Writer = WriteHalf<Box<dyn Io>>;

#[derive(Debug, Clone)]
pub struct Peer {
    /// Certificate fingerprint — the only identity that is trusted.
    pub id: String,
    pub hello: Hello,
    pub addr: SocketAddr,
}

pub struct Conn {
    pub r: Reader,
    pub w: Writer,
    pub peer: Peer,
}

impl Conn {
    pub async fn send(&mut self, msg: &Message) -> Result<()> {
        write_message(&mut self.w, msg).await
    }
    pub async fn recv(&mut self) -> Result<Message> {
        read_message(&mut self.r).await
    }
}

/// Closes our side cleanly and waits briefly for the peer to finish.
///
/// Closing a socket that still has unread data makes Windows send a TCP RST,
/// and an RST makes the peer discard data it has not read yet — including
/// the error message that explains *why* we are closing. Sending
/// close_notify and draining until the peer hangs up avoids that.
pub(crate) async fn graceful_close(conn: &mut Conn) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let _ = conn.w.shutdown().await;
    let mut buf = [0u8; 8192];
    let _ = timeout(Duration::from_secs(2), async {
        while let Ok(n) = conn.r.read(&mut buf).await {
            if n == 0 {
                break;
            }
        }
    })
    .await;
}

fn peer_fingerprint(certs: Option<&[rustls::pki_types::CertificateDer<'static>]>) -> Result<String> {
    certs
        .and_then(|c| c.first())
        .map(fingerprint)
        .ok_or_else(|| Error::Protocol("peer presented no certificate".into()))
}

impl Engine {
    pub(crate) fn hello(&self, purpose: Purpose) -> Hello {
        let s = self.settings.read();
        Hello {
            protocol: PROTOCOL_VERSION,
            device_id: self.identity.device_id.clone(),
            name: s.device_name.clone(),
            os: crate::util::os_name().into(),
            app_version: self.cfg.app_version.clone(),
            port: self.port(),
            purpose,
        }
    }

    /// Opens an authenticated connection. When `expected` is given the peer's
    /// certificate must match it exactly; this is what stops another machine
    /// from answering on a trusted device's address.
    pub async fn connect_addr(
        &self,
        addr: SocketAddr,
        expected: Option<&str>,
        purpose: Purpose,
    ) -> Result<Conn> {
        let tcp = timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
            .await
            .map_err(|_| Error::Unreachable)?
            .map_err(|_| Error::Unreachable)?;
        tcp.set_nodelay(true)?;
        let connector = TlsConnector::from(self.tls.client.clone());
        let name = rustls::pki_types::ServerName::try_from(SERVER_NAME).expect("static name");
        let tls = timeout(HANDSHAKE_TIMEOUT, connector.connect(name, tcp)).await??;
        let id = peer_fingerprint(tls.get_ref().1.peer_certificates())?;
        if id == self.identity.device_id {
            return Err(Error::Invalid("that address belongs to this device".into()));
        }
        if let Some(expected) = expected {
            if expected != id {
                self.record_security(
                    "identity_mismatch",
                    Some(expected),
                    &format!("{addr} presented a different certificate ({})", crate::util::short_id(&id)),
                );
                return Err(Error::IdentityMismatch);
            }
        }
        let boxed: Box<dyn Io> = Box::new(tls);
        let (mut r, mut w) = tokio::io::split(boxed);
        write_message(&mut w, &Message::Hello(self.hello(purpose))).await?;
        let reply = timeout(HANDSHAKE_TIMEOUT, read_message(&mut r)).await??;
        let hello = match reply {
            Message::Hello(h) => h,
            other => return Err(Error::Protocol(format!("expected hello, got {other:?}"))),
        };
        if hello.device_id != id {
            return Err(Error::IdentityMismatch);
        }
        let peer = Peer { id, hello, addr };
        self.note_peer(&peer, PresenceSource::Manual, false);
        Ok(Conn { r, w, peer })
    }

    /// Connects to a known device using the best address we have for it.
    pub async fn connect_device(&self, id: &str, purpose: Purpose) -> Result<Conn> {
        let mut candidates: Vec<SocketAddr> = Vec::new();
        if let Some(addr) = self.registry.get(id).and_then(|p| p.best_addr()) {
            candidates.push(addr);
        }
        if let Some(row) = self.db.get_device(id)? {
            if let Some(manual) = row.manual_address.as_deref() {
                if let Ok(addrs) = tokio::net::lookup_host(manual).await {
                    candidates.extend(addrs.filter(|a| a.is_ipv4()));
                }
            }
            if let (Some(ip), Some(port)) = (row.last_address.as_deref(), row.last_port) {
                if let Ok(ip) = ip.parse() {
                    candidates.push(SocketAddr::new(ip, port));
                }
            }
        }
        candidates.dedup();
        if candidates.is_empty() {
            return Err(Error::Unreachable);
        }
        let mut last_err = Error::Unreachable;
        for addr in candidates {
            match self.connect_addr(addr, Some(id), purpose).await {
                Ok(c) => return Ok(c),
                Err(e @ (Error::IdentityMismatch | Error::Blocked | Error::NotTrusted)) => return Err(e),
                Err(e) => last_err = e,
            }
        }
        if self.registry.mark_offline(id) {
            self.emit_device(id);
        }
        Err(last_err)
    }

    pub(crate) async fn bind_listener(&self, port: u16) -> Result<TcpListener> {
        let host = if self.cfg.loopback_only { "127.0.0.1" } else { "0.0.0.0" };
        if port == 0 {
            return Ok(TcpListener::bind((host, 0)).await?);
        }
        // Fall back to nearby ports if the preferred one is taken (e.g. a
        // second instance during development); mDNS advertises the real one.
        for p in port..port.saturating_add(10) {
            match TcpListener::bind((host, p)).await {
                Ok(l) => return Ok(l),
                Err(e) => tracing::debug!("port {p} unavailable: {e}"),
            }
        }
        Ok(TcpListener::bind((host, 0)).await?)
    }

    pub(crate) fn spawn_listener(&self, listener: TcpListener, stop: CancellationToken) {
        let engine = self.clone();
        self.spawn(async move {
            loop {
                let accepted = tokio::select! {
                    a = listener.accept() => a,
                    _ = stop.cancelled() => break,
                };
                match accepted {
                    Ok((tcp, addr)) => {
                        let engine = engine.clone();
                        engine.clone().spawn(async move {
                            if let Err(e) = engine.handle_inbound(tcp, addr).await {
                                tracing::debug!(%addr, "inbound connection ended: {e}");
                            }
                        });
                    }
                    Err(e) => {
                        tracing::warn!("accept failed: {e}");
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                }
            }
            tracing::info!("listener stopped");
        });
    }

    async fn handle_inbound(&self, tcp: TcpStream, addr: SocketAddr) -> Result<()> {
        if !is_local_ip(addr.ip()) && !self.settings.read().allow_remote_connections {
            tracing::info!(%addr, "refusing connection from outside the local network");
            return Ok(());
        }
        tcp.set_nodelay(true)?;
        let acceptor = TlsAcceptor::from(self.tls.server.clone());
        let tls = timeout(HANDSHAKE_TIMEOUT, acceptor.accept(tcp)).await??;
        let id = peer_fingerprint(tls.get_ref().1.peer_certificates())?;
        if id == self.identity.device_id {
            return Ok(());
        }
        let boxed: Box<dyn Io> = Box::new(tls);
        let (mut r, mut w) = tokio::io::split(boxed);
        let hello = match timeout(HANDSHAKE_TIMEOUT, read_message(&mut r)).await?? {
            Message::Hello(h) => h,
            _ => return Err(Error::Protocol("expected hello".into())),
        };
        if hello.device_id != id {
            let _ = write_message(&mut w, &Message::error("identity", "device id does not match certificate")).await;
            self.record_security("identity_mismatch", Some(&id), &format!("{addr} claimed another device's ID"));
            return Err(Error::IdentityMismatch);
        }
        if hello.protocol != PROTOCOL_VERSION {
            let _ = write_message(
                &mut w,
                &Message::error("version", format!("protocol {PROTOCOL_VERSION} required")),
            )
            .await;
            return Err(Error::Protocol("protocol version mismatch".into()));
        }
        let peer = Peer { id: id.clone(), hello: hello.clone(), addr };
        let mut conn = Conn { r, w, peer };
        let result = if self.trust_of(&id) == TrustState::Blocked {
            self.record_security("blocked_connection", Some(&id), &format!("Refused {:?} request from {addr}", hello.purpose));
            conn.send(&Message::error("blocked", "this device has been blocked")).await
        } else {
            match conn.send(&Message::Hello(self.hello(hello.purpose))).await {
                Err(e) => Err(e),
                Ok(()) => {
                    self.note_peer(&conn.peer, PresenceSource::Inbound, true);
                    match hello.purpose {
                        Purpose::Probe => Ok(()),
                        Purpose::Pair => self.handle_pair_request(&mut conn).await,
                        Purpose::Transfer => self.handle_transfer(&mut conn).await,
                        Purpose::Text => self.handle_text(&mut conn).await,
                        Purpose::Clipboard => self.handle_clipboard(&mut conn).await,
                        Purpose::Control => self.handle_control(&mut conn).await,
                    }
                }
            }
        };
        graceful_close(&mut conn).await;
        result
    }

    /// Records a successfully authenticated peer: refreshes its database row
    /// and marks it online.
    pub(crate) fn note_peer(&self, peer: &Peer, source: PresenceSource, inbound: bool) {
        let h = &peer.hello;
        let ip = peer.addr.ip().to_string();
        let port = if inbound { h.port } else { peer.addr.port() };
        let is_new = self
            .db
            .upsert_device_seen(&crate::db::SeenDevice {
                id: &peer.id,
                name: &h.name,
                os: &h.os,
                app_version: Some(&h.app_version),
                address: Some(&ip),
                port: Some(port),
            })
            .unwrap_or(false);
        self.registry.mark_online(&peer.id, vec![peer.addr.ip()], port, source, None);
        self.on_device_seen(&peer.id, is_new);
    }
}
