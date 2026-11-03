#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use droplink_core::clipboard::MemoryClipboard;
use droplink_core::engine::ClipboardMode;
use droplink_core::events::ChannelSink;
use droplink_core::models::{DeliveryStatus, DeliveryView, Direction};
use droplink_core::{Engine, EngineConfig, Event};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

pub struct Node {
    pub engine: Engine,
    pub events: UnboundedReceiver<Event>,
    pub clip: Arc<MemoryClipboard>,
    pub dir: PathBuf,
    pub downloads: PathBuf,
    _tmp: Option<tempfile::TempDir>,
}

pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".into()))
        .with_test_writer()
        .try_init();
}

pub async fn node(name: &str) -> Node {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_path_buf();
    let mut n = node_at(name, &dir, 0).await;
    n._tmp = Some(tmp);
    n
}

/// Starts (or restarts) a node whose state lives in `dir`.
pub async fn node_at(name: &str, dir: &Path, port: u16) -> Node {
    init_tracing();
    let downloads = dir.join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    let clip = Arc::new(MemoryClipboard::default());
    let (tx, rx) = unbounded_channel();
    let cfg = EngineConfig {
        data_dir: dir.join("data"),
        app_version: "test".into(),
        discovery: false,
        clipboard: ClipboardMode::Custom(clip.clone()),
        port_override: Some(port),
        loopback_only: true,
        ephemeral: false,
    };
    let engine = Engine::start(cfg, Arc::new(ChannelSink(tx))).await.unwrap();
    let mut s = engine.settings();
    s.device_name = name.into();
    s.download_dir = downloads.clone();
    s.require_approval = false;
    engine.update_settings(s).await.unwrap();
    Node { engine, events: rx, clip, dir: dir.to_path_buf(), downloads, _tmp: None }
}

impl Node {
    pub fn id(&self) -> String {
        self.engine.device_id().to_string()
    }

    pub fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.engine.port())
    }

    pub async fn set(&self, f: impl FnOnce(&mut droplink_core::Settings)) {
        let mut s = self.engine.settings();
        f(&mut s);
        self.engine.update_settings(s).await.unwrap();
    }

    /// Waits for an event matching `pred`, discarding others.
    pub async fn wait_for<T>(&mut self, secs: u64, mut pred: impl FnMut(&Event) -> Option<T>) -> T {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
        loop {
            let ev = tokio::time::timeout_at(deadline, self.events.recv())
                .await
                .expect("timed out waiting for event")
                .expect("event channel closed");
            if let Some(v) = pred(&ev) {
                return v;
            }
        }
    }

    /// Waits until a delivery of `session_id` (outgoing) reaches a terminal or
    /// paused state.
    pub async fn wait_outgoing(&mut self, session_id: &str, secs: u64) -> DeliveryView {
        let sid = session_id.to_string();
        self.wait_for(secs, |e| match e {
            Event::DeliveryUpdated { delivery }
                if delivery.session_id == sid
                    && (delivery.status.is_terminal() || delivery.status == DeliveryStatus::Paused) =>
            {
                Some(delivery.clone())
            }
            _ => None,
        })
        .await
    }

    pub async fn wait_incoming_done(&mut self, secs: u64) -> DeliveryView {
        self.wait_for(secs, |e| match e {
            Event::DeliveryUpdated { delivery }
                if delivery.direction == Direction::Incoming && delivery.status.is_terminal() =>
            {
                Some(delivery.clone())
            }
            _ => None,
        })
        .await
    }

    pub fn drain(&mut self) {
        while self.events.try_recv().is_ok() {}
    }
}

/// Connects `a` to `b` by address and completes pairing on both sides.
pub async fn pair(a: &mut Node, b: &mut Node) {
    a.engine.connect_manual(&b.addr()).await.unwrap();
    let pairing = a.engine.pair(&b.id()).unwrap();
    let (code_a, pid_a) = a
        .wait_for(10, |e| match e {
            Event::PairingCode { code, pairing_id, .. } => Some((code.clone(), pairing_id.clone())),
            _ => None,
        })
        .await;
    assert_eq!(pid_a, pairing);
    let (code_b, pid_b) = b
        .wait_for(10, |e| match e {
            Event::PairingCode { code, pairing_id, .. } => Some((code.clone(), pairing_id.clone())),
            _ => None,
        })
        .await;
    assert_eq!(code_a, code_b, "both devices must show the same code");
    a.engine.respond_pairing(&pid_a, true).unwrap();
    b.engine.respond_pairing(&pid_b, true).unwrap();
    for n in [&mut *a, &mut *b] {
        let ok = n
            .wait_for(10, |e| match e {
                Event::PairingFinished { success, .. } => Some(*success),
                _ => None,
            })
            .await;
        assert!(ok, "pairing should succeed");
    }
}

pub fn write_random(path: &Path, size: usize) -> String {
    use rand::RngCore;
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    let mut data = vec![0u8; size];
    rand::thread_rng().fill_bytes(&mut data);
    std::fs::write(path, &data).unwrap();
    hex::encode(Sha256::digest(&data))
}

pub fn sha_file(path: &Path) -> String {
    use std::io::Read;
    let mut f = std::fs::File::open(path).unwrap();
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    hex::encode(h.finalize())
}
