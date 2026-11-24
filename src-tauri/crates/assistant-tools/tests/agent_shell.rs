//! `shell` 工具端到端验收（I52 C5 / I56 §7 / §8）。
//!
//! 覆盖条目：A6（回归）/ A9–A13 / A15 / A24 / A25 / A27–A32 / A34–A38。
//! 这些断言**必须**跑真实沙箱与真实 shell：桩掉 `sandbox::spawn` 就失去验收意义，
//! 因此本文件只做「真实执行 → 结果事实」的比对，不 mock 边界。
//!
//! 时序类断言（超时 / 取消 / 后台进程）都带宽松上界（10s）：下限由工具自身的
//! `timeout_ms` 保证，上界只用来发现「杀树失效」这类回归。

use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use assistant_tools::compact::build_compact;
use assistant_tools::context::ToolUseContext;
use assistant_tools::gateway::{
    InMemoryAuditStore, InMemoryGrantStore, ScriptedApprovalPrompter, ToolGateway,
};
use assistant_tools::os::agent_shell::ShellTool;
use assistant_tools::os::register_shell_tools;
use assistant_tools::policy::{PermissionEffect, PermissionRule, ResolvedPermissionPolicy};
use assistant_tools::registry::ToolRegistry;
use assistant_tools::result::{ToolErrorKind, ToolResult};
use assistant_tools::sandbox::{
    detect, SandboxDimension, SandboxEnforcement, SandboxHostPaths, SandboxKind, SandboxProbe,
};
use assistant_tools::storage::{FileToolResultStorage, ToolResultStorage, SPILL_TRUNCATED_MARKER};
use assistant_tools::Tool;

/// 真实执行用例串行化：同一台机器上的边界与临时目录都被共享，
/// 并行执行会让「临时目录是否已删」「后台进程是否还在」这类断言互相污染。
static EXEC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// 同一段意图的两种方言：Windows 走 `cmd`，其余平台走 `sh`。
fn script(windows: &str, unix: &str) -> String {
    if cfg!(windows) {
        windows.to_string()
    } else {
        unix.to_string()
    }
}

/// 交给子进程的路径文本：与 `spawn::plain_path_text` 同口径，剥掉 `\\?\` 前缀。
///
/// 工具把可写根 canonicalize 后拼出临时目录；Windows 上带扩展长度前缀，
/// 断言必须按子进程看到的形态比较，否则永远是「路径不同」。
fn plain_path(path: &std::path::Path) -> String {
    let text = path.display().to_string();
    match text.strip_prefix(r"\\?\UNC\") {
        Some(rest) => format!(r"\\{rest}"),
        None => match text.strip_prefix(r"\\?\") {
            Some(rest) => rest.to_string(),
            None => text,
        },
    }
}

struct Harness {
    workspace: tempfile::TempDir,
    storage: tempfile::TempDir,
    /// 只靠 `Drop` 保活：路径在构造期被 `detect` 取走，用例本身不读它。
    #[allow(dead_code)]
    sensitive: tempfile::TempDir,
    probe: &'static SandboxProbe,
    tool: Arc<ShellTool>,
}

impl Harness {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let storage = tempfile::tempdir().expect("storage tempdir");
        // 敏感列表必须非空（Unix 后端以空列表判负），且须用兄弟 tempdir：
        // 与可写根不相交，macOS 后置的 `deny file-read*` 也不会罩住 workspace。
        let sensitive = tempfile::tempdir().expect("sensitive tempdir");
        let probe = detect(&SandboxHostPaths {
            sensitive_read_deny: vec![sensitive.path().to_path_buf()],
        });
        assert_ne!(
            probe.enforcement,
            SandboxEnforcement::None,
            "sandbox unavailable on this machine: {:?}",
            probe.detail
        );
        let backend: Arc<dyn ToolResultStorage> =
            Arc::new(FileToolResultStorage::new(storage.path()));
        let tool = Arc::new(ShellTool::new(probe, backend));
        Self {
            workspace,
            storage,
            sensitive,
            probe,
            tool,
        }
    }

    /// 可写根（`ShellInput` 内部 canonicalize 后的形态）。
    fn root(&self) -> PathBuf {
        std::fs::canonicalize(self.workspace.path()).expect("canonicalize workspace")
    }

    fn storage_root(&self) -> PathBuf {
        self.storage.path().to_path_buf()
    }

    /// 调用级临时目录（`<writable_root>/.assistant-tmp/<call_id>`）。
    fn scratch(&self, call_id: &str) -> PathBuf {
        self.root().join(".assistant-tmp").join(call_id)
    }

    fn ctx(&self, call_id: &str) -> ToolUseContext {
        ToolUseContext {
            workspace_root: Some(self.workspace.path().display().to_string()),
            session_id: Some("shell-e2e".to_string()),
            tool_call_id: Some(call_id.to_string()),
            ..Default::default()
        }
    }

    async fn run_with(&self, context: &ToolUseContext, input: Value) -> ToolResult {
        self.tool
            .execute(&input, context)
            .await
            .expect("shell execute never returns Err")
    }

    async fn run(&self, call_id: &str, command: String) -> ToolResult {
        let context = self.ctx(call_id);
        self.run_with(&context, json!({ "command": command })).await
    }
}

