//! End-to-end file transfer tests over real TLS sockets.

mod common;

use std::time::{Duration, UNIX_EPOCH};

use common::*;
use droplink_core::models::{DeliveryStatus, Direction, HistoryQuery};
use droplink_core::Event;

#[tokio::test]
async fn single_file_is_delivered_and_verified() {
    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    pair(&mut a, &mut b).await;

    let src = a.dir.join("src/report.pdf");
    let digest = write_random(&src, 3 * 1024 * 1024 + 17);
    let old = filetime::FileTime::from_unix_time(1_600_000_000, 0);
    filetime::set_file_mtime(&src, old).unwrap();

    let sid = a.engine.send_files(vec![src], vec![b.id()]).await.unwrap();
    let out = a.wait_outgoing(&sid, 30).await;
    assert_eq!(out.status, DeliveryStatus::Completed, "{:?}", out.error);
    assert_eq!(out.verified, Some(true));
    let inc = b.wait_incoming_done(10).await;
    assert_eq!(inc.status, DeliveryStatus::Completed);
    assert_eq!(inc.verified, Some(true));

    let dst = b.downloads.join("report.pdf");
    assert_eq!(sha_file(&dst), digest);
    let mtime = std::fs::metadata(&dst).unwrap().modified().unwrap();
    assert_eq!(mtime.duration_since(UNIX_EPOCH).unwrap().as_secs(), 1_600_000_000);
    assert!(!b.downloads.join(".droplink-partial").exists(), "partial folder is cleaned up");

    // History on both sides.
    let h = a.engine.history(&HistoryQuery::default()).unwrap();
    assert_eq!(h.len(), 1);
    assert_eq!(h[0].recipient_name, "Bravo");
    assert_eq!(h[0].verified, Some(true));
    let h = b.engine.history(&HistoryQuery { direction: Some("incoming".into()), ..Default::default() }).unwrap();
    assert_eq!(h[0].sender_name, "Alpha");
}

#[tokio::test]
async fn folder_hierarchy_and_empty_folders_are_preserved() {
    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    pair(&mut a, &mut b).await;

    let root = a.dir.join("Project");
    let d1 = write_random(&root.join("src/main.rs"), 1000);
    let d2 = write_random(&root.join("src/deep/nested/lib.rs"), 70_000);
    let d3 = write_random(&root.join("README.md"), 0);
    std::fs::create_dir_all(root.join("empty/inner")).unwrap();

    let sid = a.engine.send_files(vec![root], vec![b.id()]).await.unwrap();
    let out = a.wait_outgoing(&sid, 30).await;
    assert_eq!(out.status, DeliveryStatus::Completed, "{:?}", out.error);
    assert_eq!(out.total_files, 3);

    let dst = b.downloads.join("Project");
    assert_eq!(sha_file(&dst.join("src/main.rs")), d1);
    assert_eq!(sha_file(&dst.join("src/deep/nested/lib.rs")), d2);
    assert_eq!(sha_file(&dst.join("README.md")), d3);
    assert!(dst.join("empty/inner").is_dir());

    let h = a.engine.history(&HistoryQuery::default()).unwrap();
    assert_eq!(h[0].kind, "folder");
}

#[tokio::test]
async fn existing_files_are_kept_unless_overwrite_is_chosen() {
    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    pair(&mut a, &mut b).await;
    std::fs::write(b.downloads.join("notes.txt"), b"original").unwrap();

    let src = a.dir.join("notes.txt");
    std::fs::write(&src, b"incoming").unwrap();
    let sid = a.engine.send_files(vec![src.clone()], vec![b.id()]).await.unwrap();
    assert_eq!(a.wait_outgoing(&sid, 20).await.status, DeliveryStatus::Completed);
    assert_eq!(std::fs::read(b.downloads.join("notes.txt")).unwrap(), b"original");
    assert_eq!(std::fs::read(b.downloads.join("notes (2).txt")).unwrap(), b"incoming");

    b.set(|s| s.conflict_policy = droplink_core::settings::ConflictPolicy::Overwrite).await;
    std::fs::write(&src, b"replacement").unwrap();
    let sid = a.engine.send_files(vec![src], vec![b.id()]).await.unwrap();
    assert_eq!(a.wait_outgoing(&sid, 20).await.status, DeliveryStatus::Completed);
    assert_eq!(std::fs::read(b.downloads.join("notes.txt")).unwrap(), b"replacement");
}

