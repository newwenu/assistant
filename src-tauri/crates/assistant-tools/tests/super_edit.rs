//! `super_edit` 的门禁（I58 §3.3 G14 / G18）。
//!
//! 只钉「本工具自己的契约」，索引与自证的正确性由 `super_index` 与 `super_read` 的
//! 用例承担（判据单源，G18）：
//! - F7 / F8 / F9 / F11 的失败形态与文案；
//! - 成功路径下**区间外字节逐字节不变**（BOM 与 CRLF 原样保留）；
//! - 写成功后 `invalidate_path` 生效、回执按 `super_read` 口径写入；
//! - 首次调用自行建索引（不依赖先调用过 `super_read`）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use assistant_tools::os::super_edit::SuperEditTool;
use assistant_tools::os::super_read::SuperReadTool;
use assistant_tools::storage::{InMemoryReadReceiptStore, ReadReceiptKey, ReadReceiptStore};
use assistant_tools::{Tool, ToolUseContext};
use serde_json::{json, Value};

fn context(root: &Path, index_root: &Path) -> ToolUseContext {
    ToolUseContext {
        workspace_root: Some(root.display().to_string()),
        session_id: Some("session-1".to_string()),
        tool_call_id: Some("call-1".to_string()),
        index_root: Some(index_root.to_path_buf()),
        ..Default::default()
    }
}

fn edit_tool(receipts: Arc<dyn ReadReceiptStore>) -> SuperEditTool {
    SuperEditTool::new(receipts)
}

fn write_bytes(root: &Path, name: &str, content: &[u8]) -> PathBuf {
    let path = root.join(name);
    std::fs::write(&path, content).expect("write fixture");
    path
}

fn write(root: &Path, name: &str, content: &str) -> PathBuf {
    write_bytes(root, name, content.as_bytes())
}

fn read_bytes(path: &Path) -> Vec<u8> {
    std::fs::read(path).expect("read fixture")
}

fn read_text(path: &Path) -> String {
    std::fs::read_to_string(path).expect("read fixture")
}

async fn run(tool: &dyn Tool, input: Value, ctx: &ToolUseContext) -> (Value, String) {
    let result = tool.execute(&input, ctx).await.expect("execute");
    assert!(result.ok, "{:?}", result.error);
    (
        result.data.clone().unwrap_or(Value::Null),
        result.assistant_text().to_string(),
    )
}

async fn run_err(tool: &dyn Tool, input: Value, ctx: &ToolUseContext) -> String {
    let result = tool.execute(&input, ctx).await.expect("execute");
    assert!(!result.ok, "调用必须失败：{:?}", result.data);
    result.error.clone().unwrap_or_default()
}

fn canonical(root: &Path, name: &str) -> String {
    assistant_tools::os::path::canonicalize_lenient(&root.join(name))
        .expect("canonical fixture")
        .display()
        .to_string()
}

/// G18：首次调用自行建索引（此前没有任何 `super_read`），且区间外字节零改动。
///
/// fixture 是「BOM + CRLF」——两种最容易被整文件重写弄丢的东西同时在场：
/// BOM 属于 head，CRLF 是 tail 与 replacement 展开的依据。
#[tokio::test]
async fn a_first_call_builds_its_own_index_and_preserves_bytes_outside_the_range() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let tool = edit_tool(Arc::new(InMemoryReadReceiptStore::new()));

    let mut original = vec![0xEF, 0xBB, 0xBF];
    original.extend_from_slice(b"one\r\ntwo\r\nthree\r\n");
    let path = write_bytes(dir.path(), "doc.txt", &original);

    let (data, text) = run(
        &tool,
        json!({
            "path": "doc.txt",
            "startLine": 2,
            "startColumn": 1,
            "endColumn": 1,
            "replacement": "new\n"
        }),
        &ctx,
    )
    .await;

    let mut expected = vec![0xEF, 0xBB, 0xBF];
    expected.extend_from_slice(b"one\r\nnew\r\ntwo\r\nthree\r\n");
    assert_eq!(read_bytes(&path), expected);

    assert!(
        text.contains("[index: built"),
        "首次调用必须自行建索引：{text}"
    );
    assert_eq!(data["startLine"], json!(2));
    assert_eq!(data["endLine"], json!(2));
    assert_eq!(data["replacedChars"], json!(0));
    assert_eq!(data["bytesWritten"], json!(expected.len()));
    assert!(data["revision"].is_string());
}

