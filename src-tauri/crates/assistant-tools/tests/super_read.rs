//! `super_read` 的对齐门禁（I58 §3.2 G8 / G12）。
//!
//! 这里只钉「与 `read` 的关系」，不重复 `render.rs` 的单测：
//! - G8：`unit` 两种口径在同一 ASCII 内容上等价；BOM 文件的字节列从内容起算。
//! - G12：同一窗口参数下正文与 `read` 逐字节一致（footer 与提示除外）。

use std::path::Path;
use std::sync::Arc;

use assistant_tools::os::agent_edit::AgentEditTool;
use assistant_tools::os::agent_read::AgentReadTool;
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

fn super_tool() -> SuperReadTool {
    SuperReadTool::new(Arc::new(InMemoryReadReceiptStore::new()))
}

fn read_tool() -> AgentReadTool {
    AgentReadTool::new(Arc::new(InMemoryReadReceiptStore::new()))
}

fn write(root: &Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = root.join(name);
    std::fs::write(&path, content).expect("write fixture");
    path
}

fn write_bytes(root: &Path, name: &str, content: &[u8]) -> std::path::PathBuf {
    let path = root.join(name);
    std::fs::write(&path, content).expect("write fixture");
    path
}

/// 正文：丢掉所有以 `[` 开头的行（截断提示与 footer），保留行尾分隔符。
///
/// fixture 的正文行不以 `[` 开头，因此这个提取对两边同样成立。
fn body(text: &str) -> String {
    let mut out = String::new();
    for segment in text.split_inclusive('\n') {
        if !segment.starts_with('[') {
            out.push_str(segment);
        }
    }
    out
}

async fn run(tool: &dyn Tool, input: Value, ctx: &ToolUseContext) -> (Value, String) {
    let result = tool.execute(&input, ctx).await.expect("execute");
    assert!(result.ok, "{:?}", result.error);
    (
        result.data.clone().unwrap_or(Value::Null),
        result.assistant_text().to_string(),
    )
}

/// G8：同一 ASCII 内容下，`unit = "char"` 与 `unit = "byte"` 的同一 `column` 等价。
#[tokio::test]
async fn char_and_byte_columns_agree_on_ascii() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    write(dir.path(), "ascii.txt", "abcdefghij\nklmnopqrst\n");

    let (char_data, char_text) = run(
        &super_tool(),
        json!({ "path": "ascii.txt", "column": 4, "unit": "char" }),
        &ctx,
    )
    .await;
    let (byte_data, byte_text) = run(
        &super_tool(),
        json!({ "path": "ascii.txt", "column": 4, "unit": "byte" }),
        &ctx,
    )
    .await;

    assert_eq!(body(&char_text), body(&byte_text), "两种口径必须等价");
    assert_eq!(
        char_data["linesBeyondColumn"],
        byte_data["linesBeyondColumn"]
    );
    // 列 footer 只有口径标签不同，起点相同。
    assert!(char_text.contains("[column: char 4]"), "{char_text:?}");
    assert!(byte_text.contains("[column: byte 4]"), "{byte_text:?}");
}

/// G8：BOM 文件的字节列从**内容**起算，`column = 1` 不会把 BOM 读成正文。
#[tokio::test]
async fn bom_file_byte_column_one_starts_at_content() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let mut content = vec![0xEF, 0xBB, 0xBF];
    content.extend_from_slice(b"alpha\nbeta\n");
    write_bytes(dir.path(), "bom.txt", &content);

    let (data, text) = run(
        &super_tool(),
        json!({ "path": "bom.txt", "unit": "byte", "column": 1 }),
        &ctx,
    )
    .await;

    assert_eq!(data["totalLines"], json!(2));
    let body_text = body(&text);
    assert!(body_text.starts_with("     1\talpha\n"), "{body_text:?}");
    assert!(
        !body_text.contains('\u{feff}'),
        "BOM 不得出现在正文: {body_text:?}"
    );
    assert!(text.contains("[encoding: UTF-8 (bom)]"), "{text:?}");
}