fn data_of(result: &ToolResult) -> &Value {
    result
        .data
        .as_ref()
        .expect("shell result always carries data")
}

fn output_path(result: &ToolResult) -> String {
    data_of(result)["outputPath"]
        .as_str()
        .expect("outputPath")
        .to_string()
}

/// A9：非零退出码是「命令失败」不是「工具失败」——`ok` 仍为 `true`。
#[tokio::test]
async fn a9_nonzero_exit_is_not_a_tool_failure() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();
    let result = harness.run("call-a9", script("exit /b 3", "exit 3")).await;

    assert!(
        result.ok,
        "non-zero exit must not be a tool failure: {:?}",
        result.error
    );
    let data = data_of(&result);
    assert_eq!(data["exitCode"], json!(3));
    assert_eq!(data["signal"], Value::Null);
    let text = result.assistant_text();
    assert!(text.contains("[exit code: 3]"), "text={text}");
    assert!(text.contains("(no output)"), "text={text}");
}

/// A10：大输出落盘 + 尾窗口预览（不取头窗口），`truncated` 不由文本解析产生。
#[tokio::test]
async fn a10_large_output_spills_and_previews_tail_only() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();

    let mut content = String::with_capacity(1_000_000);
    content.push_str("HEADMARKER\n");
    for index in 1..100_000 {
        content.push_str(&format!("row{:06}\n", index));
    }
    std::fs::write(harness.root().join("big.txt"), &content).expect("write big.txt");

    let result = harness
        .run("call-a10", script("type big.txt", "cat big.txt"))
        .await;
    assert!(result.ok, "{:?}", result.error);

    let data = data_of(&result);
    assert_eq!(data["exitCode"], json!(0));
    assert_eq!(data["outputLines"], json!(100_000));

    let path = output_path(&result);
    let file = std::fs::read_to_string(&path).expect("read spilled output");
    assert_eq!(
        file.lines().count(),
        100_000,
        "whole output must be on disk"
    );

    let text = result.assistant_text();
    assert!(text.starts_with("<persisted-output path=\""), "text={text}");
    assert!(
        text.contains("chars omitted; full output in the file above"),
        "head window must be replaced by an omission marker: {text}"
    );
    assert!(
        !text.contains("HEADMARKER"),
        "shell preview must not include the head window: {text}"
    );

    let tail: String = file
        .chars()
        .rev()
        .take(1200)
        .collect::<Vec<char>>()
        .into_iter()
        .rev()
        .collect();
    assert!(text.ends_with(&tail), "text must end with the file tail");

    let compact = build_compact(
        "shell",
        &json!({ "command": script("type big.txt", "cat big.txt") }),
        &result,
        result.persisted.as_ref(),
    );
    assert!(
        compact.truncated.is_none(),
        "truncation must come from the persisted reference, not from parsing text"
    );
}

/// A11 / A25：stdout 触顶 60 MiB 只丢 stdout 超限部分，stderr 与收尾标记仍在。
#[tokio::test]
async fn a11_stdout_cap_keeps_stderr_tail_and_markers() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();

    // 61 MiB：每行 1023 字节 `a` + 换行，确保越过 60 MiB 上限。
    let line = format!("{}\n", "a".repeat(1023));
    let mut content = String::with_capacity(64 * 1024 * 1024);
    for _ in 0..62_000 {
        content.push_str(&line);
    }
    std::fs::write(harness.root().join("huge.txt"), &content).expect("write huge.txt");

    let result = harness
        .run(
            "call-a11",
            script(
                "type huge.txt & echo FAILED-TAIL 1>&2",
                "cat huge.txt; echo FAILED-TAIL >&2",
            ),
        )
        .await;
    assert!(result.ok, "{:?}", result.error);

    let data = data_of(&result);
    assert_eq!(data["outputTruncated"], json!(true));
    assert_eq!(
        data["exitCode"],
        json!(0),
        "truncation must not kill the tree"
    );
    assert!(
        data["outputBytes"].as_u64().expect("outputBytes") > 60 * 1024 * 1024,
        "total bytes stay exact even after dropping"
    );

    let text = result.assistant_text();
    assert!(text.contains(SPILL_TRUNCATED_MARKER), "text={text}");
    assert!(
        text.contains("FAILED-TAIL"),
        "stderr must survive the stdout cap: {text}"
    );

    let file = std::fs::read_to_string(output_path(&result)).expect("read spilled output");
    assert!(file.contains(SPILL_TRUNCATED_MARKER));
    assert!(file.contains("--- stderr ---"));
    assert!(file.trim_end().ends_with("FAILED-TAIL"));
}