/// 区间外字节逐字节不变：只改中间一行的一部分，前后（含行尾符）必须原样。
#[tokio::test]
async fn only_the_target_range_changes() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let tool = edit_tool(Arc::new(InMemoryReadReceiptStore::new()));
    let path = write(dir.path(), "doc.txt", "alpha\nbeta\ngamma\n");

    run(
        &tool,
        json!({
            "path": "doc.txt",
            "startLine": 2,
            "startColumn": 2,
            "endColumn": 4,
            "expected": "et",
            "replacement": "ET"
        }),
        &ctx,
    )
    .await;

    assert_eq!(read_text(&path), "alpha\nbETa\ngamma\n");
}

/// F7：非空区间缺 `expected` → 拒绝，并说明为什么必须给。
#[tokio::test]
async fn a_non_empty_range_without_expected_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let tool = edit_tool(Arc::new(InMemoryReadReceiptStore::new()));
    let path = write(dir.path(), "doc.txt", "alpha\nbeta\n");

    let error = run_err(
        &tool,
        json!({
            "path": "doc.txt",
            "startLine": 2,
            "replacement": "delta"
        }),
        &ctx,
    )
    .await;

    assert!(error.contains("'expected' is required"), "{error}");
    assert_eq!(read_text(&path), "alpha\nbeta\n", "失败调用不得改动文件");
}

/// 纯插入（空区间）不需要 `expected`：没有既有文本可校验。
#[tokio::test]
async fn an_empty_range_needs_no_expected() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let tool = edit_tool(Arc::new(InMemoryReadReceiptStore::new()));
    let path = write(dir.path(), "doc.txt", "alpha\nbeta\n");

    run(
        &tool,
        json!({
            "path": "doc.txt",
            "startLine": 2,
            "startColumn": 1,
            "endColumn": 1,
            "replacement": "inserted\n"
        }),
        &ctx,
    )
    .await;

    assert_eq!(read_text(&path), "alpha\ninserted\nbeta\n");
}

/// F8：`expected` 与现区间文本不符 → 拒绝，并回显此刻的区间文本（带行号）。
#[tokio::test]
async fn a_mismatched_expected_is_refused_with_the_current_text() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let tool = edit_tool(Arc::new(InMemoryReadReceiptStore::new()));
    let path = write(dir.path(), "doc.txt", "alpha\nbeta\ngamma\n");

    let error = run_err(
        &tool,
        json!({
            "path": "doc.txt",
            "startLine": 2,
            "expected": "BETA",
            "replacement": "delta"
        }),
        &ctx,
    )
    .await;

    assert!(error.contains("'expected' does not match"), "{error}");
    assert!(error.contains("beta"), "必须回显现文本：{error}");
    assert_eq!(read_text(&path), "alpha\nbeta\ngamma\n");
}

/// `expected` 的换行风格不参与判定：模型给 `\r\n` 与给 `\n` 都算对上。
#[tokio::test]
async fn expected_newlines_are_normalized_before_comparing() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let tool = edit_tool(Arc::new(InMemoryReadReceiptStore::new()));
    let path = write_bytes(dir.path(), "doc.txt", b"one\r\ntwo\r\n");

    run(
        &tool,
        json!({
            "path": "doc.txt",
            "startLine": 1,
            "startColumn": 1,
            "endLine": 2,
            "endColumn": 1,
            "expected": "one\n",
            "replacement": "ONE\n"
        }),
        &ctx,
    )
    .await;

    assert_eq!(read_bytes(&path), b"ONE\r\ntwo\r\n");
}

