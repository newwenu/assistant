use std::path::Path;

use assistant_tools::storage::{
    cleanup_expired_results, collect_expired, delete_candidates, persisted_reference,
    persisted_reference_from_tail, result_stats, select_persist_targets, session_namespace,
    CleanupReport, FileToolResultStorage, InMemoryReadReceiptStore, OutputStream,
    PersistToolResult, ReadReceipt, ReadReceiptKey, ReadReceiptStore, ToolResultStorage,
    MAX_SPILL_BYTES, PER_ROUND_LIMIT_CHARS, PER_TOOL_LIMIT_CHARS, PREVIEW_CHARS,
    PREVIEW_HEAD_CHARS, PREVIEW_TAIL_CHARS, SPILL_TRUNCATED_MARKER, STDERR_RING_CHARS,
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

fn sink_request(call_id: &str, session: Option<&str>) -> PersistToolResult {
    PersistToolResult {
        tool_call_id: call_id.to_string(),
        tool_name: "shell".to_string(),
        session_id: session.map(str::to_string),
        content: String::new(),
    }
}

#[tokio::test]
async fn sink_preview_keeps_only_the_tail() {
    let dir = tempfile::tempdir().unwrap();
    let storage = FileToolResultStorage::new(dir.path());
    let content = format!("HEAD{}TAIL", "x".repeat(4_000));
    let mut sink = storage
        .open_sink(sink_request("call-1", Some("session-a")))
        .await
        .unwrap();

    sink.write(OutputStream::Stdout, content.as_bytes())
        .unwrap();
    let spilled = sink.finish().unwrap();

    let reference = spilled.reference.expect("non-empty output must persist");
    assert_eq!(std::fs::read_to_string(&reference.path).unwrap(), content);
    assert_eq!(reference.bytes, content.len());
    assert_eq!(spilled.total_chars, 4_008);
    let expected_tail: String = content.chars().skip(4_008 - PREVIEW_CHARS).collect();
    assert_eq!(
        spilled.preview, expected_tail,
        "preview must be the file tail, not head + tail"
    );
    assert!(
        !spilled.preview.contains("HEAD"),
        "head window must not be included"
    );
    assert_eq!(spilled.output_bytes, content.len());
    assert_eq!(spilled.output_lines, 1);
}

#[tokio::test]
async fn sink_short_output_preview_is_the_whole_file() {
    let dir = tempfile::tempdir().unwrap();
    let storage = FileToolResultStorage::new(dir.path());
    let mut sink = storage
        .open_sink(sink_request("call-2", None))
        .await
        .unwrap();

    sink.write(OutputStream::Stdout, b"hello\nworld\n").unwrap();
    let spilled = sink.finish().unwrap();

    assert_eq!(spilled.preview, "hello\nworld\n");
    assert_eq!(spilled.total_chars, 12);
    assert_eq!(spilled.output_lines, 2);
    assert!(!spilled.truncated);
    assert!(!spilled.stderr_truncated);
}

#[tokio::test]
async fn sink_empty_output_removes_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let storage = FileToolResultStorage::new(dir.path());
    let sink = storage
        .open_sink(sink_request("call-3", Some("session-b")))
        .await
        .unwrap();

    let spilled = sink.finish().unwrap();

    assert!(spilled.reference.is_none(), "empty output must not persist");
    assert!(spilled.preview.is_empty());
    assert_eq!(spilled.total_chars, 0);
    assert_eq!(spilled.output_bytes, 0);
    let namespace = storage
        .namespace_root(Some("session-b"))
        .expect("file storage always exposes a namespace root");
    let leftovers: Vec<_> = std::fs::read_dir(&namespace)
        .map(|entries| entries.filter_map(Result::ok).collect())
        .unwrap_or_default();
    assert!(leftovers.is_empty(), "0-byte file must be deleted");
}

#[tokio::test]
async fn sink_appends_stderr_section_and_preview_ends_with_it() {
    let dir = tempfile::tempdir().unwrap();
    let storage = FileToolResultStorage::new(dir.path());
    let mut sink = storage
        .open_sink(sink_request("call-4", Some("session-c")))
        .await
        .unwrap();

    sink.write(OutputStream::Stdout, b"building...\n").unwrap();
    sink.write(OutputStream::Stderr, b"error: boom\n").unwrap();
    let spilled = sink.finish().unwrap();

    let reference = spilled.reference.expect("output must persist");
    let file = std::fs::read_to_string(&reference.path).unwrap();
    assert_eq!(file, "building...\n--- stderr ---\nerror: boom\n");
    assert!(
        spilled.preview.ends_with("error: boom\n"),
        "tail must land on the stderr last line: {:?}",
        spilled.preview
    );
    assert_eq!(spilled.output_bytes, 12 + 12);
    assert_eq!(spilled.output_lines, 2);
}