#[tokio::test]
async fn incoming_requests_need_approval_and_can_be_declined() {
    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    pair(&mut a, &mut b).await;
    b.set(|s| s.require_approval = true).await;

    let src = a.dir.join("photo.jpg");
    write_random(&src, 1234);

    // Declined.
    let sid = a.engine.send_files(vec![src.clone()], vec![b.id()]).await.unwrap();
    let req = b
        .wait_for(10, |e| match e {
            Event::IncomingRequest { request } => Some(request.clone()),
            _ => None,
        })
        .await;
    assert_eq!(req.total_files, 1);
    assert_eq!(req.total_bytes, 1234);
    assert_eq!(req.device_name, "Alpha");
    b.engine.respond_incoming(&req.id, false, None, None).unwrap();
    let out = a.wait_outgoing(&sid, 10).await;
    assert_eq!(out.status, DeliveryStatus::Rejected);
    assert!(!b.downloads.join("photo.jpg").exists());

    // Accepted into a folder chosen at approval time.
    let custom = b.dir.join("chosen");
    std::fs::create_dir_all(&custom).unwrap();
    let sid = a.engine.send_files(vec![src], vec![b.id()]).await.unwrap();
    let req = b
        .wait_for(10, |e| match e {
            Event::IncomingRequest { request } => Some(request.clone()),
            _ => None,
        })
        .await;
    b.engine.respond_incoming(&req.id, true, Some(custom.clone()), None).unwrap();
    assert_eq!(a.wait_outgoing(&sid, 20).await.status, DeliveryStatus::Completed);
    assert!(custom.join("photo.jpg").exists());

    // Per-device auto-accept skips the prompt.
    b.engine.set_device_permissions(&a.id(), Some(true), None).unwrap();
    let src2 = a.dir.join("auto.txt");
    std::fs::write(&src2, b"auto").unwrap();
    let sid = a.engine.send_files(vec![src2], vec![b.id()]).await.unwrap();
    assert_eq!(a.wait_outgoing(&sid, 20).await.status, DeliveryStatus::Completed);
}

#[tokio::test]
async fn pause_and_resume_continue_from_partial_data() {
    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    pair(&mut a, &mut b).await;
    // Throttle so the transfer is slow enough to pause mid-way.
    a.set(|s| s.bandwidth_limit_mbps = 4.0).await;

    let src = a.dir.join("video.mov");
    let size = 6 * 1024 * 1024;
    let digest = write_random(&src, size);
    let sid = a.engine.send_files(vec![src], vec![b.id()]).await.unwrap();
    let (did, paused_at) = a
        .wait_for(20, |e| match e {
            Event::DeliveryProgress { progress } if progress.bytes_done > 1_500_000 => {
                Some((progress.delivery_id.clone(), progress.bytes_done))
            }
            _ => None,
        })
        .await;
    a.engine.pause_delivery(&did).unwrap();
    let d = a.wait_outgoing(&sid, 10).await;
    assert_eq!(d.status, DeliveryStatus::Paused);
    let inc = b
        .wait_for(10, |e| match e {
            Event::DeliveryUpdated { delivery }
                if delivery.direction == Direction::Incoming && delivery.status == DeliveryStatus::Paused =>
            {
                Some(delivery.clone())
            }
            _ => None,
        })
        .await;
    assert!(inc.error.unwrap().contains("Paused by Alpha"));
    let partial = b.downloads.join(".droplink-partial").join(&did).join("0.part");
    let kept = std::fs::metadata(&partial).unwrap().len();
    assert!(kept >= 1_000_000 && kept < size as u64, "partial data kept: {kept}");

    a.set(|s| s.bandwidth_limit_mbps = 0.0).await;
    a.drain();
    a.engine.resume_delivery(&did).await.unwrap();
    // The first progress report after resuming starts from the partial data,
    // not from zero.
    let first = a
        .wait_for(10, |e| match e {
            Event::DeliveryProgress { progress } if progress.delivery_id == did => Some(progress.bytes_done),
            _ => None,
        })
        .await;
    assert!(first >= kept.min(paused_at), "resumed at {first}, partial had {kept}");
    let d = a.wait_outgoing(&sid, 30).await;
    assert_eq!(d.status, DeliveryStatus::Completed, "{:?}", d.error);
    assert_eq!(sha_file(&b.downloads.join("video.mov")), digest);
}

