use assistant_storage::attachment_store::{
    delete_attachment_refs_by_attachment_id, get_all_referenced_attachment_ids,
    insert_attachment_refs, is_plain_file_name, AttachmentError, AttachmentStore,
};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn store() -> (tempfile::TempDir, AttachmentStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = AttachmentStore::new(dir.path().to_path_buf());
    (dir, store)
}

/// 带 `attachment_refs` 表的库：`A1` 的锁内引用校验要真查 DB。
fn seeded() -> (tempfile::TempDir, AttachmentStore, Arc<Mutex<Connection>>) {
    let dir = tempfile::tempdir().unwrap();
    let conn = assistant_storage::db::init_db(&dir.path().join("assistant.db")).unwrap();
    let attachments = dir.path().join("attachments");
    std::fs::create_dir_all(&attachments).unwrap();
    let store = AttachmentStore::new(attachments);
    (dir, store, Arc::new(Mutex::new(conn)))
}

/// 采样时刻取「现在之后」：刚写入的文件 `mtime` 必然早于它，满足执行侧判据。
fn future_sampled_at() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
        .saturating_add(10)
}

fn is_invalid_id(err: &AttachmentError) -> bool {
    matches!(err, AttachmentError::InvalidId(_))
}

#[test]
fn t3_plain_file_name_vectors() {
    assert!(is_plain_file_name("abc.png"));
    assert!(is_plain_file_name("0123456789abcdef0123456789abcdef.pdf"));

    for bad in [
        "",
        ".",
        "..",
        "a/b",
        "a\\b",
        ".hidden",
        "x.tmp",
        "C:x",
        "a:b",
        "abc\u{0}.png",
        "abc\u{1}.png",
        "../abc.png",
    ] {
        assert!(!is_plain_file_name(bad), "应拒绝 id: {:?}", bad);
    }
}

#[tokio::test]
async fn t5_invalid_id_rejected_at_every_entry() {
    let (_guard, store) = store();
    let bad = "../escape.png";

    assert!(is_invalid_id(&store.read(bad).await.unwrap_err()));
    assert!(is_invalid_id(&store.file_path(bad).unwrap_err()));
    assert!(is_invalid_id(
        &store.write_derived_text(bad, "x").await.unwrap_err()
    ));
    assert!(is_invalid_id(
        &store.delete_derived_text(bad).await.unwrap_err()
    ));
    assert!(is_invalid_id(&store.delete(bad).await.unwrap_err()));
}

#[tokio::test]
async fn t5_read_derived_text_returns_none_without_io() {
    let (guard, store) = store();
    assert_eq!(store.read_derived_text("../escape.pdf").await, None);
    let derived = guard.path().join("derived");
    assert!(!derived.exists(), "非法 id 不应触发任何 IO");
}

#[tokio::test]
async fn valid_id_roundtrip_through_file_path() {
    let (guard, store) = store();
    let saved = store
        .save_attachment(b"pdf-bytes", "application/pdf")
        .await
        .unwrap();
    store.write_derived_text(&saved.id, "hello").await.unwrap();
    assert_eq!(
        store.read_derived_text(&saved.id).await.as_deref(),
        Some("hello")
    );
    assert_eq!(
        store.file_path(&saved.id).unwrap(),
        guard.path().join(&saved.id)
    );
    store.delete(&saved.id).await.unwrap();
    assert_eq!(store.read_derived_text(&saved.id).await, None);
    assert!(!guard.path().join(&saved.id).exists());
}

#[tokio::test]
async fn t12_long_path_survives_save_read_delete() {
    let base = tempfile::tempdir().unwrap();
    let mut root = base.path().to_path_buf();
    for _ in 0..6 {
        root.push("a".repeat(40));
    }
    tokio::fs::create_dir_all(&root).await.unwrap();

    let store = AttachmentStore::new(root.clone());
    let data = b"long path payload";
    let saved = store.save_attachment(data, "text/plain").await.unwrap();
    let path = store.file_path(&saved.id).unwrap();

    assert!(
        path.as_os_str().len() > 260,
        "构造的路径应超过 260，实际 {}",
        path.as_os_str().len()
    );
    assert_eq!(path, root.join(&saved.id), "路径不得被截断或改写");
    assert_eq!(store.read(&saved.id).await.unwrap(), data);

    store.delete(&saved.id).await.unwrap();
    assert!(!path.exists());
}