#[tokio::test]
async fn sink_decodes_multibyte_chars_split_across_chunks() {
    let dir = tempfile::tempdir().unwrap();
    let storage = FileToolResultStorage::new(dir.path());
    let mut sink = storage
        .open_sink(sink_request("call-5", None))
        .await
        .unwrap();

    let bytes = "中文".as_bytes();
    sink.write(OutputStream::Stdout, &bytes[..4]).unwrap();
    sink.write(OutputStream::Stdout, &bytes[4..]).unwrap();
    let spilled = sink.finish().unwrap();

    assert_eq!(
        spilled.total_chars, 2,
        "a split char must not be counted twice"
    );
    assert_eq!(spilled.preview, "中文");
    assert_eq!(spilled.output_bytes, 6);
}

#[tokio::test]
async fn sink_marks_stdout_truncation_in_file_and_preview() {
    let dir = tempfile::tempdir().unwrap();
    let storage = FileToolResultStorage::new(dir.path());
    let mut sink = storage
        .open_sink(sink_request("call-6", None))
        .await
        .unwrap();

    let chunk = vec![b'a'; 1024 * 1024];
    let total = MAX_SPILL_BYTES + 1024 * 1024;
    let mut written = 0;
    while written < total {
        sink.write(OutputStream::Stdout, &chunk).unwrap();
        written += chunk.len();
    }
    let spilled = sink.finish().unwrap();

    assert!(spilled.truncated);
    assert_eq!(spilled.output_bytes, total, "total bytes stay exact");
    let reference = spilled.reference.expect("output must persist");
    assert_eq!(
        reference.bytes,
        MAX_SPILL_BYTES + 1 + SPILL_TRUNCATED_MARKER.len()
    );
    assert!(
        spilled.preview.ends_with(SPILL_TRUNCATED_MARKER),
        "marker must land at the preview tail"
    );
    assert_eq!(
        spilled.preview.matches(SPILL_TRUNCATED_MARKER).count(),
        1,
        "marker must not be duplicated"
    );
}

#[tokio::test]
async fn sink_keeps_the_truncation_marker_when_stderr_pushes_it_out() {
    let dir = tempfile::tempdir().unwrap();
    let storage = FileToolResultStorage::new(dir.path());
    let mut sink = storage
        .open_sink(sink_request("call-7", None))
        .await
        .unwrap();

    let chunk = vec![b'a'; 1024 * 1024];
    let total = MAX_SPILL_BYTES + 1024 * 1024;
    let mut written = 0;
    while written < total {
        sink.write(OutputStream::Stdout, &chunk).unwrap();
        written += chunk.len();
    }
    sink.write(OutputStream::Stderr, &vec![b'e'; PREVIEW_CHARS + 100])
        .unwrap();
    let spilled = sink.finish().unwrap();

    assert!(spilled.truncated);
    assert!(
        spilled.preview.ends_with(SPILL_TRUNCATED_MARKER),
        "stderr must not hide the truncation marker from the tail"
    );
    assert_eq!(spilled.preview.matches(SPILL_TRUNCATED_MARKER).count(), 1);
    let reference = spilled.reference.expect("output must persist");
    let file = std::fs::read_to_string(&reference.path).unwrap();
    assert_eq!(
        file.matches(SPILL_TRUNCATED_MARKER).count(),
        1,
        "file must contain the marker exactly once"
    );
}

#[tokio::test]
async fn sink_truncates_stderr_ring_and_counts_full_stream() {
    let dir = tempfile::tempdir().unwrap();
    let storage = FileToolResultStorage::new(dir.path());
    let mut sink = storage
        .open_sink(sink_request("call-8", None))
        .await
        .unwrap();

    let overflow = STDERR_RING_CHARS + 1_000;
    sink.write(OutputStream::Stderr, &vec![b'x'; overflow])
        .unwrap();
    let spilled = sink.finish().unwrap();

    assert!(spilled.stderr_truncated);
    assert_eq!(spilled.output_bytes, overflow, "counters stay exact");
    let reference = spilled.reference.expect("output must persist");
    let file = std::fs::read_to_string(&reference.path).unwrap();
    assert!(
        file.contains(&format!(
            "--- stderr (last {} of {} chars) ---",
            STDERR_RING_CHARS, overflow
        )),
        "header must state the ring window: {file:.40}"
    );
}