/// G12：同一 `offset` / `limit` 下，正文与 `read` 逐字节一致。
#[tokio::test]
async fn matches_read_byte_for_byte() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let content: String = (1..=120).map(|n| format!("line {}\n", n)).collect();
    write(dir.path(), "plain.txt", &content);

    let window = json!({ "path": "plain.txt", "offset": 10, "limit": 25 });
    let (_, super_text) = run(&super_tool(), window.clone(), &ctx).await;
    let (_, read_text) = run(&read_tool(), window, &ctx).await;

    assert_eq!(
        body(&super_text),
        body(&read_text),
        "super_read 正文必须与 read 逐字节一致"
    );
    // 差异只允许出现在 footer：B 多一个 `[index: ...]`，A 没有。
    assert!(super_text.contains("[index: "), "{super_text:?}");
    assert!(!read_text.contains("[index: "), "{read_text:?}");
}

/// G12：超长行被切时，两边的可见段与截断标记也必须一致。
#[tokio::test]
async fn matches_read_on_cut_long_lines() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let long = format!("{}\n", "x".repeat(2500));
    let content = format!("short\ntail\n{}last\n", long);
    write(dir.path(), "long.txt", &content);

    let window = json!({ "path": "long.txt" });
    let (super_data, super_text) = run(&super_tool(), window.clone(), &ctx).await;
    let (read_data, read_text) = run(&read_tool(), window, &ctx).await;

    assert_eq!(
        body(&super_text),
        body(&read_text),
        "截断后的可见段必须一致"
    );
    assert_eq!(super_data["truncatedLines"], read_data["truncatedLines"]);
    assert_eq!(super_data["totalLines"], json!(4));
    assert_eq!(
        super_data["lineWindow"],
        json!(2000),
        "默认宽度与 read 同值"
    );
}

/// `column > 1` 时正文前缀仍是同一行号格式，可见段是完整行去掉前 `column - 1` 字符。
#[tokio::test]
async fn column_shifts_visible_segment() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    write(dir.path(), "shift.txt", "0123456789\n");

    let (_, full) = run(&super_tool(), json!({ "path": "shift.txt" }), &ctx).await;
    let (data, shifted) = run(
        &super_tool(),
        json!({ "path": "shift.txt", "column": 3 }),
        &ctx,
    )
    .await;

    assert_eq!(data["startLine"], json!(1));
    let full_body = body(&full);
    let shifted_body = body(&shifted);
    assert!(full_body.contains("     1\t0123456789\n"), "{full_body:?}");
    assert!(
        shifted_body.contains("     1\t23456789\n"),
        "{shifted_body:?}"
    );
    assert!(shifted.contains("[column: char 3]"), "{shifted:?}");
}

/// 总行数恒精确：`read` 的扫描撞预算后给 `null`，`super_read` 仍给出真实值。
#[tokio::test]
async fn total_lines_is_exact_where_read_gives_null() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    // 每行 601 字节 × 20000 行 ≈ 11.7 MB，超出 `read` 的 1MB 扫描预算。
    let content = format!("{}\n", "y".repeat(600)).repeat(20_000);
    write(dir.path(), "huge.txt", &content);

    let (read_data, _) = run(&read_tool(), json!({ "path": "huge.txt" }), &ctx).await;
    assert_eq!(
        read_data["totalLines"],
        Value::Null,
        "前提：read 拿不到总数"
    );

    let (super_data, _) = run(&super_tool(), json!({ "path": "huge.txt" }), &ctx).await;
    assert_eq!(super_data["totalLines"], json!(20_000));
}

/// 索引生命周期：首次 `built`，同 `revision` 再读为 `reused`。
#[tokio::test]
async fn index_is_built_then_reused() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    write(dir.path(), "cache.txt", "a\nb\nc\n");

    let (first, _) = run(&super_tool(), json!({ "path": "cache.txt" }), &ctx).await;
    assert_eq!(first["index"]["state"], json!("built"));
    assert_eq!(first["index"]["lines"], json!(3));

    let (second, _) = run(&super_tool(), json!({ "path": "cache.txt" }), &ctx).await;
    assert_eq!(second["index"]["state"], json!("reused"));
    assert_eq!(second["index"]["buildMs"], json!(0));
}

/// 没有索引根就不提供本工具：报业务错误，不静默降级为线性扫描。
#[tokio::test]
async fn missing_index_root_is_a_business_error() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ToolUseContext {
        workspace_root: Some(dir.path().display().to_string()),
        session_id: Some("session-1".to_string()),
        ..Default::default()
    };
    write(dir.path(), "a.txt", "x\n");

    let result = super_tool()
        .execute(&json!({ "path": "a.txt" }), &ctx)
        .await
        .unwrap();
    assert!(!result.ok);
    assert!(
        result.error.unwrap_or_default().contains("index root"),
        "必须点名缺少索引根"
    );
}

