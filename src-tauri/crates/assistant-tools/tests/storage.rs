use std::path::Path;

use assistant_tools::storage::{
    cleanup_expired_results, persisted_reference, select_persist_targets, session_namespace,
    CleanupReport, FileToolResultStorage, InMemoryReadReceiptStore, PersistToolResult, ReadReceipt,
    ReadReceiptKey, ReadReceiptStore, ToolResultStorage, PER_ROUND_LIMIT_CHARS,
    PER_TOOL_LIMIT_CHARS, PREVIEW_CHARS, PREVIEW_HEAD_CHARS, PREVIEW_TAIL_CHARS,
};

#[test]
fn small_results_stay_inline() {
    let targets = select_persist_targets(&[100, 200, 300]);
    assert_eq!(targets, vec![false, false, false]);
}

#[test]
fn oversized_result_persists_even_below_round_budget() {
    let targets = select_persist_targets(&[PER_TOOL_LIMIT_CHARS + 1]);
    assert_eq!(
        targets,
        vec![true],
        "per-tool limit is independent of the round budget"
    );
}

#[test]
fn round_budget_evicts_the_minimum_number_of_results() {
    let sizes = [40_000, 45_000, 44_000, 43_000, 42_000];
    let targets = select_persist_targets(&sizes);

    assert_eq!(
        targets.iter().filter(|flag| **flag).count(),
        1,
        "only one result needs to leave the inline budget"
    );
    assert_eq!(
        targets,
        vec![true, false, false, false, false],
        "descending accumulation evicts the smallest result last"
    );
}

#[test]
fn equal_sizes_evict_in_stable_order() {
    let sizes = [45_000; 5];
    let targets = select_persist_targets(&sizes);

    assert_eq!(targets.iter().filter(|flag| **flag).count(), 1);
    assert_eq!(
        targets,
        vec![false, false, false, false, true],
        "stable sort keeps original order for ties, so the tail is evicted"
    );
}

const _: () = assert!(45_000 * 5 > PER_ROUND_LIMIT_CHARS);

#[test]
fn preview_keeps_head_and_tail_with_omission_marker() {
    let content = "y".repeat(5_000);
    let rendered = persisted_reference("out.txt", &content);
    let lines: Vec<&str> = rendered.split('\n').collect();

    assert!(rendered.starts_with("<persisted-output path=\"out.txt\">\n"));
    assert_eq!(lines.len(), 4, "tag / head / marker / tail");
    assert_eq!(lines[1].chars().count(), PREVIEW_HEAD_CHARS);
    assert_eq!(lines[3].chars().count(), PREVIEW_TAIL_CHARS);
    assert_eq!(
        lines[2],
        format!(
            "[… {} chars omitted; full output in the file above …]",
            5_000 - PREVIEW_CHARS
        )
    );
    assert!(
        !lines[2].contains("[truncated:"),
        "存储预览省略不得复用采集侧截断语法"
    );
}

#[test]
fn short_result_is_returned_verbatim_without_marker() {
    let content = "z".repeat(PREVIEW_CHARS);
    let rendered = persisted_reference("out.txt", &content);

    assert_eq!(
        rendered,
        format!("<persisted-output path=\"out.txt\">\n{}", content),
        "未超过预览预算时不得出现省略标记"
    );
}

#[test]
fn omission_count_is_exact_one_char_over_budget() {
    let content = "a".repeat(PREVIEW_CHARS + 1);
    let rendered = persisted_reference("out.txt", &content);

    assert!(
        rendered.contains("[… 1 chars omitted; full output in the file above …]"),
        "越界 1 字符时省略数必须为 1"
    );
}

#[test]
fn preview_splits_multibyte_content_by_char() {
    let content = "中".repeat(PREVIEW_CHARS + 10);
    let rendered = persisted_reference("out.txt", &content);

    let lines: Vec<&str> = rendered.split('\n').collect();
    assert_eq!(lines[1].chars().count(), PREVIEW_HEAD_CHARS);
    assert_eq!(lines[3].chars().count(), PREVIEW_TAIL_CHARS);
    assert!(rendered.contains("[… 10 chars omitted; full output in the file above …]"));
}