#[test]
fn streamed_reference_renders_the_same_tag_and_marker() {
    let short = persisted_reference_from_tail("p.txt", "abc", 3);
    assert_eq!(short, "<persisted-output path=\"p.txt\">\nabc");

    let total = PREVIEW_CHARS + 1_000;
    let long = persisted_reference_from_tail("p.txt", &"b".repeat(PREVIEW_CHARS), total);
    let expected = format!(
        "<persisted-output path=\"p.txt\">\n[… {} chars omitted; full output in the file above …]\n{}",
        1_000,
        "b".repeat(PREVIEW_CHARS)
    );
    assert_eq!(long, expected);
    assert_eq!(
        assistant_tools::compact::parse_persisted_tag(&long)
            .map(|tag| tag.path)
            .as_deref(),
        Some("p.txt")
    );
}

#[test]
fn namespace_root_matches_the_persist_directory() {
    let dir = tempfile::tempdir().unwrap();
    let storage = FileToolResultStorage::new(dir.path());
    let root = storage
        .namespace_root(Some("session-a"))
        .expect("file storage always exposes a namespace root");
    assert_eq!(root, dir.path().join(session_namespace(Some("session-a"))));
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
fn cleanup_skips_missing_root_silently() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("does-not-exist");

    let report = cleanup_expired_results(&missing, 30);

    // 目录不存在 = 从未落盘过结果，属正常空态：既不算删除也不算失败。
    assert_eq!(report, CleanupReport::default());
}

#[test]
fn result_stats_shares_the_cleanup_scope() {
    let dir = tempfile::tempdir().unwrap();
    let expired = dir.path().join("old.txt");
    let fresh = dir.path().join("new.txt");
    write_file(&expired, "old");
    write_file(&fresh, "new");
    set_mtime_days_ago(&expired, 40);

    let before = result_stats(dir.path()).unwrap();
    assert_eq!(before.count, 2, "统计覆盖整个受管目录");
    assert_eq!(before.bytes, 6, "按文件长度求和");
    assert!(before.oldest_at.is_some());

    let report = cleanup_expired_results(dir.path(), 30);
    assert_eq!(report.removed, 1);

    let after = result_stats(dir.path()).unwrap();
    assert_eq!(after.count, 1, "清理后只剩未过期文件");
    assert_eq!(after.bytes, 3, "统计与清理共用同一遍历口径");
}

#[test]
fn result_stats_ignores_companion_files() {
    let dir = tempfile::tempdir().unwrap();
    write_file(&dir.path().join("a.txt"), "12");
    write_file(&dir.path().join("a.txt.tmp"), "partial");

    let stats = result_stats(dir.path()).unwrap();
    assert_eq!(stats.count, 1, "伴生 .tmp 不计入条数");
    assert_eq!(stats.bytes, 2, "伴生 .tmp 不计入字节");
}

#[test]
fn retention_is_injected_once_and_survives_disabled_policy() {
    let dir = tempfile::tempdir().unwrap();
    let storage = FileToolResultStorage::new(dir.path()).with_retention(0, 1_700_000_000);

    assert_eq!(
        storage.result_retention(),
        (0, 1_700_000_000),
        "注入值原样自述，0 表示禁用"
    );
    assert_eq!(
        ToolResultStorage::result_retention(&storage),
        Some((0, 1_700_000_000)),
        "trait 侧同样给出注入值"
    );
    assert!(
        ToolResultStorage::result_stats(&storage)
            .expect("文件实现必须提供 stats")
            .is_ok(),
        "策略禁用不影响观测：stats 仍可出数"
    );
}

fn relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// 三份过期文件（根目录 / 命名空间子目录 / 伴生 `.tmp`）+ 一份未过期文件。
fn expired_layout(root: &Path) {
    let namespace = root.join(session_namespace(Some("s1")));
    std::fs::create_dir_all(&namespace).unwrap();
    write_file(&root.join("old.txt"), "old");
    write_file(&namespace.join("nested.txt"), "nested");
    write_file(&root.join("partial.txt.tmp"), "partial");
    write_file(&root.join("new.txt"), "new");
    set_mtime_days_ago(&root.join("old.txt"), 40);
    set_mtime_days_ago(&namespace.join("nested.txt"), 45);
    set_mtime_days_ago(&root.join("partial.txt.tmp"), 50);
}