/// F6：`unit = "char"` 的 `column` 超过 20 万字符上限 → 业务错误，提示改用字节口径。
#[tokio::test]
async fn a_char_column_beyond_the_cap_is_f6() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    write(dir.path(), "line.txt", "abcdef\n");

    let result = super_tool()
        .execute(
            &json!({ "path": "line.txt", "column": 200_001, "unit": "char" }),
            &ctx,
        )
        .await
        .expect("execute");
    assert!(!result.ok, "超上限必须报业务错误");
    let error = result.error.unwrap_or_default();
    assert!(error.contains("200000"), "必须给出上限: {error:?}");
    assert!(error.contains("byte"), "必须提示改用字节口径: {error:?}");

    // 上限本身可用（此处只是越界于行尾，不是参数错误）。
    let at_cap = super_tool()
        .execute(
            &json!({ "path": "line.txt", "column": 200_000, "unit": "char" }),
            &ctx,
        )
        .await
        .expect("execute");
    assert!(at_cap.ok, "{:?}", at_cap.error);
}

/// 伴随工具门控：`read` 只在**确实做不到**、且本轮存在申请入口（`request_tool`）时才提示升级。
#[tokio::test]
async fn read_hints_super_read_only_when_it_cannot_express_the_result() {
    let dir = tempfile::tempdir().unwrap();
    let ctx_with_super = ToolUseContext {
        workspace_root: Some(dir.path().display().to_string()),
        available_tool_names: Some(vec!["read".to_string(), "request_tool".to_string()]),
        ..Default::default()
    };
    let ctx_without_super = ToolUseContext {
        workspace_root: Some(dir.path().display().to_string()),
        available_tool_names: Some(vec!["read".to_string()]),
        ..Default::default()
    };

    let long = format!("{}\n", "z".repeat(2500));
    write(dir.path(), "long.txt", &long);
    write(dir.path(), "plain.txt", "one\ntwo\n");

    // 长行被切 + 申请入口可用 → 提示（指向 request_tool，而不是已启用的 super_read）。
    let (_, cut) = run(&read_tool(), json!({ "path": "long.txt" }), &ctx_with_super).await;
    assert!(cut.contains("[hint:"), "长行被切必须提示升级: {cut:?}");
    assert!(cut.contains("request_tool"), "{cut:?}");
    assert!(cut.contains("super_read"), "{cut:?}");

    // 长行被切 + 申请入口不可用 → 不提示（建议一个调不到的工具比不提示更糟）。
    let (_, cut_without) = run(
        &read_tool(),
        json!({ "path": "long.txt" }),
        &ctx_without_super,
    )
    .await;
    assert!(!cut_without.contains("[hint:"), "{cut_without:?}");

    // 普通文件 → 不提示：`read` 完全够用。
    let (_, plain) = run(
        &read_tool(),
        json!({ "path": "plain.txt" }),
        &ctx_with_super,
    )
    .await;
    assert!(!plain.contains("[hint:"), "{plain:?}");
}

/// 工具描述的静态事实：三档各司其职，描述不抢手册的活（I58 §3.6 / G19）。
///
/// `full` 档（description）读者已拿到 schema，写「怎么用不出错」；
/// `brief` 档写「何时该申请」。把选择指引塞进 description 会在调用时变成反向指令。
#[tokio::test]
async fn description_serves_the_full_tier_only() {
    let tool = super_tool();
    let description = tool.description().await.unwrap();
    assert!(
        !description.contains("Default to"),
        "描述不得反向引导「默认用别的工具」: {description:?}"
    );
    assert!(
        !description.contains("200..20000"),
        "参数范围属于 schema，不该在描述里复述: {description:?}"
    );
    assert!(
        description.contains("linesBeyondColumn"),
        "描述要带上会导致失败的约束: {description:?}"
    );
    assert!(
        description.len() <= 600,
        "full 档描述超出预算（{} 字符）: {description:?}",
        description.len()
    );

    let brief = tool.brief().expect("super_read 必须有 brief 档");
    assert!(
        brief.contains("Request it when"),
        "brief 档要写何时该申请: {brief:?}"
    );
    assert!(
        !brief.contains("linesBeyondColumn"),
        "调用约束不进 brief 档: {brief:?}"
    );
    assert!(
        brief.len() <= 120,
        "brief 档超出首句预算（{} 字符）: {brief:?}",
        brief.len()
    );

    let manual = tool.manual().expect("super_read 必须有手册");
    assert!(manual.contains("When to use it"), "{manual:?}");
    assert!(
        manual.contains("stay with it"),
        "手册里说明何时不该用: {manual:?}"
    );
}

