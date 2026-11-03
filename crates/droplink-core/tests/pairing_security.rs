//! Pairing, trust and protocol-level security.

mod common;

use std::net::SocketAddr;

use common::*;
use droplink_core::models::{DeliveryStatus, TrustState};
use droplink_core::protocol::{read_frame, write_data, Frame, Message, Offer, OfferItem, Purpose};
use droplink_core::{Error, Event};

#[tokio::test]
async fn pairing_creates_mutual_trust() {
    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    pair(&mut a, &mut b).await;

    let b_on_a = a.engine.device_view(&b.id()).unwrap().unwrap();
    let a_on_b = b.engine.device_view(&a.id()).unwrap().unwrap();
    assert_eq!(b_on_a.trust, TrustState::Trusted);
    assert_eq!(a_on_b.trust, TrustState::Trusted);
    assert_eq!(b_on_a.name, "Bravo");
    assert_eq!(a_on_b.name, "Alpha");
    assert!(a.engine.security_events(10).unwrap().iter().any(|e| e.kind == "paired"));
}

#[tokio::test]
async fn declined_pairing_grants_no_trust() {
    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    a.engine.connect_manual(&b.addr()).await.unwrap();
    a.engine.pair(&b.id()).unwrap();
    let pid_b = b
        .wait_for(10, |e| match e {
            Event::PairingCode { pairing_id, .. } => Some(pairing_id.clone()),
            _ => None,
        })
        .await;
    let pid_a = a
        .wait_for(10, |e| match e {
            Event::PairingCode { pairing_id, .. } => Some(pairing_id.clone()),
            _ => None,
        })
        .await;
    // The initiator confirms but the responder declines: neither side may trust.
    a.engine.respond_pairing(&pid_a, true).unwrap();
    b.engine.respond_pairing(&pid_b, false).unwrap();
    for n in [&mut a, &mut b] {
        let ok = n
            .wait_for(10, |e| match e {
                Event::PairingFinished { success, .. } => Some(*success),
                _ => None,
            })
            .await;
        assert!(!ok);
    }
    assert_eq!(a.engine.device_view(&b.id()).unwrap().unwrap().trust, TrustState::Unknown);
    assert_eq!(b.engine.device_view(&a.id()).unwrap().unwrap().trust, TrustState::Unknown);
}

#[tokio::test]
async fn untrusted_devices_cannot_send_anything() {
    let mut a = node("Alpha").await;
    let b = node("Bravo").await;
    a.engine.connect_manual(&b.addr()).await.unwrap();

    // Locally refused before touching the network.
    let file = a.dir.join("x.txt");
    std::fs::write(&file, b"hi").unwrap();
    assert!(matches!(a.engine.send_files(vec![file.clone()], vec![b.id()]).await, Err(Error::NotTrusted)));

    // And refused by the receiver when a sender skips the local check.
    let addr: SocketAddr = b.addr().parse().unwrap();
    let mut conn = a.engine.connect_addr(addr, Some(&b.id()), Purpose::Transfer).await.unwrap();
    conn.send(&Message::Offer(offer("../evil.txt"))).await.unwrap();
    assert!(matches!(conn.recv().await, Err(Error::NotTrusted)));
    let mut conn = a.engine.connect_addr(addr, Some(&b.id()), Purpose::Text).await.unwrap();
    conn.send(&Message::Text { id: uuid::Uuid::new_v4().to_string(), content: "hi".into() }).await.unwrap();
    assert!(matches!(conn.recv().await, Err(Error::NotTrusted)));
    assert!(b.engine.list_texts(10).unwrap().is_empty());
    let _ = a.events.try_recv();
}

#[tokio::test]
async fn revoked_trust_stops_transfers_from_that_device() {
    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    pair(&mut a, &mut b).await;
    b.engine.revoke_device(&a.id()).unwrap();

    let file = a.dir.join("doc.txt");
    std::fs::write(&file, b"secret").unwrap();
    let sid = a.engine.send_files(vec![file], vec![b.id()]).await.unwrap();
    let d = a.wait_outgoing(&sid, 20).await;
    assert_eq!(d.status, DeliveryStatus::Failed);
    assert!(d.error.unwrap().contains("not trusted"));
    assert!(std::fs::read_dir(&b.downloads).unwrap().next().is_none());
}

#[tokio::test]
async fn blocked_devices_are_refused_at_connection() {
    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    pair(&mut a, &mut b).await;
    b.engine.block_device(&a.id()).unwrap();
    assert_eq!(b.engine.device_view(&a.id()).unwrap().unwrap().trust, TrustState::Blocked);

    let addr: SocketAddr = b.addr().parse().unwrap();
    let r = a.engine.connect_addr(addr, Some(&b.id()), Purpose::Probe).await;
    assert!(matches!(r, Err(Error::Blocked)), "got {:?}", r.err());
    assert!(b.engine.security_events(20).unwrap().iter().any(|e| e.kind == "blocked_connection"));

    b.engine.unblock_device(&a.id()).unwrap();
    assert!(a.engine.connect_addr(addr, Some(&b.id()), Purpose::Probe).await.is_ok());
    // Unblocking does not restore trust; pairing is required again.
    assert_eq!(b.engine.device_view(&a.id()).unwrap().unwrap().trust, TrustState::Unknown);
}