/// A12：超时杀整棵树、保留已写输出，且 `error_kind = Timeout`。
#[tokio::test]
async fn a12_timeout_kills_tree_and_keeps_partial_output() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();
    let context = harness.ctx("call-a12");

    let started = Instant::now();
    let result = harness
        .run_with(
            &context,
            json!({
                "command": script(
                    "echo BEFORE-TIMEOUT & ping -n 30 127.0.0.1 >nul",
                    "echo BEFORE-TIMEOUT; sleep 30"
                ),
                "timeout_ms": 1000
            }),
        )
        .await;

    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Timeout));
    let data = data_of(&result);
    assert_eq!(
        data["exitCode"],
        Value::Null,
        "our kill is not a command failure"
    );
    assert!(
        data["durationMs"].as_u64().expect("durationMs") >= 1000,
        "duration must reach the deadline"
    );

    let file = std::fs::read_to_string(output_path(&result)).expect("partial output kept");
    assert!(file.contains("BEFORE-TIMEOUT"), "file={file}");

    let text = result.assistant_text();
    assert!(text.contains("[timed out after 1000ms]"), "text={text}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "timeout must not wait for the whole command"
    );
}

/// A12（取消）：取消信号同样杀树、保留输出，`error_kind = Cancelled`。
#[tokio::test]
async fn a12_cancellation_kills_tree_and_keeps_partial_output() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();
    let token = CancellationToken::new();
    let context = ToolUseContext {
        cancel: Some(token.clone()),
        ..harness.ctx("call-a12-cancel")
    };

    let tool = harness.tool.clone();
    let started = Instant::now();
    let handle = tokio::spawn(async move {
        let input = json!({
            "command": script(
                "echo BEFORE-CANCEL & ping -n 30 127.0.0.1 >nul",
                "echo BEFORE-CANCEL; sleep 30"
            )
        });
        tool.execute(&input, &context).await
    });

    tokio::time::sleep(Duration::from_millis(800)).await;
    token.cancel();

    let result = handle
        .await
        .expect("join cancel task")
        .expect("shell execute never returns Err");
    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Cancelled));

    let file = std::fs::read_to_string(output_path(&result)).expect("partial output kept");
    assert!(file.contains("BEFORE-CANCEL"), "file={file}");
    assert!(result
        .assistant_text()
        .contains("[stopped: turn cancelled]"));
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "cancellation must not wait for the whole command"
    );
}

/// A13：`cwd` 越界在审批前就被 `blocked`，且网关不会弹审批框。
#[tokio::test]
async fn a13_cwd_outside_root_is_blocked_before_approval() {
    let harness = Harness::new();
    let outside = tempfile::tempdir().expect("outside tempdir");
    let context = harness.ctx("call-a13");
    let input = json!({
        "command": script("cd", "pwd"),
        "cwd": outside.path().display().to_string()
    });

    let validation = harness.tool.validate_input(&input, &context);
    assert!(!validation.ok, "outside cwd must not validate");
    assert!(
        validation.blocks_input_rewrite,
        "换路径重试同一意图不构成新意图"
    );

    let prompter = Arc::new(ScriptedApprovalPrompter::new(Vec::new()));
    let gateway = ToolGateway::new(
        Arc::new(InMemoryGrantStore::default()),
        Arc::new(InMemoryAuditStore::default()),
        prompter.clone(),
        Duration::from_secs(5),
    );
    let policy = ResolvedPermissionPolicy::new(
        vec![PermissionRule::new("*", "*", PermissionEffect::Allow)],
        Vec::new(),
    );
    let evaluated = gateway
        .evaluate(
            harness.tool.as_ref() as &dyn Tool,
            &input,
            &context,
            &policy,
        )
        .await;
    assert!(
        evaluated.is_err(),
        "unparseable input must not reach the approval prompt"
    );
    assert_eq!(prompter.request_count(), 0, "no approval card may be shown");
}

/// A15：白名单透传 + 凭据类变量不进子进程。
#[tokio::test]
async fn a15_env_whitelist_passes_through_without_secrets() {
    let _guard = EXEC_LOCK.lock().await;
    struct EnvGuard;
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            std::env::remove_var("MY_API_TOKEN");
        }
    }
    let _env = EnvGuard;
    std::env::set_var("MY_API_TOKEN", "secret123");

    let harness = Harness::new();
    let result = harness
        .run(
            "call-a15",
            script(
                "echo PATH=%PATH% & echo TOKEN=%MY_API_TOKEN%",
                "echo \"PATH=$PATH\"; echo \"TOKEN=$MY_API_TOKEN\"",
            ),
        )
        .await;
    assert!(result.ok, "{:?}", result.error);

    let file = std::fs::read_to_string(output_path(&result)).expect("read spilled output");
    assert!(
        !file.contains("secret123"),
        "credential variables must not reach the child: {file}"
    );

    let path_line = file
        .lines()
        .find(|line| line.starts_with("PATH="))
        .expect("PATH line");
    let value = &path_line["PATH=".len()..];
    assert!(value.len() > 10, "PATH must be passed through: {path_line}");
    assert!(
        value.contains(if cfg!(windows) { ';' } else { ':' }),
        "PATH must look like a real search path: {path_line}"
    );
}

