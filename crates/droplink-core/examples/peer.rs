//! Headless DropLink peer for manual and end-to-end testing.
//!
//! Runs a full engine (mDNS discovery, TLS listener) with its own identity
//! and data folder, so you can exercise the desktop app against a second
//! device on the same machine:
//!
//! ```text
//! cargo run -p droplink-core --example peer -- --name "Test Peer" --dir ./target/peer
//! ```
//!
//! Options:
//!   --name <name>       advertised device name (default "DropLink Peer")
//!   --dir <path>        state and download folder (default ./target/peer)
//!   --accept-pairing    confirm pairing codes automatically (prints the code)
//!   --send <path>       after pairing, send this file or folder to the paired device
//!   --text <message>    after pairing, send this text to the paired device
//!
//! Incoming transfers from trusted devices are accepted automatically and
//! saved to `<dir>/downloads`.

use std::path::PathBuf;
use std::sync::Arc;

use droplink_core::clipboard::MemoryClipboard;
use droplink_core::engine::ClipboardMode;
use droplink_core::events::{ChannelSink, PairingRole};
use droplink_core::{Engine, EngineConfig, Event};

#[tokio::main]
async fn main() -> droplink_core::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info,mdns_sd=warn".into()))
        .init();

    let mut name = "DropLink Peer".to_string();
    let mut dir = PathBuf::from("target/peer");
    let mut accept_pairing = false;
    let mut send: Option<PathBuf> = None;
    let mut text: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--name" => name = args.next().expect("--name needs a value"),
            "--dir" => dir = PathBuf::from(args.next().expect("--dir needs a value")),
            "--accept-pairing" => accept_pairing = true,
            "--send" => send = Some(PathBuf::from(args.next().expect("--send needs a path"))),
            "--text" => text = Some(args.next().expect("--text needs a value")),
            other => panic!("unknown argument {other}"),
        }
    }

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut cfg = EngineConfig::new(dir.join("data"), "peer");
    cfg.clipboard = ClipboardMode::Custom(Arc::new(MemoryClipboard::default()));
    cfg.port_override = Some(0);
    let engine = Engine::start(cfg, Arc::new(ChannelSink(tx))).await?;
    let mut s = engine.settings();
    s.device_name = name.clone();
    s.download_dir = dir.join("downloads");
    s.require_approval = false;
    std::fs::create_dir_all(&s.download_dir)?;
    engine.update_settings(s).await?;
    println!("peer \"{name}\" listening on port {} — id {}", engine.port(), &engine.device_id()[..16]);

    while let Some(ev) = rx.recv().await {
        match &ev {
            Event::DeviceDiscovered { device } => println!("discovered {} ({})", device.name, device.os),
            Event::PairingCode { pairing_id, device_name, code, role, .. } => {
                println!("pairing with {device_name}: code {code} ({role:?})");
                if accept_pairing {
                    engine.respond_pairing(pairing_id, true)?;
                } else if *role == PairingRole::Responder {
                    println!("  (run with --accept-pairing to confirm automatically)");
                    engine.respond_pairing(pairing_id, false)?;
                }
            }
            Event::PairingFinished { device_id, success, message, .. } => {
                println!("pairing finished: {success} — {message}");
                if *success {
                    if let Some(p) = send.take() {
                        let sid = engine.send_files(vec![p], vec![device_id.clone()]).await?;
                        println!("sending session {sid}");
                    }
                    if let Some(t) = text.take() {
                        for r in engine.send_text(t, vec![device_id.clone()]).await? {
                            println!("text to {}: {}", r.device_name, if r.ok { "ok" } else { "failed" });
                        }
                    }
                }
            }
            Event::DeliveryUpdated { delivery } => println!(
                "{:?} {} {} — {:?}{}",
                delivery.direction,
                delivery.label,
                delivery.device_name,
                delivery.status,
                delivery.error.as_ref().map(|e| format!(" ({e})")).unwrap_or_default()
            ),
            Event::TextReceived { message } => println!("text from {}: {}", message.device_name, message.content),
            _ => {}
        }
    }
    Ok(())
}