/// G9：同 `(path, revision)` 并发 N 次只建一次索引（`built` 恰好出现一次）。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_calls_build_the_index_once() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    // 够多的行让建索引不可能是「瞬间完成」，并发才有机会真的重叠。
    let content: String = (1..=40_000).map(|n| format!("line {}\n", n)).collect();
    write(dir.path(), "wide.txt", &content);

    let tool = Arc::new(super_tool());
    let mut handles = Vec::new();
    for _ in 0..8 {
        let tool = Arc::clone(&tool);
        let ctx = ctx.clone();
        handles.push(tokio::spawn(async move {
            let result = tool
                .execute(&json!({ "path": "wide.txt", "limit": 5 }), &ctx)
                .await
                .expect("execute");
            assert!(result.ok, "{:?}", result.error);
            result.data.expect("data")
        }));
    }

    let mut built = 0;
    for handle in handles {
        let data = handle.await.expect("join");
        assert_eq!(data["totalLines"], json!(40_000));
        if data["index"]["state"] == json!("built") {
            built += 1;
        }
    }
    assert_eq!(built, 1, "并发调用只能有一个建索引");
}

/// G10：文件变化后索引被纠正——粗筛（`revision`）负责常规情形，
/// 「等长改写 + mtime 设回原值」让粗筛失效，由行起点自证兜底（I58 §4）。
#[tokio::test]
async fn a_changed_file_rebuilds_the_index() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let receipts: Arc<dyn ReadReceiptStore> = Arc::new(InMemoryReadReceiptStore::new());
    write(dir.path(), "doc.txt", "alpha\nbeta\n");

    let super_tool = SuperReadTool::new(Arc::clone(&receipts));
    let (first, _) = run(&super_tool, json!({ "path": "doc.txt" }), &ctx).await;
    assert_eq!(first["index"]["state"], json!("built"));

    // 外部改写（不经过本会话工具）：修订号变化，下一次调用必须重建。
    write(dir.path(), "doc.txt", "alpha\nbeta\ngamma\n");
    let (after_write, _) = run(&super_tool, json!({ "path": "doc.txt" }), &ctx).await;
    assert_eq!(after_write["index"]["state"], json!("built"));
    assert_eq!(after_write["totalLines"], json!(3));
}

/// G10 的严格版：等长改写 + mtime 设回原值 → `revision` 不变、缓存键命中，
/// 唯一能发现变化的是行起点自证。
#[tokio::test]
async fn an_equal_length_rewrite_with_a_restored_mtime_rebuilds_via_self_proof() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let receipts: Arc<dyn ReadReceiptStore> = Arc::new(InMemoryReadReceiptStore::new());
    let path = write(dir.path(), "doc.txt", "alpha\nbeta\n");
    let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();

    let super_tool = SuperReadTool::new(Arc::clone(&receipts));
    let (first, _) = run(&super_tool, json!({ "path": "doc.txt" }), &ctx).await;
    assert_eq!(first["index"]["state"], json!("built"));
    assert_eq!(first["totalLines"], json!(2));

    // 11 字节改写为 11 字节，但换行位置前移：旧起点表整张失效。
    write(dir.path(), "doc.txt", "alp\na\nbeta\n");
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

    let (after_rewrite, text) = run(&super_tool, json!({ "path": "doc.txt" }), &ctx).await;
    assert_eq!(after_rewrite["index"]["state"], json!("built"));
    assert_eq!(after_rewrite["totalLines"], json!(3));
    assert_eq!(body(&text), "     1\talp\n     2\ta\n     3\tbeta\n");
}