/// A24：compact head 的 subject 取 `command`、scale 为 `exit N · M lines · bytes`、anchor 为 `None`。
#[tokio::test]
async fn a24_compact_head_subject_scale_and_anchor() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();
    let command = script("echo compact-head", "echo compact-head");
    let arguments = json!({ "command": command });
    let result = harness
        .run_with(&harness.ctx("call-a24"), arguments.clone())
        .await;
    assert!(result.ok, "{:?}", result.error);

    let compact = build_compact("shell", &arguments, &result, result.persisted.as_ref());
    assert!(
        compact.head.starts_with("ok shell · echo compact-head · "),
        "head={}",
        compact.head
    );
    assert!(
        compact.head.contains(" · exit 0 · 1 lines · "),
        "head={}",
        compact.head
    );
    assert!(compact.anchor.is_none(), "shell has no document anchor");

    let truncated = ToolResult::ok_with_assistant(
        json!({
            "exitCode": 0,
            "outputLines": 2,
            "outputBytes": 10,
            "outputTruncated": true
        }),
        "x",
    );
    let compact = build_compact("shell", &json!({ "command": "run" }), &truncated, None);
    assert!(
        compact.head.contains(" · exit 0 · 2+ lines · "),
        "head={}",
        compact.head
    );
}

/// A25：`cargo build` 式场景——stdout 写满 60 MiB 后命令**失败**，
/// 文件尾部仍是 stderr 末行，回填尾窗口含真实错误行。
///
/// 与 A11 的区别只在退出码：A11 验「截断不杀树」（`exit 0`），
/// 这里验「截断 + 失败」时错误信息不被截断标记挤掉。
#[tokio::test]
async fn a25_stdout_cap_then_failure_keeps_the_error_line() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();

    let line = format!("{}\n", "a".repeat(1023));
    let mut content = String::with_capacity(64 * 1024 * 1024);
    for _ in 0..62_000 {
        content.push_str(&line);
    }
    std::fs::write(harness.root().join("huge.txt"), &content).expect("write huge.txt");

    let result = harness
        .run(
            "call-a25",
            script(
                "type huge.txt & echo REAL-ERROR 1>&2 & exit /b 7",
                "cat huge.txt; echo REAL-ERROR >&2; exit 7",
            ),
        )
        .await;
    assert!(
        result.ok,
        "non-zero exit is not a tool failure: {:?}",
        result.error
    );

    let data = data_of(&result);
    assert_eq!(data["outputTruncated"], json!(true));
    assert_eq!(data["exitCode"], json!(7));

    let file = std::fs::read_to_string(output_path(&result)).expect("read spilled output");
    assert!(
        file.trim_end().ends_with("REAL-ERROR"),
        "stderr must stay at the very end of the file: {}",
        file.chars().rev().take(120).collect::<String>()
    );

    let text = result.assistant_text();
    assert!(text.contains("REAL-ERROR"), "text={text}");
    assert!(text.contains("[exit code: 7]"), "text={text}");
}

/// A26：空输出——`outputPath` 为 `null`、回填 `(no output)`、不留 0 字节文件。
#[tokio::test]
async fn a26_empty_output_leaves_no_file() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();

    let result = harness.run("call-a26", script("exit /b 0", "exit 0")).await;
    assert!(result.ok, "{:?}", result.error);

    let data = data_of(&result);
    assert_eq!(data["outputPath"], json!(null));
    assert_eq!(data["outputBytes"], json!(0));
    assert_eq!(data["outputLines"], json!(0));
    assert_eq!(data["exitCode"], json!(0));
    assert_eq!(result.assistant_text(), "(no output)");

    // `finish` 必须删掉空文件：本用例独占一个落盘根，任何文件都算残留。
    let leftovers: Vec<_> = walk_files(harness.storage_root());
    assert!(
        leftovers.is_empty(),
        "empty output must not leave a file: {leftovers:?}"
    );
}

/// 递归收集目录下的文件（只用于 A26 的「不留空文件」断言）。
fn walk_files(root: std::path::PathBuf) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path.display().to_string());
            }
        }
    }
    out
}

/// A27：交互式挂起与渲染噪声被强制消除（`PAGER` / `GIT_PAGER` / `GIT_TERMINAL_PROMPT`）。
#[tokio::test]
async fn a27_forced_env_neutralizes_interactive_pagers() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();
    let result = harness
        .run(
            "call-a27",
            script(
                "echo PAGER=%PAGER% & echo GITPAGER=%GIT_PAGER% & echo PROMPT=%GIT_TERMINAL_PROMPT%",
                "echo \"PAGER=$PAGER GITPAGER=$GIT_PAGER PROMPT=$GIT_TERMINAL_PROMPT\"",
            ),
        )
        .await;
    assert!(result.ok, "{:?}", result.error);

    let file = std::fs::read_to_string(output_path(&result)).expect("read spilled output");
    assert!(file.contains("PAGER=cat"), "file={file}");
    assert!(file.contains("GITPAGER=cat"), "file={file}");
    assert!(file.contains("PROMPT=0"), "file={file}");
}