#[test]
fn collect_expired_candidates_match_cleanup_one_by_one() {
    let collect_root = tempfile::tempdir().unwrap();
    let sweep_root = tempfile::tempdir().unwrap();
    expired_layout(collect_root.path());
    expired_layout(sweep_root.path());

    let candidates = collect_expired(collect_root.path(), 30);
    let mut candidate_names: Vec<String> = candidates
        .iter()
        .map(|candidate| relative_to(collect_root.path(), &candidate.path))
        .collect();
    candidate_names.sort();
    assert_eq!(
        candidate_names.len(),
        3,
        "过期文件 + 命名空间内文件 + 伴生 .tmp"
    );
    assert!(
        !candidate_names.iter().any(|name| name == "new.txt"),
        "未过期文件不得进候选"
    );

    let expected_bytes = 3 + 6 + 7;
    let collected_bytes: u64 = candidates.iter().map(|candidate| candidate.bytes).sum();
    assert_eq!(
        collected_bytes, expected_bytes,
        "候选字节取收集时刻的 metadata.len()"
    );

    // 同一份布局交给启动清理：删掉的必须与候选逐条一致。
    let report = cleanup_expired_results(sweep_root.path(), 30);
    assert_eq!(
        report.removed,
        candidates.len(),
        "启动清理与候选收集删同一批"
    );
    assert_eq!(report.failed, 0);
    for name in &candidate_names {
        assert!(
            !sweep_root.path().join(name).exists(),
            "启动清理应删掉候选 {}",
            name
        );
    }
    assert!(sweep_root.path().join("new.txt").exists());

    // 候选执行侧：条数 / 字节与收集时刻一致，重复执行只计 skipped。
    let deleted = delete_candidates(&candidates);
    assert_eq!(deleted.removed, candidates.len() as u64);
    assert_eq!(deleted.removed_bytes, expected_bytes);
    assert_eq!(deleted.failed, 0);
    assert_eq!(deleted.skipped, 0);

    let again = delete_candidates(&candidates);
    assert_eq!(again.removed, 0, "已删过的候选不得再计成功");
    assert_eq!(
        again.skipped,
        candidates.len() as u64,
        "文件已不存在属判据排除"
    );
    assert_eq!(again.failed, 0, "不存在不是失败");
}

#[test]
fn storage_trait_candidates_match_the_free_functions() {
    let dir = tempfile::tempdir().unwrap();
    expired_layout(dir.path());
    let storage = FileToolResultStorage::new(dir.path()).with_retention(30, 1_700_000_000);

    let candidates = ToolResultStorage::collect_expired(&storage).expect("文件实现必须提供候选");
    assert_eq!(candidates, collect_expired(dir.path(), 30));

    let report = ToolResultStorage::delete_candidates(&storage, &candidates)
        .expect("文件实现必须提供候选删除");
    assert_eq!(report.removed, candidates.len() as u64);
    assert_eq!(report.removed_bytes, 3 + 6 + 7);
    assert_eq!(report.failed, 0);
}

#[test]
fn future_mtime_is_not_an_expired_candidate() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("future.txt");
    write_file(&path, "future");
    let future = std::time::SystemTime::now() + std::time::Duration::from_secs(5 * 86_400);
    let file = std::fs::File::options().write(true).open(&path).unwrap();
    file.set_modified(future).unwrap();
    drop(file);

    assert!(
        collect_expired(dir.path(), 30).is_empty(),
        "mtime 落在未来一律判未过期（fail-closed）"
    );
    assert_eq!(
        cleanup_expired_results(dir.path(), 30),
        CleanupReport::default()
    );
    assert!(path.exists());
    assert_eq!(
        result_stats(dir.path()).unwrap().count,
        1,
        "观测口径不受年龄判据影响"
    );
}

#[test]
fn collect_expired_is_empty_when_retention_is_zero() {
    let dir = tempfile::tempdir().unwrap();
    let expired = dir.path().join("old.txt");
    write_file(&expired, "old");
    set_mtime_days_ago(&expired, 400);

    assert!(
        collect_expired(dir.path(), 0).is_empty(),
        "策略禁用时不得产生任何候选"
    );
    assert!(expired.exists());
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
        truncated_lines: 2,
        line_window_max_chars: 2000,
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