#[tokio::test]
async fn transfer_survives_receiver_restart() {
    let mut a = node("Alpha").await;
    let tmp_b = tempfile::tempdir().unwrap();
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let mut b = node_at("Bravo", tmp_b.path(), port).await;
    pair(&mut a, &mut b).await;
    a.set(|s| s.bandwidth_limit_mbps = 4.0).await;

    let src = a.dir.join("archive.zip");
    let digest = write_random(&src, 5 * 1024 * 1024);
    let sid = a.engine.send_files(vec![src], vec![b.id()]).await.unwrap();
    let did = a
        .wait_for(20, |e| match e {
            Event::DeliveryProgress { progress } if progress.bytes_done > 1_000_000 => Some(progress.delivery_id.clone()),
            _ => None,
        })
        .await;

    // The receiving app quits mid-transfer.
    b.engine.shutdown();
    drop(b);
    let d = a.wait_outgoing(&sid, 60).await;
    assert_eq!(d.status, DeliveryStatus::Paused, "interrupted transfer waits: {:?}", d.error);
    assert!(d.error.as_deref().unwrap_or("").contains("Connection lost"));

    // It comes back on the same port with the same identity and partial data.
    a.set(|s| s.bandwidth_limit_mbps = 0.0).await;
    let mut b = node_at("Bravo", tmp_b.path(), port).await;
    let partial = b.downloads.join(".droplink-partial").join(&did).join("0.part");
    assert!(partial.exists(), "partial data survives a restart");
    // Contact from the returning device triggers automatic resumption.
    b.engine.connect_manual(&a.addr()).await.unwrap();
    let d = a.wait_outgoing(&sid, 60).await;
    assert_eq!(d.status, DeliveryStatus::Completed, "{:?}", d.error);
    assert_eq!(sha_file(&b.downloads.join("archive.zip")), digest);
    let inc = b.wait_incoming_done(10).await;
    assert_eq!(inc.verified, Some(true));
}

#[tokio::test]
async fn cancel_discards_partial_data_on_both_sides() {
    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    pair(&mut a, &mut b).await;
    a.set(|s| s.bandwidth_limit_mbps = 4.0).await;

    let src = a.dir.join("big.iso");
    write_random(&src, 4 * 1024 * 1024);
    let sid = a.engine.send_files(vec![src], vec![b.id()]).await.unwrap();
    let did = a
        .wait_for(20, |e| match e {
            Event::DeliveryProgress { progress } if progress.bytes_done > 500_000 => Some(progress.delivery_id.clone()),
            _ => None,
        })
        .await;
    a.engine.cancel_delivery(&did).await.unwrap();
    assert_eq!(a.wait_outgoing(&sid, 10).await.status, DeliveryStatus::Cancelled);
    let inc = b.wait_incoming_done(10).await;
    assert_eq!(inc.status, DeliveryStatus::Cancelled);
    assert!(!b.downloads.join(".droplink-partial").exists());
    assert!(!b.downloads.join("big.iso").exists());
}

#[tokio::test]
async fn multiple_recipients_progress_and_fail_independently() {
    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    let mut c = node("Charlie").await;
    let mut d = node("Delta").await;
    pair(&mut a, &mut b).await;
    pair(&mut a, &mut c).await;
    pair(&mut a, &mut d).await;
    // Delta stops trusting Alpha, so its delivery fails while others succeed.
    d.engine.revoke_device(&a.id()).unwrap();

    let src = a.dir.join("shared");
    let d1 = write_random(&src.join("one.bin"), 2 * 1024 * 1024);
    let d2 = write_random(&src.join("two.bin"), 512 * 1024);
    let sid = a.engine.send_files(vec![src], vec![b.id(), c.id(), d.id()]).await.unwrap();

    let deliveries: Vec<_> = a.engine.list_deliveries(50).unwrap().into_iter().filter(|x| x.session_id == sid).collect();
    assert_eq!(deliveries.len(), 3, "one delivery per recipient");

    let mut results = std::collections::HashMap::new();
    while results.len() < 3 {
        let v = a.wait_outgoing(&sid, 60).await;
        results.insert(v.device_id.clone(), v);
    }
    assert_eq!(results[&b.id()].status, DeliveryStatus::Completed);
    assert_eq!(results[&c.id()].status, DeliveryStatus::Completed);
    assert_eq!(results[&d.id()].status, DeliveryStatus::Failed);
    for n in [&b, &c] {
        assert_eq!(sha_file(&n.downloads.join("shared/one.bin")), d1);
        assert_eq!(sha_file(&n.downloads.join("shared/two.bin")), d2);
    }

    // Retrying only the failed delivery after trust is restored.
    pair(&mut a, &mut d).await;
    let failed = &results[&d.id()];
    a.drain();
    a.engine.resume_delivery(&failed.id).await.unwrap();
    let v = a.wait_outgoing(&sid, 30).await;
    assert_eq!(v.id, failed.id);
    assert_eq!(v.status, DeliveryStatus::Completed, "{:?}", v.error);
    assert_eq!(sha_file(&d.downloads.join("shared/one.bin")), d1);
}