/// A28：命令留下的后台进程不得拖住调用返回（整树随 `cmd` 退出被回收）。
#[tokio::test]
async fn a28_background_process_does_not_hold_the_call() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();

    let started = Instant::now();
    // 不给 `start` 加重定向：`>` 作用在 `start` 这一行而非它拉起的进程上，
    // 结果会是「测试在验证重定向语法」而不是「后台进程不拖住调用」。
    let result = harness
        .run(
            "call-a28",
            script(
                r"start /b %SystemRoot%\System32\ping.exe -n 30 127.0.0.1",
                "sleep 30 & exit 0",
            ),
        )
        .await;
    assert!(result.ok, "{:?}", result.error);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the call must return once the shell exits, not when the background child does"
    );
}

/// C8：`git log` 在无 TTY 的管道里跑完、输出落盘，且不因分页器拖住调用。
///
/// 真实仓库由工具自身逐条命令搭起来（`git init` → 三次空提交），因此这条用例
/// 同时覆盖「多条命令共用同一工作区」与「git 在强制 env 下正常工作」。
/// 机器上没有 `git` 时跳过探针而不是判失败：这是环境缺失，不是实现缺陷。
#[tokio::test]
async fn c8_git_log_completes_and_lands_in_the_output_file() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();

    let probe = harness
        .run("call-c8-probe", "git --version".to_string())
        .await;
    if !probe.ok || data_of(&probe)["exitCode"] != json!(0) {
        eprintln!("skip: git is not available inside the sandbox");
        return;
    }

    let build = "git init -q . \
         && git -c user.name=shell -c user.email=shell@example.com commit -q --allow-empty -m c1 \
         && git -c user.name=shell -c user.email=shell@example.com commit -q --allow-empty -m c2 \
         && git -c user.name=shell -c user.email=shell@example.com commit -q --allow-empty -m c3";
    let built = harness.run("call-c8-build", build.to_string()).await;
    assert!(built.ok, "{:?}", built.error);
    assert_eq!(
        data_of(&built)["exitCode"],
        json!(0),
        "repo setup must succeed: {}",
        std::fs::read_to_string(output_path(&built)).unwrap_or_default()
    );

    let started = Instant::now();
    let result = harness
        .run("call-c8-log", "git log --oneline".to_string())
        .await;
    assert!(result.ok, "{:?}", result.error);
    assert_eq!(data_of(&result)["exitCode"], json!(0));
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "git log must return without waiting for a pager"
    );

    let file = std::fs::read_to_string(output_path(&result)).expect("read spilled output");
    for subject in ["c1", "c2", "c3"] {
        assert!(file.contains(subject), "missing {subject} in {file}");
    }
}

/// A29：编码探测——UTF-8 原样落盘；OEM 代码页转码后可读。
#[tokio::test]
async fn a29_output_encoding_utf8_and_oem() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();

    std::fs::write(harness.root().join("utf8.txt"), "中文 UTF-8 输出\n").expect("write utf8.txt");
    let result = harness
        .run("call-a29-utf8", script("type utf8.txt", "cat utf8.txt"))
        .await;
    assert!(result.ok, "{:?}", result.error);
    assert_eq!(data_of(&result)["outputEncoding"], json!("utf8"));
    let file = std::fs::read_to_string(output_path(&result)).expect("read spilled output");
    assert!(file.contains("中文 UTF-8 输出"), "file={file}");

    // 非 UTF-8 流：`cmd` 按控制台代码页（本机 936）输出 GBK 字节，
    // 落盘前必须转码，否则文件读出来是乱码。此处不依赖 `chcp` 探测——
    // 无控制台句柄时 `chcp` 不输出任何内容，断言会误判。
    #[cfg(windows)]
    {
        let result = harness
            .run("call-a29-oem", "echo 中文输出".to_string())
            .await;
        assert!(result.ok, "{:?}", result.error);
        let encoding = data_of(&result)["outputEncoding"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert!(
            matches!(encoding.as_str(), "utf8" | "oem936"),
            "unexpected encoding: {encoding}"
        );
        let file = std::fs::read_to_string(output_path(&result)).expect("read spilled output");
        // 只有转码成功，这段 GBK 字节才能被 `read_to_string` 读成可读文本。
        assert!(file.contains("中文输出"), "file={file}");
    }
}