#[tokio::test]
async fn certificate_must_match_expected_identity() {
    let a = node("Alpha").await;
    let b = node("Bravo").await;
    let c = node("Charlie").await;
    // Someone else answering on the address we expect `b` at.
    let addr: SocketAddr = c.addr().parse().unwrap();
    let r = a.engine.connect_addr(addr, Some(&b.id()), Purpose::Probe).await;
    assert!(matches!(r, Err(Error::IdentityMismatch)));
    assert!(a.engine.security_events(10).unwrap().iter().any(|e| e.kind == "identity_mismatch"));
}

fn offer(path: &str) -> Offer {
    Offer {
        session_id: uuid::Uuid::new_v4().to_string(),
        label: "x".into(),
        items: vec![OfferItem { path: path.into(), size: 4, mtime: None, dir: false }],
    }
}

#[tokio::test]
async fn path_traversal_offers_are_rejected() {
    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    pair(&mut a, &mut b).await;
    let addr: SocketAddr = b.addr().parse().unwrap();
    for evil in ["../escape.txt", "a/../../escape.txt", "/etc/passwd", "a//b"] {
        let mut conn = a.engine.connect_addr(addr, Some(&b.id()), Purpose::Transfer).await.unwrap();
        conn.send(&Message::Offer(offer(evil))).await.unwrap();
        match conn.recv().await.unwrap() {
            Message::Reject { .. } => {}
            other => panic!("{evil}: expected rejection, got {other:?}"),
        }
    }
    assert!(!b.dir.join("escape.txt").exists());
    assert!(b.engine.security_events(20).unwrap().iter().any(|e| e.kind == "invalid_offer"));
}

#[tokio::test]
async fn corrupted_data_fails_sha256_verification() {
    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    pair(&mut a, &mut b).await;
    let addr: SocketAddr = b.addr().parse().unwrap();
    let mut conn = a.engine.connect_addr(addr, Some(&b.id()), Purpose::Transfer).await.unwrap();
    conn.send(&Message::Offer(offer("data.bin"))).await.unwrap();
    match conn.recv().await.unwrap() {
        Message::Accept { offsets } => assert_eq!(offsets, vec![0]),
        other => panic!("expected accept, got {other:?}"),
    }
    conn.send(&Message::FileStart { index: 0, offset: 0 }).await.unwrap();
    write_data(&mut conn.w, b"abcd").await.unwrap();
    // Digest of different content.
    conn.send(&Message::FileEnd { index: 0, sha256: "00".repeat(32) }).await.unwrap();
    conn.send(&Message::Complete).await.unwrap();
    let mut buf = Vec::new();
    match read_frame(&mut conn.r, &mut buf).await.unwrap() {
        Frame::Control(Message::Summary { failed }) => assert_eq!(failed, vec![0]),
        _ => panic!("expected summary"),
    }
    let d = b.wait_incoming_done(10).await;
    assert_eq!(d.status, DeliveryStatus::Failed);
    assert_eq!(d.verified, Some(false));
    assert!(!b.downloads.join("data.bin").exists(), "unverified data must not be kept");
    assert!(b.engine.security_events(20).unwrap().iter().any(|e| e.kind == "integrity_failure"));
}

#[tokio::test]
async fn claiming_a_trusted_id_without_its_key_is_rejected() {
    use droplink_core::identity::Identity;
    use droplink_core::protocol::{read_message, write_message, Hello};

    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    pair(&mut a, &mut b).await;

    // An attacker with its own certificate claims to be the trusted `a`.
    let attacker = Identity::ephemeral().unwrap();
    let tls = droplink_core::tls::build(&attacker).unwrap();
    let tcp = tokio::net::TcpStream::connect(b.addr()).await.unwrap();
    let connector = tokio_rustls::TlsConnector::from(tls.client);
    let name = rustls::pki_types::ServerName::try_from("droplink.local").unwrap();
    let mut stream = connector.connect(name, tcp).await.unwrap();
    let hello = Hello {
        protocol: droplink_core::PROTOCOL_VERSION,
        device_id: a.id(),
        name: "Alpha".into(),
        os: "windows".into(),
        app_version: "x".into(),
        port: 1,
        purpose: Purpose::Transfer,
    };
    write_message(&mut stream, &Message::Hello(hello)).await.unwrap();
    assert!(matches!(read_message(&mut stream).await, Err(Error::IdentityMismatch)));
    assert!(b.engine.security_events(20).unwrap().iter().any(|e| e.kind == "identity_mismatch"));
}