#[tokio::test]
async fn group_send_reaches_only_trusted_members() {
    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    let mut c = node("Charlie").await;
    pair(&mut a, &mut b).await;
    pair(&mut a, &mut c).await;
    let g = a.engine.create_group("Office", "blue", &[b.id(), c.id()]).unwrap();
    a.engine.revoke_device(&c.id()).unwrap();
    assert_eq!(a.engine.group_recipients(&g.id).unwrap(), vec![b.id()]);

    let src = a.dir.join("memo.txt");
    std::fs::write(&src, b"memo").unwrap();
    let sid = a.engine.send_to_group(vec![src], &g.id).await.unwrap();
    let v = a.wait_outgoing(&sid, 20).await;
    assert_eq!(v.device_id, b.id());
    assert_eq!(v.status, DeliveryStatus::Completed);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!c.downloads.join("memo.txt").exists());
    let _ = c.events.try_recv();
}

#[tokio::test]
async fn concurrency_limit_queues_extra_transfers() {
    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    pair(&mut a, &mut b).await;
    a.set(|s| {
        s.max_concurrent_transfers = 1;
        s.bandwidth_limit_mbps = 4.0;
    })
    .await;
    let f1 = a.dir.join("1.bin");
    let f2 = a.dir.join("2.bin");
    write_random(&f1, 4 * 1024 * 1024);
    write_random(&f2, 64 * 1024);
    let s1 = a.engine.send_files(vec![f1], vec![b.id()]).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let s2 = a.engine.send_files(vec![f2], vec![b.id()]).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let list = a.engine.list_deliveries(10).unwrap();
    let second = list.iter().find(|d| d.session_id == s2).unwrap();
    assert_eq!(second.status, DeliveryStatus::Queued, "second transfer waits for a slot");

    let mut done = std::collections::HashSet::new();
    while done.len() < 2 {
        let v = a
            .wait_for(30, |e| match e {
                Event::DeliveryUpdated { delivery } if delivery.status == DeliveryStatus::Completed => {
                    Some(delivery.session_id.clone())
                }
                _ => None,
            })
            .await;
        done.insert(v);
    }
    assert!(done.contains(&s1) && done.contains(&s2));
}

#[tokio::test]
#[ignore = "large transfer; run with --ignored (≈1 GiB of disk)"]
async fn one_gigabyte_file_streams_without_buffering() {
    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    pair(&mut a, &mut b).await;
    let src = a.dir.join("huge.bin");
    let digest = write_random(&src, 1024 * 1024 * 1024);
    let started = std::time::Instant::now();
    let sid = a.engine.send_files(vec![src], vec![b.id()]).await.unwrap();
    let v = a.wait_outgoing(&sid, 600).await;
    assert_eq!(v.status, DeliveryStatus::Completed);
    let secs = started.elapsed().as_secs_f64();
    eprintln!("1 GiB in {secs:.1}s ({:.0} MB/s)", 1024.0 / secs);
    assert_eq!(sha_file(&b.downloads.join("huge.bin")), digest);
}

#[tokio::test]
async fn large_file_transfer() {
    let mut a = node("Alpha").await;
    let mut b = node("Bravo").await;
    pair(&mut a, &mut b).await;
    let src = a.dir.join("large.bin");
    let digest = write_random(&src, 96 * 1024 * 1024);
    let started = std::time::Instant::now();
    let sid = a.engine.send_files(vec![src], vec![b.id()]).await.unwrap();
    let v = a.wait_outgoing(&sid, 180).await;
    assert_eq!(v.status, DeliveryStatus::Completed, "{:?}", v.error);
    eprintln!("96 MiB in {:.2}s", started.elapsed().as_secs_f64());
    assert_eq!(sha_file(&b.downloads.join("large.bin")), digest);
}