/// A30：临时目录收进可写根、且调用结束即删除。
#[tokio::test]
async fn a30_scratch_dir_is_inside_root_and_removed() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();
    let call_id = "call-a30";
    let result = harness
        .run(
            call_id,
            script("echo TMPDIR=%TEMP%", "echo \"TMPDIR=$TMPDIR\""),
        )
        .await;
    assert!(result.ok, "{:?}", result.error);

    let expected = harness.scratch(call_id);
    let file = std::fs::read_to_string(output_path(&result)).expect("read spilled output");
    let reported = file
        .trim()
        .strip_prefix("TMPDIR=")
        .expect("TMPDIR line")
        .to_string();
    assert!(
        reported.starts_with(&plain_path(&harness.root())),
        "scratch dir must live inside the writable root: {reported}"
    );
    assert!(
        reported.ends_with(&format!(
            "{}{}{}",
            ".assistant-tmp",
            std::path::MAIN_SEPARATOR,
            call_id
        )),
        "scratch dir must be call-scoped: {reported}"
    );
    assert!(
        !expected.exists(),
        "scratch dir must be removed after the call"
    );
}

/// A31：边界外的写失败被标注为「沙箱拒绝」，但工具结果仍是 `ok`。
#[tokio::test]
async fn a31_denied_write_is_flagged_not_failed() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();
    let target = harness.storage_root().join("denied-probe.tmp");
    // Unix 侧刻意不用 `echo x > path`：`dash` 的重定向失败返回 2，
    // 而 2 是「命令用法错误」的排除码，判定器会拒绝归因为沙箱拒绝。
    // `touch` 在 coreutils 里恒返回 1，且报 `Permission denied`。
    let command = script(
        &format!("echo probe> \"{}\"", target.display()),
        &format!("touch '{}'", target.display()),
    );

    let result = harness.run("call-a31", command).await;
    assert!(
        result.ok,
        "sandbox denial is not a tool failure: {:?}",
        result.error
    );
    assert_eq!(data_of(&result)["sandbox"]["denied"], json!(true));
    assert!(result
        .assistant_text()
        .contains("[sandbox: write outside writable root denied]"));
    assert!(!target.exists(), "the write must have been blocked");
}

/// A32：`cmd /d` 关掉注册表里的 AutoRun，命令前缀不被注入。
#[cfg(windows)]
#[tokio::test]
async fn a32_autorun_is_disabled_by_dash_d() {
    let _guard = EXEC_LOCK.lock().await;

    struct AutorunGuard {
        created: bool,
    }
    impl Drop for AutorunGuard {
        fn drop(&mut self) {
            if self.created {
                autorun_delete();
            }
        }
    }

    let created = !autorun_exists();
    let guard = AutorunGuard { created };
    if created && !autorun_set("echo INJECTED-AUTORUN") {
        // 本机不允许写该键时无法制造对照条件：跳过探针而不是判失败。
        eprintln!("skip: cannot install the AutoRun probe");
        return;
    }

    let harness = Harness::new();
    let result = harness.run("call-a32", "echo HELLO".to_string()).await;
    drop(guard);

    assert!(result.ok, "{:?}", result.error);
    let file = std::fs::read_to_string(output_path(&result)).expect("read spilled output");
    assert!(file.contains("HELLO"), "file={file}");
    assert!(
        !file.contains("INJECTED-AUTORUN"),
        "AutoRun must be disabled by `/d`: {file}"
    );
}

#[cfg(windows)]
const AUTORUN_KEY: &str = r"HKCU\Software\Microsoft\Command Processor";

