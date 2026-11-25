//! 路径引用的方向性拒绝（I73 §7 A5 / A16 / A26 / A27）。
//!
//! 两个域（`scratch://`、`tool-output://`）各跑一遍同一组工具：
//! 六个写方向工具（write / edit / mkdir / delete / move / fs_link）加 shell 的 `cwd`，
//! 全部必须在**任何副作用发生之前**拒绝，写方向文案按域分叉（`write_rejected`）；
//! 三个浏览类工具（list / grep / glob）同样在任何 IO 之前拒绝，但文案是 `BROWSE_REJECTED`
//! ——浏览不是写入，复用写方向文案会让模型误判工具类别（§4 V5）。
//!
//! 每个用例都先准备好域内真实文件（`handoff.txt`），拒绝后再核对文件系统：
//! 拒绝若发生在副作用之后，这里会立刻看到多出来的文件或被改写的内容。

use std::path::Path;
use std::sync::Arc;

use assistant_tools::context::ToolUseContext;
use assistant_tools::os::agent_delete::AgentDeleteTool;
use assistant_tools::os::agent_edit::AgentEditTool;
use assistant_tools::os::agent_glob::AgentGlobTool;
use assistant_tools::os::agent_grep::AgentGrepTool;
use assistant_tools::os::agent_list::AgentListTool;
use assistant_tools::os::agent_mkdir::AgentMkdirTool;
use assistant_tools::os::agent_move::AgentMoveTool;
use assistant_tools::os::agent_shell::schema::ShellInput;
use assistant_tools::os::agent_write::AgentWriteTool;
use assistant_tools::os::fs_link::FsLinkTool;
use assistant_tools::os::path::{
    write_rejected, Scheme, SchemeNature, BROWSE_REJECTED, CWD_REJECTED,
};
use assistant_tools::storage::InMemoryReadReceiptStore;
use assistant_tools::Tool;
use serde_json::{json, Value};

fn scheme_of(name: &str) -> Scheme {
    match name {
        "scratch" => Scheme::Scratch,
        "tool-output" => Scheme::ToolOutput,
        other => panic!("unknown scheme {other}"),
    }
}

fn context(workspace: &Path, scratch: &Path, output: &Path) -> ToolUseContext {
    ToolUseContext {
        workspace_root: Some(workspace.display().to_string()),
        session_id: Some("i73-write-direction".to_string()),
        tool_call_id: Some("call-i73".to_string()),
        tool_output_root: Some(output.to_path_buf()),
        tool_scratch_root: Some(scratch.to_path_buf()),
        ..Default::default()
    }
}

/// 取出拒绝文案：工具既可能 `Err`（解析失败），也可能 `Ok(err_result)`（业务拒绝）。
async fn rejection(tool: &dyn Tool, input: Value, context: &ToolUseContext) -> String {
    match tool.execute(&input, context).await {
        Ok(result) => {
            assert!(!result.ok, "write direction must reject the reference");
            result.error.unwrap_or_default()
        }
        Err(error) => error,
    }
}

fn write_input(path: String, content: &str) -> Value {
    json!({ "path": path, "content": content })
}