/// F9：互斥参数、倒序区间、列越界都必须在定位前被挡住。
#[tokio::test]
async fn invalid_addresses_are_rejected_before_locating() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let tool = edit_tool(Arc::new(InMemoryReadReceiptStore::new()));
    let path = write(dir.path(), "doc.txt", "alpha\nbeta\n");

    let error = run_err(
        &tool,
        json!({
            "path": "doc.txt",
            "startLine": 1,
            "endLine": 1,
            "endColumn": 3,
            "toEndOfLine": true
        }),
        &ctx,
    )
    .await;
    assert!(error.contains("mutually exclusive"), "{error}");

    let error = run_err(
        &tool,
        json!({ "path": "doc.txt", "startLine": 2, "endLine": 1 }),
        &ctx,
    )
    .await;
    assert!(error.contains("must not be before"), "{error}");

    let error = run_err(
        &tool,
        json!({ "path": "doc.txt", "startLine": 1, "startColumn": 99 }),
        &ctx,
    )
    .await;
    assert!(error.contains("past the end of line"), "{error}");

    assert_eq!(read_text(&path), "alpha\nbeta\n");
}

/// F5：`unit = "byte"` 的列切在多字节字符中间 → 报最近边界，不改文件。
#[tokio::test]
async fn a_byte_column_inside_a_character_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let tool = edit_tool(Arc::new(InMemoryReadReceiptStore::new()));
    let path = write(dir.path(), "doc.txt", "中文\n");

    let error = run_err(
        &tool,
        json!({
            "path": "doc.txt",
            "startLine": 1,
            "startColumn": 2,
            "endColumn": 3,
            "unit": "byte",
            "expected": "x"
        }),
        &ctx,
    )
    .await;

    assert!(error.contains("multi-byte character"), "{error}");
    assert!(error.contains("nearest character boundary"), "{error}");
    assert_eq!(read_text(&path), "中文\n");
}

/// F11：目标区间超过 16 MiB → 按区间口径报错（不提示改用 `edit`）。
#[tokio::test]
async fn a_range_over_the_cap_is_refused_by_range_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let tool = edit_tool(Arc::new(InMemoryReadReceiptStore::new()));

    let big = "a".repeat(17 * 1024 * 1024);
    let path = write(dir.path(), "big.txt", &big);

    let error = run_err(
        &tool,
        json!({ "path": "big.txt", "startLine": 1, "toEndOfLine": true, "expected": "x" }),
        &ctx,
    )
    .await;

    assert!(error.contains("over the"), "{error}");
    assert!(error.contains("byte cap"), "{error}");
    assert_eq!(read_bytes(&path).len(), big.len());
}

/// 写成功后索引缓存被丢弃：等长改写 + mtime 复原后 `revision` 不变，
/// 若没有 `invalidate_path`，下一次 `super_read` 会命中旧索引报 `reused`。
#[tokio::test]
async fn a_successful_edit_invalidates_the_index_cache() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let receipts: Arc<dyn ReadReceiptStore> = Arc::new(InMemoryReadReceiptStore::new());
    let path = write(dir.path(), "doc.txt", "alpha\nbeta\n");
    let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();

    let read = SuperReadTool::new(Arc::clone(&receipts));
    let (first, _) = run(&read, json!({ "path": "doc.txt" }), &ctx).await;
    assert_eq!(first["index"]["state"], json!("built"));

    let edit = SuperEditTool::new(Arc::clone(&receipts));
    run(
        &edit,
        json!({
            "path": "doc.txt",
            "startLine": 1,
            "startColumn": 1,
            "endColumn": 6,
            "expected": "alpha",
            "replacement": "ALPHA"
        }),
        &ctx,
    )
    .await;
    assert_eq!(read_text(&path), "ALPHA\nbeta\n");

    // 等长 + mtime 复原 → 修订号与首次读取相同，缓存键仍会命中。
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    assert_eq!(
        assistant_tools::os::agent_read::file_revision(&path).unwrap(),
        first["index"]["revision"].as_str().unwrap(),
        "前置条件：修订号必须与首次读取时相同"
    );

    let (again, text) = run(&read, json!({ "path": "doc.txt" }), &ctx).await;
    assert_eq!(
        again["index"]["state"],
        json!("built"),
        "写后必须丢弃旧索引（invalidate_path）"
    );
    assert!(text.contains("ALPHA"), "{text}");
}