#[tokio::test]
async fn purge_derived_orphans_is_idempotent() {
    let (_guard, store) = store();
    store
        .write_derived_text("orphan.png", "hello")
        .await
        .unwrap();

    let entries = store.list_derived().await.unwrap();
    assert!(!entries.is_empty(), "先造出一条孤儿侧车");
    assert!(entries.iter().all(|entry| entry.orphan), "无基附件 → 孤儿");
    let names: Vec<String> = entries.into_iter().map(|entry| entry.name).collect();

    // 计划采样时间取「现在之后」：写入的 mtime 必然更早，满足执行侧判据。
    let sampled_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
        .saturating_add(10);

    let first = store.purge_derived_orphans(&names, sampled_at).await;
    assert_eq!(first.removed, names.len() as u64);
    assert_eq!(first.failed, 0, "首次执行不应有失败");
    assert_eq!(first.skipped, 0);
    assert!(store.list_derived().await.unwrap().is_empty(), "孤儿已清空");

    let second = store.purge_derived_orphans(&names, sampled_at).await;
    assert_eq!(second.removed, 0, "重复执行不得再删到东西");
    assert_eq!(second.failed, 0, "目标已不存在属判据排除，不是失败");
    assert_eq!(second.skipped, names.len() as u64, "全部计入 skipped");
    assert!(store.list_derived().await.unwrap().is_empty());
}

#[tokio::test]
async fn derived_stats_counts_regular_files_and_skips_companions() {
    let (dir, store) = store();

    let empty = store.derived_stats().await.unwrap();
    assert_eq!(empty.count, 0, "从未抽取时为空态，不是降级");
    assert_eq!(empty.bytes, 0);
    assert_eq!(empty.oldest_at, None, "空目录两端为 None，不是 0");
    assert_eq!(empty.newest_at, None);

    let derived = dir.path().join("derived");
    tokio::fs::create_dir_all(&derived).await.unwrap();
    tokio::fs::write(derived.join("a.1.txt"), b"12345")
        .await
        .unwrap();
    tokio::fs::write(derived.join("b.1.txt"), b"12")
        .await
        .unwrap();
    tokio::fs::write(derived.join("c.1.txt.tmp"), b"partial")
        .await
        .unwrap();

    let stats = store.derived_stats().await.unwrap();
    assert_eq!(stats.count, 2, "伴生 .tmp 不计入条数");
    assert_eq!(stats.bytes, 7, "伴生 .tmp 不计入字节");
    assert!(stats.oldest_at.is_some());
    assert!(stats.newest_at.is_some());
}

/// 符号链接不计入、也不使整条降级；建链接失败时（未开特权）显式跳过。
#[tokio::test]
async fn derived_stats_skips_symlinks() {
    let (dir, store) = store();
    let derived = dir.path().join("derived");
    tokio::fs::create_dir_all(&derived).await.unwrap();
    let outside = dir.path().join("outside.txt");
    tokio::fs::write(&outside, b"0123456789").await.unwrap();
    if !file_link(&outside, &derived.join("a.1.txt")) {
        eprintln!("skip derived_stats_skips_symlinks: cannot create file symlink");
        return;
    }

    let stats = store.derived_stats().await.unwrap();
    assert_eq!(stats.count, 0, "符号链接不计入");
    assert_eq!(stats.bytes, 0);
}

fn file_link(target: &std::path::Path, link: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link);
        false
    }
}

/// V11：`A1`（`delete_orphans_by_ids`）与「导入 + 写引用」并发，不得留下指向已删文件的引用。
///
/// 关键窗口：`save_attachment` 不持 DB 连接锁，`A1` 可以在「文件已写入、引用尚未落库」
/// 之间删掉文件；此时 `insert_attachment_refs` 的锁内文件校验必须让整条写入失败。
///
/// 导入侧的写文件与写引用分两步，这里只让「写引用」与 `A1` 竞争：文件已就位，
/// `A1` 随时可能把它删掉。两边共用同一 ID 锁，结果只能是「删成功 → 引用写不进」
/// 或「引用先落库 → 删被跳过」，绝不允许「文件没了、引用还在」。
#[test]
fn a1_and_writer_never_leave_a_dangling_reference() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (_dir, store, conn) = seeded();

    for round in 0..16u32 {
        let content = format!("payload-{round}");
        let saved = runtime
            .block_on(store.save_attachment(content.as_bytes(), "text/plain"))
            .unwrap();
        let id = saved.id;
        let sampled_at = future_sampled_at();
        let barrier = Arc::new(std::sync::Barrier::new(2));

        let (a_store, a_conn, a_id, a_barrier) = (
            store.clone(),
            Arc::clone(&conn),
            id.clone(),
            Arc::clone(&barrier),
        );
        let reclaim = std::thread::spawn(move || {
            a_barrier.wait();
            let conn = a_conn.lock().unwrap();
            a_store.delete_orphans_by_ids(&conn, &[a_id], sampled_at)
        });

        let (w_store, w_conn, w_id, w_barrier) =
            (store.clone(), Arc::clone(&conn), id.clone(), barrier);
        let writer = std::thread::spawn(move || {
            w_barrier.wait();
            let conn = w_conn.lock().unwrap();
            insert_attachment_refs(&w_store, &conn, &[w_id], "conv-1", "msg-1")
        });

        let report = reclaim.join().unwrap();
        let inserted = writer.join().unwrap();

        let conn = conn.lock().unwrap();
        let path = store.file_path(&id).unwrap();
        let exists = path.exists();
        let referenced = get_all_referenced_attachment_ids(&conn)
            .unwrap()
            .iter()
            .any(|ref_id| ref_id == &id);

        assert!(
            exists || !referenced,
            "round {round}: 不得留下指向已删文件的引用"
        );
        if inserted.is_ok() {
            assert!(exists, "round {round}: 引用写入成功时文件必须存在");
            assert!(referenced, "round {round}: 写入成功的引用必须可见");
        } else {
            // 写入失败只有一种成因：锁内校验时文件已被 `A1` 删掉。
            assert!(
                !exists,
                "round {round}: 引用写入失败说明文件已被删，不得又出现"
            );
            assert_eq!(report.removed, 1, "round {round}: 删除应计入成功");
        }
        assert_eq!(report.failed, 0, "round {round}: 不得出现失败");

        delete_attachment_refs_by_attachment_id(&conn, &id).unwrap();
        drop(conn);
        let _ = std::fs::remove_file(&path);
    }
}