/// A16 / A5：写方向六组对 `scratch://` 与 `tool-output://` 一律拒绝，且不留痕迹。
#[tokio::test]
async fn write_direction_rejects_both_reference_schemes() {
    for scheme in ["scratch", "tool-output"] {
        let workspace = tempfile::tempdir().expect("workspace");
        let scratch = tempfile::tempdir().expect("scratch");
        let output = tempfile::tempdir().expect("output");
        // 域内真实文件：拒绝必须让它原样留在原地。
        let domain_dir = if scheme == "scratch" {
            scratch.path()
        } else {
            output.path()
        };
        std::fs::write(domain_dir.join("handoff.txt"), "carried\n").expect("seed handoff");

        let ctx = context(workspace.path(), scratch.path(), output.path());
        let expected = write_rejected(scheme_of(scheme));
        let reference = |leaf: &str| format!("{}://{}", scheme, leaf);
        let domain_path = |leaf: &str| domain_dir.join(leaf);
        let workspace_path = |leaf: &str| workspace.path().join(leaf);
        let receipts = Arc::new(InMemoryReadReceiptStore::new());

        let write = AgentWriteTool::new(receipts.clone());
        let message = rejection(
            &write,
            write_input(reference("new.txt"), "should not land"),
            &ctx,
        )
        .await;
        assert_eq!(message, expected, "{} write", scheme);
        assert!(
            !domain_path("new.txt").exists(),
            "{} write left a file",
            scheme
        );

        let edit = AgentEditTool::new(receipts.clone());
        let message = rejection(
            &edit,
            json!({
                "path": reference("handoff.txt"),
                "old_string": "carried",
                "new_string": "edited",
            }),
            &ctx,
        )
        .await;
        assert_eq!(message, expected, "{} edit", scheme);
        assert_eq!(
            std::fs::read_to_string(domain_path("handoff.txt")).expect("handoff"),
            "carried\n",
            "{} edit rewrote the file",
            scheme
        );

        let mkdir = AgentMkdirTool::new();
        let message = rejection(&mkdir, json!({ "path": reference("newdir") }), &ctx).await;
        assert_eq!(message, expected, "{} mkdir", scheme);
        assert!(
            !domain_path("newdir").exists(),
            "{} mkdir created a dir",
            scheme
        );

        let delete = AgentDeleteTool::new(receipts.clone());
        let message = rejection(
            &delete,
            json!({ "path": reference("handoff.txt"), "permanent": true }),
            &ctx,
        )
        .await;
        assert_eq!(message, expected, "{} delete", scheme);
        assert!(
            domain_path("handoff.txt").exists(),
            "{} delete removed it",
            scheme
        );

        let mv = AgentMoveTool::new(receipts.clone());
        let message = rejection(
            &mv,
            json!({
                "source": reference("handoff.txt"),
                "target": workspace_path("renamed.txt").display().to_string(),
            }),
            &ctx,
        )
        .await;
        assert_eq!(message, expected, "{} move source", scheme);
        assert!(
            domain_path("handoff.txt").exists(),
            "{} move lost the source",
            scheme
        );
        assert!(
            !workspace_path("renamed.txt").exists(),
            "{} move landed a file",
            scheme
        );

        let link = FsLinkTool::new();
        let message = rejection(
            &link,
            json!({
                "action": "create",
                "source": reference("new.link"),
                "target": workspace_path("seed.txt").display().to_string(),
                "kind": "symlink",
            }),
            &ctx,
        )
        .await;
        assert_eq!(message, expected, "{} fs_link", scheme);
        assert!(
            !domain_path("new.link").exists(),
            "{} fs_link created a link",
            scheme
        );

        // shell 的 `cwd`：校验阶段就拦住，且标记为不可通过改写输入绕过。
        let validation = ShellInput::validate(
            &json!({ "command": "echo hi", "cwd": reference("sub") }),
            &ctx,
        );
        assert!(!validation.ok, "{} shell cwd accepted", scheme);
        assert_eq!(
            validation.message.as_deref(),
            Some(CWD_REJECTED),
            "{} shell cwd",
            scheme
        );
        assert!(
            validation.blocks_input_rewrite,
            "{} shell cwd rewritable",
            scheme
        );
        assert!(!domain_path("sub").exists(), "{} shell cwd ran", scheme);
    }
}

