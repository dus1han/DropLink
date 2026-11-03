//! Universal clipboard and text sharing between engines.

mod common;

use std::time::Duration;

use common::*;
use droplink_core::clipboard::ClipContent;
use droplink_core::models::Direction;
use droplink_core::Event;

async fn eventually(secs: u64, mut f: impl FnMut() -> bool) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    while tokio::time::Instant::now() < deadline {
        if f() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

async fn sync_pair() -> (Node, Node) {
    let mut a = node("Mac").await;
    let mut b = node("PC").await;
    pair(&mut a, &mut b).await;
    for n in [&a, &b] {
        n.set(|s| s.clipboard_auto_sync = true).await;
    }
    // Let both watchers take their baseline.
    tokio::time::sleep(Duration::from_millis(900)).await;
    (a, b)
}

#[tokio::test]
async fn clipboard_syncs_both_ways_without_loops() {
    let (a, b) = sync_pair().await;

    a.clip.set(ClipContent::Text("copied on the Mac".into()));
    assert!(eventually(10, || b.clip.get() == Some(ClipContent::Text("copied on the Mac".into()))).await);

    b.clip.set(ClipContent::Text("copied on the PC".into()));
    assert!(eventually(10, || a.clip.get() == Some(ClipContent::Text("copied on the PC".into()))).await);

    // Let several watcher cycles pass: nothing may bounce back and forth.
    let (wa, wb) = (a.clip.writes(), b.clip.writes());
    tokio::time::sleep(Duration::from_millis(3000)).await;
    assert_eq!(a.clip.writes(), wa, "no echo on the Mac");
    assert_eq!(b.clip.writes(), wb, "no echo on the PC");

    let hist = b.engine.clipboard_history(10).unwrap();
    assert!(hist.iter().any(|e| e.origin == "remote" && e.text.as_deref() == Some("copied on the Mac")));
    assert!(hist.iter().any(|e| e.origin == "local" && e.text.as_deref() == Some("copied on the PC")));
}

#[tokio::test]
async fn images_sync_when_enabled() {
    let (a, b) = sync_pair().await;
    let img = ClipContent::Image { width: 16, height: 8, rgba: (0..16 * 8 * 4).map(|i| (i % 251) as u8).collect() };
    a.clip.set(img.clone());
    assert!(eventually(10, || b.clip.get() == Some(img.clone())).await);

    b.set(|s| s.clipboard_images = false).await;
    let img2 = ClipContent::Image { width: 2, height: 2, rgba: vec![9; 16] };
    a.clip.set(img2.clone());
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert_eq!(b.clip.get(), Some(img));
}

#[tokio::test]
async fn sensitive_and_unpermitted_content_is_not_synced() {
    let (a, b) = sync_pair().await;

    a.clip.set_sensitive(ClipContent::Text("password123".into()));
    tokio::time::sleep(Duration::from_millis(2000)).await;
    assert_ne!(b.clip.get(), Some(ClipContent::Text("password123".into())));
    assert!(a.engine.clipboard_history(10).unwrap().iter().all(|e| e.text.as_deref() != Some("password123")));

    // Receiver disables clipboard sync for the sender.
    b.engine.set_device_permissions(&a.id(), None, Some(false)).unwrap();
    a.clip.set(ClipContent::Text("should not arrive".into()));
    tokio::time::sleep(Duration::from_millis(2000)).await;
    assert_ne!(b.clip.get(), Some(ClipContent::Text("should not arrive".into())));

    // Automatic sync off on the sender: nothing is sent at all.
    b.engine.set_device_permissions(&a.id(), None, Some(true)).unwrap();
    a.set(|s| s.clipboard_auto_sync = false).await;
    a.clip.set(ClipContent::Text("auto sync is off".into()));
    tokio::time::sleep(Duration::from_millis(2000)).await;
    assert_ne!(b.clip.get(), Some(ClipContent::Text("auto sync is off".into())));

    // A manual send still works.
    let r = a.engine.send_clipboard(vec![b.id()]).await.unwrap();
    assert!(r[0].ok, "{:?}", r[0].error);
    assert_eq!(b.clip.get(), Some(ClipContent::Text("auto sync is off".into())));
}

#[tokio::test]
async fn untrusted_devices_cannot_push_clipboard() {
    let a = node("Mac").await;
    let b = node("PC").await;
    b.set(|s| s.clipboard_auto_sync = true).await;
    a.engine.connect_manual(&b.addr()).await.unwrap();
    a.clip.set(ClipContent::Text("hello".into()));
    let r = a.engine.send_clipboard(vec![b.id()]).await.unwrap();
    assert!(!r[0].ok);
    assert_eq!(b.clip.get(), None);
}

#[tokio::test]
async fn text_messages_reach_every_recipient() {
    let mut a = node("Mac").await;
    let mut b = node("PC").await;
    let mut c = node("Laptop").await;
    pair(&mut a, &mut b).await;
    pair(&mut a, &mut c).await;

    let text = "Meeting notes:\n  • item one\n  • item two\n\nhttps://example.com".to_string();
    let results = a.engine.send_text(text.clone(), vec![b.id(), c.id()]).await.unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| r.ok));

    for n in [&mut b, &mut c] {
        let m = n
            .wait_for(10, |e| match e {
                Event::TextReceived { message } => Some(message.clone()),
                _ => None,
            })
            .await;
        assert_eq!(m.content, text, "formatting and whitespace are preserved");
        assert_eq!(m.device_name, "Mac");
        assert_eq!(m.direction, Direction::Incoming);
    }
    assert_eq!(a.engine.list_texts(10).unwrap().len(), 2);
    let h = b.engine.history(&Default::default()).unwrap();
    assert_eq!(h[0].kind, "text");
}