/// 反向用例：改写不移动任何换行时，起点表依然正确，**不得**触发重建。
#[tokio::test]
async fn a_rewrite_that_keeps_the_line_breaks_is_not_a_stale_index() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let receipts: Arc<dyn ReadReceiptStore> = Arc::new(InMemoryReadReceiptStore::new());
    let path = write(dir.path(), "doc.txt", "alpha\nbeta\n");
    let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();

    let super_tool = SuperReadTool::new(Arc::clone(&receipts));
    let (first, _) = run(&super_tool, json!({ "path": "doc.txt" }), &ctx).await;
    assert_eq!(first["index"]["state"], json!("built"));

    write(dir.path(), "doc.txt", "ALPHA\nBETA\n");
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(mtime)
        .unwrap();

    let (after_rewrite, text) = run(&super_tool, json!({ "path": "doc.txt" }), &ctx).await;
    assert_eq!(
        after_rewrite["index"]["state"],
        json!("reused"),
        "换行未动时起点表仍然有效，不应重建"
    );
    assert_eq!(body(&text), "     1\tALPHA\n     2\tBETA\n");
}

/// A 侧 `edit` 不再通知索引（I58 §4）：下一次 `super_read` 的正确性靠修订号与自证，
/// 而不是靠写工具记得调 `invalidate_path`。
#[tokio::test]
async fn an_edit_is_picked_up_without_any_write_side_notification() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let receipts: Arc<dyn ReadReceiptStore> = Arc::new(InMemoryReadReceiptStore::new());
    write(dir.path(), "doc.txt", "alpha\nbeta\n");

    let super_tool = SuperReadTool::new(Arc::clone(&receipts));
    run(&super_tool, json!({ "path": "doc.txt" }), &ctx).await;

    let edit = AgentEditTool::new(Arc::clone(&receipts));
    let result = edit
        .execute(
            &json!({ "path": "doc.txt", "old_string": "beta", "new_string": "delta" }),
            &ctx,
        )
        .await
        .expect("execute");
    assert!(result.ok, "{:?}", result.error);
    assert_eq!(read_file(&dir.path().join("doc.txt")), "alpha\ndelta\n");

    let (_, text) = run(&super_tool, json!({ "path": "doc.txt" }), &ctx).await;
    assert_eq!(body(&text), "     1\talpha\n     2\tdelta\n");
}

/// G13：`super_read` 写的回执与 `read` 同构——`truncated_lines` 同样让 `edit` fail-closed。
#[tokio::test]
async fn the_receipt_is_isomorphic_and_gates_edit() {
    let dir = tempfile::tempdir().unwrap();
    let indexes = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), indexes.path());
    let receipts: Arc<dyn ReadReceiptStore> = Arc::new(InMemoryReadReceiptStore::new());
    write(dir.path(), "wide.txt", "alpha\n");

    let tool = SuperReadTool::new(Arc::clone(&receipts));
    run(&tool, json!({ "path": "wide.txt" }), &ctx).await;

    let key = ReadReceiptKey {
        session_id: ctx.session_id.clone(),
        path: canonical(dir.path(), "wide.txt"),
    };
    let receipt = receipts.lookup(&key).await.expect("回执必须写入");
    let revision = assistant_tools::os::agent_read::file_revision(&dir.path().join("wide.txt"))
        .expect("revision");
    assert_eq!(receipt.revision, revision);
    assert_eq!(receipt.encoding, "UTF-8");
    assert!(!receipt.has_bom);
    assert_eq!(receipt.truncated_lines, 0);
    assert_eq!(receipt.line_window_max_chars, 2000);

    // 有行被切时，`edit` 必须拒绝：与 `read` 的回执走同一个闸门。
    let long = format!("alpha\n{}\n", "x".repeat(5000));
    write(dir.path(), "cut.txt", &long);
    run(&tool, json!({ "path": "cut.txt" }), &ctx).await;

    let cut_key = ReadReceiptKey {
        session_id: ctx.session_id.clone(),
        path: canonical(dir.path(), "cut.txt"),
    };
    let cut_receipt = receipts.lookup(&cut_key).await.expect("回执必须写入");
    assert_eq!(cut_receipt.truncated_lines, 1);

    let edit = AgentEditTool::new(Arc::clone(&receipts));
    let result = edit
        .execute(
            &json!({ "path": "cut.txt", "old_string": "alpha", "new_string": "beta" }),
            &ctx,
        )
        .await
        .expect("execute");
    assert!(!result.ok, "被切行必须挡住 edit");
    assert_eq!(read_file(&dir.path().join("cut.txt")), long);
}

fn read_file(path: &Path) -> String {
    std::fs::read_to_string(path).expect("read fixture")
}

fn canonical(root: &Path, name: &str) -> String {
    assistant_tools::os::path::canonicalize_lenient(&root.join(name))
        .expect("canonical fixture")
        .display()
        .to_string()
}