/// 回执口径与 `super_read` 同构：写成功即登记，供后续 `edit` 的闸门使用。
#[tokio::test]
async fn a_successful_edit_records_a_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let receipts: Arc<dyn ReadReceiptStore> = Arc::new(InMemoryReadReceiptStore::new());
    let tool = SuperEditTool::new(Arc::clone(&receipts));
    let path = write(dir.path(), "doc.txt", "alpha\nbeta\n");

    run(
        &tool,
        json!({
            "path": "doc.txt",
            "startLine": 2,
            "startColumn": 1,
            "endColumn": 5,
            "expected": "beta",
            "replacement": "delta"
        }),
        &ctx,
    )
    .await;

    let key = ReadReceiptKey {
        session_id: ctx.session_id.clone(),
        path: canonical(dir.path(), "doc.txt"),
    };
    let receipt = receipts.lookup(&key).await.expect("回执必须写入");
    assert_eq!(
        receipt.revision,
        assistant_tools::os::agent_read::file_revision(&path).expect("revision")
    );
    assert_eq!(receipt.encoding, "UTF-8");
    assert!(!receipt.has_bom);
    assert_eq!(receipt.truncated_lines, 0);
    assert_eq!(receipt.line_window_max_chars, 2000);
}

/// 未注入 `index_root` → fail-closed，不降级为线性扫描（与 `super_read` 同一口径）。
#[tokio::test]
async fn a_missing_index_root_is_a_business_error() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ToolUseContext {
        workspace_root: Some(dir.path().display().to_string()),
        ..Default::default()
    };
    let tool = edit_tool(Arc::new(InMemoryReadReceiptStore::new()));
    let path = write(dir.path(), "doc.txt", "alpha\n");

    let error = run_err(
        &tool,
        json!({ "path": "doc.txt", "startLine": 1, "replacement": "x" }),
        &ctx,
    )
    .await;

    assert!(error.contains("index root is unavailable"), "{error}");
    assert_eq!(read_text(&path), "alpha\n");
}

/// 混用行尾（LF 与 CRLF 同文件）：区间外两段的行尾都按原字节保留，
/// 不会因为一次编辑被「统一」成某一种。
#[tokio::test]
async fn mixed_line_endings_outside_the_range_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let tool = edit_tool(Arc::new(InMemoryReadReceiptStore::new()));
    let path = write_bytes(dir.path(), "doc.txt", b"a\nb\r\nc\n");

    run(
        &tool,
        json!({
            "path": "doc.txt",
            "startLine": 3,
            "startColumn": 1,
            "endColumn": 2,
            "expected": "c",
            "replacement": "C"
        }),
        &ctx,
    )
    .await;

    assert_eq!(read_bytes(&path), b"a\nb\r\nC\n");
}