#[test]
fn tail_keeps_tool_footer_parseable_as_truncation() {
    let mut content = "x".repeat(PER_TOOL_LIMIT_CHARS);
    content.push_str("\n[truncated: showing 30 of 159 items; narrow the pattern]");
    let rendered = persisted_reference("out.txt", &content);

    let truncated = assistant_tools::compact::parse_truncation(&rendered)
        .expect("尾部 footer 必须落在预览尾部窗口内");
    assert_eq!(truncated.returned, Some(30));
    assert_eq!(truncated.total, Some(159));
}

#[tokio::test]
async fn file_storage_writes_the_full_content() {
    let dir = tempfile::tempdir().unwrap();
    let storage = FileToolResultStorage::new(dir.path());
    let content = "z".repeat(1_000);

    let reference = storage
        .persist(PersistToolResult {
            tool_call_id: "call/with:bad*chars".to_string(),
            tool_name: "probe".to_string(),
            session_id: Some("session-1".to_string()),
            content: content.clone(),
        })
        .await
        .unwrap();

    let written = std::fs::read_to_string(&reference.path).unwrap();
    assert_eq!(written, content);
    assert_eq!(reference.bytes, content.len());
    assert!(reference.path.ends_with(".txt"));
}

#[tokio::test]
async fn file_storage_namespaces_by_session_and_prefixes_names() {
    let dir = tempfile::tempdir().unwrap();
    let storage = FileToolResultStorage::new(dir.path());

    let first = storage
        .persist(PersistToolResult {
            tool_call_id: "call-1".to_string(),
            tool_name: "read".to_string(),
            session_id: Some("session-a".to_string()),
            content: "a".to_string(),
        })
        .await
        .unwrap();
    let second = storage
        .persist(PersistToolResult {
            tool_call_id: "call-1".to_string(),
            tool_name: "read".to_string(),
            session_id: Some("session-a".to_string()),
            content: "b".to_string(),
        })
        .await
        .unwrap();
    let other_session = storage
        .persist(PersistToolResult {
            tool_call_id: "call-1".to_string(),
            tool_name: "read".to_string(),
            session_id: Some("session-b".to_string()),
            content: "c".to_string(),
        })
        .await
        .unwrap();

    let namespace_of = |path: &str| {
        Path::new(path)
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    };
    assert_eq!(
        namespace_of(&first.path),
        session_namespace(Some("session-a"))
    );
    assert_eq!(
        namespace_of(&other_session.path),
        session_namespace(Some("session-b"))
    );
    assert_ne!(namespace_of(&first.path), namespace_of(&other_session.path));
    // 同会话同调用 id 也不覆盖：随机前缀保证文件名唯一。
    assert_ne!(first.path, second.path);
    assert_eq!(std::fs::read_to_string(&first.path).unwrap(), "a");
    assert_eq!(std::fs::read_to_string(&second.path).unwrap(), "b");
    let name = Path::new(&first.path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(
        name.ends_with("-read-call-1.txt"),
        "unexpected name: {name}"
    );
}

#[test]
fn session_namespace_is_stable_and_buckets_anonymous_calls() {
    assert_eq!(
        session_namespace(Some("abc")),
        session_namespace(Some("abc"))
    );
    assert_ne!(
        session_namespace(Some("abc")),
        session_namespace(Some("abd"))
    );
    assert_eq!(session_namespace(None), "shared");
    assert_eq!(session_namespace(Some("")), "shared");
}

fn write_file(path: &Path, content: &str) {
    std::fs::write(path, content).unwrap();
}

fn set_mtime_days_ago(path: &Path, days: u64) {
    let age = std::time::Duration::from_secs(days * 86_400);
    let mtime = std::time::SystemTime::now() - age;
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(mtime).unwrap();
}

#[test]
fn cleanup_removes_expired_files_and_keeps_fresh_ones() {
    let dir = tempfile::tempdir().unwrap();
    let expired = dir.path().join("old.txt");
    let fresh = dir.path().join("new.txt");
    write_file(&expired, "old");
    write_file(&fresh, "new");
    set_mtime_days_ago(&expired, 40);

    let report = cleanup_expired_results(dir.path(), 30);

    assert_eq!(report.removed, 1);
    assert!(!expired.exists());
    assert!(fresh.exists());
}

#[test]
fn cleanup_disabled_when_retention_is_zero() {
    let dir = tempfile::tempdir().unwrap();
    let expired = dir.path().join("old.txt");
    write_file(&expired, "old");
    set_mtime_days_ago(&expired, 400);

    let report = cleanup_expired_results(dir.path(), 0);

    assert_eq!(report, CleanupReport::default());
    assert!(expired.exists());
}

#[test]
fn cleanup_sweeps_namespace_directories_and_keeps_empty_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let namespace = dir.path().join(session_namespace(Some("s1")));
    std::fs::create_dir_all(&namespace).unwrap();
    let expired = namespace.join("old.txt");
    write_file(&expired, "old");
    set_mtime_days_ago(&expired, 60);

    let report = cleanup_expired_results(dir.path(), 30);

    assert_eq!(report.removed, 1);
    assert!(!expired.exists());
    // OQ6 定稿：只删文件，不修剪空目录。
    assert!(namespace.is_dir());
}