#[cfg(windows)]
fn autorun_exists() -> bool {
    std::process::Command::new("reg")
        .args(["query", AUTORUN_KEY, "/v", "AutoRun"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn autorun_set(value: &str) -> bool {
    std::process::Command::new("reg")
        .args([
            "add",
            AUTORUN_KEY,
            "/v",
            "AutoRun",
            "/t",
            "REG_SZ",
            "/d",
            value,
            "/f",
        ])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn autorun_delete() {
    let _ = std::process::Command::new("reg")
        .args(["delete", AUTORUN_KEY, "/v", "AutoRun", "/f"])
        .output();
}

/// A34：`data.sandbox.network` / `fsRead` 报的是真实能力（注册期探测 ∩ 执行计划）。
#[tokio::test]
async fn a34_sandbox_flags_report_real_boundary() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();
    let result = harness.run("call-a34", script("echo hi", "echo hi")).await;
    assert!(result.ok, "{:?}", result.error);

    let sandbox = &data_of(&result)["sandbox"];
    // `ShellSpawnPlan.network` 恒 `false`（I56 §8.4 另行求交），故 `sandbox.network` 恒为 `false`。
    let expected_fs_read = !harness.probe.missing.contains(&SandboxDimension::FsRead);
    assert_eq!(sandbox["network"].as_bool(), Some(false));
    assert_eq!(sandbox["fsRead"].as_bool(), Some(expected_fs_read));
    assert_eq!(
        sandbox["enforcement"],
        json!(harness.probe.enforcement.as_str())
    );

    #[cfg(windows)]
    {
        assert_eq!(sandbox["network"], json!(false), "Windows 无网络边界");
        assert_eq!(sandbox["fsRead"], json!(false), "Windows 无读边界");
    }
}

/// A35：`enforcement == none` 时既不注册、也不构造出可执行实例。
#[tokio::test]
async fn a35_registration_gate_refuses_unsandboxed_shell() {
    let storage_root = tempfile::tempdir().expect("storage tempdir");
    let backend: Arc<dyn ToolResultStorage> =
        Arc::new(FileToolResultStorage::new(storage_root.path()));
    let unavailable: &'static SandboxProbe = Box::leak(Box::new(SandboxProbe::unavailable(
        SandboxKind::None,
        "probe_failed",
    )));

    let registry = ToolRegistry::new();
    register_shell_tools(&registry, backend.clone(), unavailable);
    assert!(
        registry.find("shell").is_none(),
        "no empty tool may be registered when enforcement is none"
    );

    let constructed = std::panic::catch_unwind(AssertUnwindSafe(|| {
        ShellTool::new(unavailable, backend.clone())
    }));
    assert!(
        constructed.is_err(),
        "ShellTool must not be constructible without a boundary"
    );

    let harness = Harness::new();
    let healthy = ToolRegistry::new();
    register_shell_tools(&healthy, backend, harness.probe);
    assert!(
        healthy.find("shell").is_some(),
        "shell must register once the boundary is real"
    );

    // 注册只是第一步：装配面板的清单必须真的能列出这一条，否则用户无从勾选。
    let descriptors = healthy.agent_tool_descriptors().await;
    let shell = descriptors
        .iter()
        .find(|descriptor| descriptor.id == "shell")
        .expect("shell must be selectable in the tool assembly list");
    assert_eq!(shell.tier, assistant_tools::ToolTier::Os);
    assert!(
        shell.companion_of.is_none(),
        "shell is a base tool, not a companion"
    );
}

/// A36：未知字段一律拒绝——输入里不存在 `shell` / `program` / `env` / `stdin` 通道。
#[tokio::test]
async fn a36_unknown_input_fields_are_rejected() {
    let harness = Harness::new();
    let context = harness.ctx("call-a36");

    for input in [
        json!({ "command": "echo hi", "shell": "cmd" }),
        json!({ "command": "echo hi", "program": "bash" }),
        json!({ "command": "echo hi", "env": { "A": "1" } }),
        json!({ "command": "echo hi", "stdin": "yes" }),
    ] {
        let validation = harness.tool.validate_input(&input, &context);
        assert!(!validation.ok, "unknown field must not validate: {input}");
        assert!(
            !validation.blocks_input_rewrite,
            "unknown field is rewritable, not blocked: {input}"
        );
        assert!(
            validation
                .message
                .as_deref()
                .unwrap_or_default()
                .contains("unknown field"),
            "message must name the problem"
        );
    }

    let schema = harness.tool.input_schema();
    assert_eq!(schema["additionalProperties"], json!(false));
    assert_eq!(
        schema["properties"].as_object().expect("properties").len(),
        3,
        "only command / cwd / timeout_ms are accepted"
    );

    let result = harness
        .run_with(&context, json!({ "command": "echo hi", "shell": "cmd" }))
        .await;
    assert!(!result.ok);
    assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    assert!(
        result
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("unknown field"),
        "execute must fail through the same parse path"
    );
}

/// A38：方言只进 `data.sandbox.shell`，不进审批摘要。
#[tokio::test]
async fn a38_shell_flavor_is_diagnostic_only() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();
    let input = json!({ "command": script("echo hi", "echo hi") });
    let result = harness
        .run_with(&harness.ctx("call-a38"), input.clone())
        .await;
    assert!(result.ok, "{:?}", result.error);

    let shell = data_of(&result)["sandbox"]["shell"]
        .as_str()
        .expect("sandbox.shell")
        .to_string();
    assert!(
        !shell.contains('/') && !shell.contains('\\'),
        "only the flavor, never a path: {shell}"
    );
    assert!(matches!(shell.as_str(), "cmd" | "sh"), "shell={shell}");
    #[cfg(windows)]
    assert_eq!(shell, "cmd");
    #[cfg(unix)]
    assert_eq!(shell, "sh");

    let preview = harness
        .tool
        .preview_for_approval(&input, &harness.ctx("call-a38-preview"))
        .await
        .expect("preview");
    assert!(preview.estimated, "impact is unknown before execution");
    assert!(!preview.truncated);
    assert!(
        preview.summary.contains("隔离："),
        "summary={}",
        preview.summary
    );
    assert!(
        !preview.summary.contains("cmd"),
        "approval summary must not carry the shell: {}",
        preview.summary
    );
}