/// 闸门按区间计：20 MiB 的文件里改 6 个字符必须放行（整文件闸门会误拒）。
#[tokio::test]
async fn a_small_edit_in_a_large_file_is_not_gated_by_file_size() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let tool = edit_tool(Arc::new(InMemoryReadReceiptStore::new()));

    let mut content = String::from("header\n");
    content.push_str(&"x".repeat(20 * 1024 * 1024));
    content.push('\n');
    let path = write(dir.path(), "big.txt", &content);
    let original_len = read_bytes(&path).len();

    run(
        &tool,
        json!({
            "path": "big.txt",
            "startLine": 1,
            "startColumn": 1,
            "endColumn": 7,
            "expected": "header",
            "replacement": "HEADER"
        }),
        &ctx,
    )
    .await;

    let after = read_bytes(&path);
    assert_eq!(after.len(), original_len, "等长改写不得改变文件大小");
    assert!(after.starts_with(b"HEADER\nx"), "只动第一行");
    assert_eq!(&after[original_len - 2..], b"x\n", "尾部原样");
}

/// G18（结构性判据）：行起点自证、单行解码、列口径转换只有 `super_index/proof.rs` 一份。
///
/// 删掉 `super_index/proof.rs` 后两个工具都无法编译——这里用「同名函数/判据在
/// `src/os/tools` 下只出现在一个文件」的源码搜索断言来替代删文件编译。
#[test]
fn the_line_proof_has_exactly_one_implementation() {
    fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read dir") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                collect(&path, out);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }

    let tools_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/os/tools");
    let mut files = Vec::new();
    collect(&tools_root, &mut files);

    for needle in [
        "fn read_line(",
        "fn decode_line(",
        "fn byte_column_to_char_start(",
        "fn char_index_to_byte(",
        // 只查「构造」，`super_edit` 里 `matches!(.., StaleIndex { .. })` 是消费不是实现。
        "IndexError::StaleIndex { line",
    ] {
        let hits: Vec<&PathBuf> = files
            .iter()
            .filter(|file| {
                std::fs::read_to_string(file)
                    .map(|text| text.contains(needle))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(
            hits.len(),
            1,
            "`{needle}` 必须只出现在 super_index/proof.rs，实际命中：{hits:?}"
        );
        assert!(
            hits[0].ends_with(Path::new("super_index").join("proof.rs")),
            "`{needle}` 的唯一实现必须落在 super_index/proof.rs，实际：{:?}",
            hits[0]
        );
    }
}

/// 工具描述的静态事实：三档各司其职（I58 §3.6 / G19）。
///
/// `full` 档写「怎么用不出错」，`brief` 档写「何时该申请」。
/// 另有一条**措辞**约束：不得用否定式定义（「不做子串匹配」）——它会被读成
/// 「不用提供准确文本」，而 `expected` 恰恰要求逐字节相等，两句连起来自相矛盾（§6）。
#[tokio::test]
async fn description_serves_the_full_tier_only() {
    let receipts: Arc<dyn ReadReceiptStore> = Arc::new(InMemoryReadReceiptStore::new());
    let tool = SuperEditTool::new(receipts);

    let description = tool.description().await.unwrap();
    assert!(
        !description.contains("Default to"),
        "描述不得反向引导「默认用别的工具」: {description:?}"
    );
    assert!(
        !description.contains("substring"),
        "不得用否定式定义工具（「不做子串匹配」）: {description:?}"
    );
    assert!(
        description.contains("no unique anchor"),
        "要表达的是定位方式（按位置，不需要唯一锚点）: {description:?}"
    );
    assert!(
        description.contains("empty range"),
        "描述要带上会导致失败的约束: {description:?}"
    );
    assert!(
        description.len() <= 600,
        "full 档描述超出预算（{} 字符）: {description:?}",
        description.len()
    );

    let brief = tool.brief().expect("super_edit 必须有 brief 档");
    assert!(
        brief.contains("Request it when"),
        "brief 档要写何时该申请: {brief:?}"
    );
    assert!(
        !brief.contains("substring"),
        "不得用否定式定义工具: {brief:?}"
    );
    assert!(
        brief.len() <= 120,
        "brief 档超出首句预算（{} 字符）: {brief:?}",
        brief.len()
    );

    let manual = tool.manual().expect("super_edit 必须有手册");
    assert!(manual.contains("Range semantics"), "{manual:?}");
}