#[test]
fn cleanup_counts_failures_without_panicking_on_missing_root() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("does-not-exist");

    let report = cleanup_expired_results(&missing, 30);

    assert_eq!(report.removed, 0);
    assert_eq!(report.failed, 1);
}

#[cfg(unix)]
#[test]
fn cleanup_does_not_follow_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let target_dir = tempfile::tempdir().unwrap();
    let target = target_dir.path().join("outside.txt");
    write_file(&target, "keep me");
    set_mtime_days_ago(&target, 400);

    let link = dir.path().join("link.txt");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let report = cleanup_expired_results(dir.path(), 30);

    assert_eq!(report.removed, 0);
    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
    assert!(target.exists());
}

fn receipt_key(session: Option<&str>, path: &str) -> ReadReceiptKey {
    ReadReceiptKey {
        session_id: session.map(str::to_string),
        path: path.to_string(),
    }
}

fn receipt(revision: &str, tool_call_id: &str) -> ReadReceipt {
    ReadReceipt::utf8(revision, tool_call_id)
}

#[tokio::test]
async fn receipt_lookup_is_isolated_by_session() {
    let store = InMemoryReadReceiptStore::new();
    let recorded = receipt("1:1", "call-1");
    store
        .record(
            receipt_key(Some("session-1"), "/repo/file.txt"),
            recorded.clone(),
        )
        .await;

    assert_eq!(
        store
            .lookup(&receipt_key(Some("session-1"), "/repo/file.txt"))
            .await,
        Some(recorded.clone())
    );
    assert_eq!(
        store
            .lookup(&receipt_key(Some("session-2"), "/repo/file.txt"))
            .await,
        None
    );
    assert_eq!(
        store.lookup(&receipt_key(None, "/repo/file.txt")).await,
        None
    );
}

#[tokio::test]
async fn toolbox_calls_share_a_single_session_bucket() {
    let store = InMemoryReadReceiptStore::new();
    let recorded = receipt("2:2", "call-2");
    store
        .record(receipt_key(None, "/repo/file.txt"), recorded.clone())
        .await;

    assert_eq!(
        store.lookup(&receipt_key(None, "/repo/file.txt")).await,
        Some(recorded)
    );
}

#[tokio::test]
async fn receipt_carries_encoding_and_bom_state() {
    let store = InMemoryReadReceiptStore::new();
    let recorded = ReadReceipt {
        revision: "4:4".to_string(),
        tool_call_id: "call-4".to_string(),
        encoding: "GBK".to_string(),
        has_bom: true,
    };
    store
        .record(receipt_key(None, "/repo/gbk.txt"), recorded.clone())
        .await;

    assert_eq!(
        store.lookup(&receipt_key(None, "/repo/gbk.txt")).await,
        Some(recorded)
    );
}

#[tokio::test]
async fn invalidate_path_removes_subtree_but_not_similar_prefix() {
    let store = InMemoryReadReceiptStore::new();
    let recorded = receipt("3:3", "call-3");
    store
        .record(receipt_key(None, "/a/b/f1.txt"), recorded.clone())
        .await;
    store
        .record(receipt_key(None, "/a/b/sub/f2.txt"), recorded.clone())
        .await;
    store
        .record(receipt_key(None, "/a/bc/f3.txt"), recorded.clone())
        .await;

    store.invalidate_path(Path::new("/a/b")).await;

    assert_eq!(store.lookup(&receipt_key(None, "/a/b/f1.txt")).await, None);
    assert_eq!(
        store.lookup(&receipt_key(None, "/a/b/sub/f2.txt")).await,
        None
    );
    assert_eq!(
        store.lookup(&receipt_key(None, "/a/bc/f3.txt")).await,
        Some(recorded)
    );
}