/// A27：写方向文案不诱导拼路径，且「是否提示替代通道」由**类别**而非 scheme 名决定。
///
/// 对 `Scheme::ALL` 这个闭集逐项断言，新增 scheme 自动纳入：
/// 1. 文案里**没有**「plain path / absolute path」这类对输入的要求——域引用的物理路径
///    正是模型不知道的，要求它给普通路径等于让它去猜（I73 §4 V7）。
/// 2. `Namespace`（平铺内容域）**一律不**提示通道：域本身不可写，提示即假提示。
/// 3. `Location`（可写目录重定向）的提示与否严格跟随它声明的 `write_alternate`。
#[test]
fn write_rejection_copy_follows_scheme_nature() {
    for scheme in Scheme::ALL {
        let copy = write_rejected(scheme);
        let lowered = copy.to_lowercase();

        assert!(
            !lowered.contains("plain path"),
            "{} rejection must not ask for a plain path: {copy}",
            scheme.as_str()
        );
        assert!(
            !lowered.contains("absolute"),
            "{} rejection must not ask for an absolute path: {copy}",
            scheme.as_str()
        );

        match scheme.nature() {
            SchemeNature::Namespace => assert!(
                !lowered.contains("shell"),
                "{} is a content domain; advertising a write channel is a fake hint: {copy}",
                scheme.as_str()
            ),
            SchemeNature::Location { write_alternate } => assert_eq!(
                lowered.contains("shell"),
                write_alternate.is_some(),
                "{} channel advertisement must follow its declared alternate: {copy}",
                scheme.as_str()
            ),
        }
    }

    // 两类文案必须不同：合并会让「域不可写」与「只是传输层限制」混成一句话（§4）。
    assert_ne!(
        write_rejected(Scheme::Scratch),
        write_rejected(Scheme::ToolOutput),
        "copy must differ across natures"
    );
}

/// A26：浏览类工具对引用一律拒绝，且文案是 `BROWSE_REJECTED` 而不是写方向文案。
///
/// 三个工具都必须在 `validate_input` 阶段就拒绝（`invalid`，不是 `blocked`）：
/// 这类拒绝是「本工具不收这个输入」，不是「要用户批准才放行」。
#[tokio::test]
async fn browse_direction_rejects_both_reference_schemes() {
    for scheme in ["scratch", "tool-output"] {
        let workspace = tempfile::tempdir().expect("workspace");
        let scratch = tempfile::tempdir().expect("scratch");
        let output = tempfile::tempdir().expect("output");
        let domain_dir = if scheme == "scratch" {
            scratch.path()
        } else {
            output.path()
        };
        std::fs::write(domain_dir.join("handoff.txt"), "carried\n").expect("seed handoff");

        let ctx = context(workspace.path(), scratch.path(), output.path());
        // body 必填：`tool-output://` 的空 body 在解析期就被拒，走不到方向判定。
        let reference = format!("{}://handoff.txt", scheme);

        let list = AgentListTool::new();
        let message = rejection(&list, json!({ "path": reference }), &ctx).await;
        assert_eq!(message, BROWSE_REJECTED, "{} list", scheme);

        let grep = AgentGrepTool::new();
        let message = rejection(
            &grep,
            json!({ "path": reference, "pattern": "carried" }),
            &ctx,
        )
        .await;
        assert_eq!(message, BROWSE_REJECTED, "{} grep", scheme);

        let glob = AgentGlobTool::new();
        let message = rejection(
            &glob,
            json!({ "path": reference, "pattern": "*.txt" }),
            &ctx,
        )
        .await;
        assert_eq!(message, BROWSE_REJECTED, "{} glob", scheme);

        // 校验阶段同口径：文案一致，且不是 `blocked`（不进入审批）。
        for (tool, input) in [
            (&list as &dyn Tool, json!({ "path": reference })),
            (
                &grep as &dyn Tool,
                json!({ "path": reference, "pattern": "carried" }),
            ),
            (
                &glob as &dyn Tool,
                json!({ "path": reference, "pattern": "*.txt" }),
            ),
        ] {
            let validation = tool.validate_input(&input, &ctx);
            assert!(!validation.ok, "{} validation accepted", scheme);
            assert_eq!(
                validation.message.as_deref(),
                Some(BROWSE_REJECTED),
                "{} validation message",
                scheme
            );
            assert!(
                !validation.blocks_input_rewrite,
                "{} validation must not be blocked",
                scheme
            );
        }

        // 拒绝之后域内文件必须原样存在（未发生任何 IO 副作用）。
        assert_eq!(
            std::fs::read_to_string(domain_dir.join("handoff.txt")).expect("handoff"),
            "carried\n",
            "{} browse touched the domain",
            scheme
        );
    }
}