/// V11：同一 ID 串行——持有 ID 锁时 `A1` 不得进入临界区。
#[test]
fn a1_waits_for_the_same_id_guard() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (_dir, store, conn) = seeded();
    let saved = runtime
        .block_on(store.save_attachment(b"guarded payload", "text/plain"))
        .unwrap();
    let id = saved.id;
    let sampled_at = future_sampled_at();

    let guard = store.id_guard(&id);
    let (tx, rx) = std::sync::mpsc::channel();
    let (t_store, t_conn, t_id) = (store.clone(), Arc::clone(&conn), id.clone());
    let worker = std::thread::spawn(move || {
        let conn = t_conn.lock().unwrap();
        let report = t_store.delete_orphans_by_ids(&conn, &[t_id], sampled_at);
        tx.send(report.removed).unwrap();
    });

    assert!(
        matches!(
            rx.recv_timeout(Duration::from_millis(150)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ),
        "持有同一 ID 锁时 A1 必须阻塞"
    );
    drop(guard);

    let removed = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(removed, 1, "释放锁后 A1 应删掉该孤儿");
    worker.join().unwrap();
    assert!(!store.file_path(&id).unwrap().exists());
}

/// V11：`A1` 与 `delete_attachments` 的删除序列（删引用 → 删文件）并发，同一 ID 只删一次。
#[test]
fn a1_and_manual_delete_are_serialized_per_id() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let (_dir, store, conn) = seeded();

    for round in 0..8u32 {
        let saved = runtime
            .block_on(store.save_attachment(format!("manual-{round}").as_bytes(), "text/plain"))
            .unwrap();
        let id = saved.id;
        let sampled_at = future_sampled_at();
        let barrier = Arc::new(std::sync::Barrier::new(2));

        let (a_store, a_conn, a_id, a_barrier) = (
            store.clone(),
            Arc::clone(&conn),
            id.clone(),
            Arc::clone(&barrier),
        );
        let reclaim = std::thread::spawn(move || {
            a_barrier.wait();
            let conn = a_conn.lock().unwrap();
            a_store
                .delete_orphans_by_ids(&conn, &[a_id], sampled_at)
                .removed
        });

        let (m_store, m_conn, m_id, m_barrier) =
            (store.clone(), Arc::clone(&conn), id.clone(), barrier);
        let manual = std::thread::spawn(move || {
            m_barrier.wait();
            let conn = m_conn.lock().unwrap();
            let _guard = m_store.id_guard(&m_id);
            delete_attachment_refs_by_attachment_id(&conn, &m_id).unwrap();
            m_store.delete_blocking(&m_id).unwrap()
        });

        let removed = reclaim.join().unwrap();
        let manual_bytes = manual.join().unwrap();
        assert_eq!(
            removed + u64::from(manual_bytes.is_some()),
            1,
            "round {round}: 同一 ID 只能被删一次"
        );
        assert!(!store.file_path(&id).unwrap().exists());
        let conn = conn.lock().unwrap();
        assert!(
            get_all_referenced_attachment_ids(&conn).unwrap().is_empty(),
            "round {round}: 删完不得留悬空引用"
        );
    }
}