/// A39（放弃 future）：agent 侧取消是「丢弃 future」——`cancellable` 的 `select!`
/// 先到即丢弃，本工具的终态分支（第 8–10 步）永不执行。
///
/// 因此守卫必须在 drop 路径补「杀整棵树 + 删调用级临时目录」，
/// 落盘文件由 `FileSpillSink::Drop` 在最后一个读取任务释放时删除。
#[tokio::test]
async fn a39_abandoned_future_reclaims_tree_scratch_and_spill() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();
    let call_id = "call-a39-abandon";
    let tool = harness.tool.clone();
    let context = harness.ctx(call_id);
    let root = harness.root();
    // 延迟写：树被杀则永不落地，树存活则 ~5s 后出现（只用来判定「杀没杀」）。
    let input = json!({
        "command": script(
            "ping -n 6 127.0.0.1 >nul & echo LEAKED> leaked.txt",
            "sleep 5; echo LEAKED > leaked.txt"
        )
    });

    let handle = tokio::spawn(async move { tool.execute(&input, &context).await });
    // 等进程起来、产出首批字节后丢弃 future。
    tokio::time::sleep(Duration::from_millis(800)).await;
    handle.abort();
    let _ = handle.await;

    assert!(
        !harness.scratch(call_id).exists(),
        "drop path must remove the call-scoped scratch dir"
    );

    // 树若没被杀，延迟写会在 ~5s 后落地；同时给读取任务留出释放 sink 的时间。
    tokio::time::sleep(Duration::from_secs(7)).await;
    assert!(
        !root.join("leaked.txt").exists(),
        "the process tree must be killed when the future is dropped"
    );
    assert!(
        !has_spill_file(&harness.storage_root(), call_id),
        "drop path must remove the abandoned spill file"
    );
}

/// 落盘目录下是否存在文件名含 `call_id` 的条目（文件名以 sanitize 后的调用 id 结尾）。
fn has_spill_file(root: &std::path::Path, call_id: &str) -> bool {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.contains(call_id) {
                return true;
            }
        }
    }
    false
}

/// A33（模型侧）：`description()` 必须包含 I56 §11.2 的 6 项约束。
///
/// 模型不读规格，只读这段描述；缺一项就是模型会用错的地方（§11.2 表格右列）。
/// 用户侧短句（`tool_desc.shell` 的 zh / en）是**另一个断言对象**，
/// 由 `src/chat/tool/__tests__/panelStructure.test.ts` 覆盖——面板悬停放不下这 6 句。
#[tokio::test]
async fn a33_description_covers_the_six_contracts() {
    let harness = Harness::new();
    let storage_root = tempfile::tempdir().expect("storage tempdir");
    let backend: Arc<dyn ToolResultStorage> =
        Arc::new(FileToolResultStorage::new(storage_root.path()));
    let description = ShellTool::new(harness.probe, backend)
        .description()
        .await
        .expect("description")
        .to_lowercase();

    let contracts: [(&str, &[&str]); 6] = [
        (
            "非零退出码不是工具失败",
            &["non-zero exit code", "not a tool failure"],
        ),
        ("无 stdin / 无 PTY", &["no stdin", "no pty"]),
        (
            "Windows 是 cmd 语法",
            &["cmd", "single quotes are not quotes", "not a separator"],
        ),
        (
            "cwd 缺省工作区根且不可越界",
            &["defaults to the workspace root", "stay inside it"],
        ),
        (
            "输出落盘、用 read 读 outputPath",
            &["outputpath", "instead of re-running"],
        ),
        (
            "长任务显式调大 timeout_ms（含默认与上限）",
            &["timeout_ms", "120000", "1800000"],
        ),
    ];

    for (label, keywords) in contracts {
        for keyword in keywords {
            assert!(
                description.contains(keyword),
                "描述缺少「{}」的关键词 `{}`：{}",
                label,
                keyword,
                description
            );
        }
    }
}

/// A6 回归：同一份输入在 `validate_input` / `permission_intents` / `execute` 之间
/// 得到同一条命令文本（审批键与执行体不得分叉）。
#[tokio::test]
async fn a6_command_text_is_shared_by_validation_intents_and_execution() {
    let _guard = EXEC_LOCK.lock().await;
    let harness = Harness::new();
    let command = script(
        "echo  spaced  & echo second",
        "echo '  spaced  '; echo second",
    );
    let input = json!({ "command": command });

    let validation = harness.tool.validate_input(&input, &harness.ctx("call-a6"));
    assert!(validation.ok, "{:?}", validation.message);

    let intents = harness
        .tool
        .permission_intents(&input, &harness.ctx("call-a6"))
        .expect("intents");
    assert_eq!(intents.len(), 1, "one command is one intent");
    assert_eq!(intents[0].resources, vec![command.clone()]);

    let result = harness.run_with(&harness.ctx("call-a6"), input).await;
    assert!(result.ok, "{:?}", result.error);
    let file = std::fs::read_to_string(output_path(&result)).expect("read spilled output");
    assert!(
        file.contains("second"),
        "the executed body is the same text: {file}"
    );
}
