//! `execute` 的十一步编排（I56 §7.1 / §7.5 / §8）。
//!
//! 顺序不是风格问题：**先建 sink、先接管管道、最后才进入等待**。反过来（先 `wait()`
//! 再读管道）会死锁——子进程写满匿名管道缓冲区后阻塞，父进程在 `wait()` 上等子进程退出。
//!
//! 本模块只做流程，不写 IO 细节：管道读取循环、落盘写入、编码判定都在存储层；
//! 本模块负责「何时开、何时读、何时杀、何时收尾」。

use std::io::Read;
use std::path::Path;
use std::process::ExitStatus;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::context::ToolUseContext;
use crate::result::{ToolErrorKind, ToolResult};
use crate::sandbox::{
    self, SandboxChild, SandboxDimension, SandboxEnforcement, SandboxError, SandboxProbe,
};
use crate::storage::{
    persisted_reference_from_tail, OutputStream, PersistToolResult, SpilledOutput, ToolResultSink,
    ToolResultStorage,
};

use super::deny::is_sandbox_denied;
use super::schema::{ShellInput, ShellInputError};
use super::spawn::{self, ShellSpawnPlan};

/// 管道单次读取块大小：与缓冲区上限无关，只决定「多久检查一次取消与 sink 是否还在」。
const READ_CHUNK_BYTES: usize = 64 * 1024;

/// 读取任务的有界 join 宽限（§7.1 第 9 步）。
const READER_JOIN_GRACE: Duration = Duration::from_secs(2);

/// 工具失败码（回填文本以它开头，模型与日志都按这几个串检索）。
const CODE_SANDBOX_UNAVAILABLE: &str = "sandbox_unavailable";
const CODE_OUTPUT_SPILL_FAILED: &str = "output_spill_failed";
const CODE_SANDBOX_SETUP_FAILED: &str = "sandbox_setup_failed";
const CODE_SPAWN_FAILED: &str = "spawn_failed";

/// 沙箱拒绝的固定回填行（§8.5）。
const DENIED_MARKER: &str = "[sandbox: write outside writable root denied]";

/// 执行入口。**永不返回 `Err`**：所有失败都归一为带 `error_kind` 的 `ToolResult`。
pub async fn execute(
    input: &Value,
    context: &ToolUseContext,
    probe: &'static SandboxProbe,
    storage: Arc<dyn ToolResultStorage>,
) -> ToolResult {
    let started = Instant::now();

    // 第 1 步：输入解析（与 `validate_input` 同一条路径，杜绝「校验 A、执行 B」）。
    let parsed = match ShellInput::parse(input, context) {
        Ok(parsed) => parsed,
        Err(error) => return ToolResult::err_with_kind(ToolErrorKind::Business, describe(&error)),
    };

    // 第 2 步：运行期读注册期缓存结论；`none` 拒绝执行，**不得**无沙箱执行。
    if probe.enforcement == SandboxEnforcement::None {
        return failure(
            CODE_SANDBOX_UNAVAILABLE,
            "sandbox enforcement is none; shell refuses to run outside a boundary",
        );
    }

    // 第 3 步：推导执行计划并建调用级临时目录。
    let call_id = context
        .tool_call_id
        .clone()
        .unwrap_or_else(|| format!("call-{}", next_call_seq()));
    let plan = match spawn::build_plan(&parsed, &call_id) {
        Ok(plan) => plan,
        Err(error) => {
            return failure(error.code(), &error.to_message());
        }
    };

    // 第 4 步：先建 sink（落盘是执行的前置条件，不是事后收尾）。
    let request = PersistToolResult {
        tool_call_id: call_id.clone(),
        tool_name: "shell".to_string(),
        session_id: context.session_id.clone(),
        content: String::new(),
    };
    let sink = match storage.open_sink(request).await {
        Ok(sink) => sink,
        Err(error) => {
            spawn::cleanup_scratch(&plan.scratch);
            return failure(CODE_OUTPUT_SPILL_FAILED, &error);
        }
    };
    // 两个读取任务并发写同一个 sink：临界区只有一次 `write`（§8.1 不变式 3）。
    let slot = Arc::new(Mutex::new(Some(sink)));
    let spill_failed = Arc::new(AtomicBool::new(false));

    // 第 5 步：在边界内创建进程。
    let mut child = match sandbox::spawn(&plan.plan) {
        Ok(child) => child,
        Err(error) => {
            discard_sink(slot, &plan.scratch).await;
            return failure(spawn_code(&error), error.detail());
        }
    };

    // 第 6 步：立刻接管两个管道，起两个并发读取任务（各占一个 blocking 线程）。
    let mut readers: Vec<tokio::task::JoinHandle<()>> = Vec::new();
    for (stream, handle) in [
        (OutputStream::Stdout, child.stdout()),
        (OutputStream::Stderr, child.stderr()),
    ] {
        if let Some(handle) = handle {
            let slot = slot.clone();
            let spill_failed = spill_failed.clone();
            readers.push(tokio::task::spawn_blocking(move || {
                drain(handle, stream, slot, spill_failed)
            }));
        }
    }

    // 第 7 步：`wait()` 丢进 blocking 池，async 侧与 deadline / 取消竞争。
    // 杀树句柄先取出：`wait()` 独占子进程时，超时与取消路径仍要能打断它。
    let kill = child.kill_handle();
    let mut wait_task = tokio::task::spawn_blocking(move || reap(child));

    let deadline = tokio::time::sleep(Duration::from_millis(parsed.timeout_ms));
    tokio::pin!(deadline);
    let cancel = context.cancel.clone();

    let outcome = match cancel.as_ref() {
        Some(token) => tokio::select! {
            joined = &mut wait_task => Outcome::Exited(joined),
            _ = &mut deadline => Outcome::TimedOut,
            _ = token.cancelled() => Outcome::Cancelled,
        },
        None => tokio::select! {
            joined = &mut wait_task => Outcome::Exited(joined),
            _ = &mut deadline => Outcome::TimedOut,
        },
    };

    // 第 8 步：任何终态都「杀 + 收尸」。超时 / 取消先杀树打断等待，再等收尸返回。
    // 正常结束不在这里补杀：`SandboxChild` 的 `Drop` 已做整树扫尾 + 收尸，
    // 而收尸任务返回前一定先经过 `Drop`，因此第 9 步 join 之前树已扫过。
    let (status, timed_out, cancelled) = match outcome {
        Outcome::Exited(joined) => (joined.ok().and_then(Result::ok), false, false),
        Outcome::TimedOut => {
            kill.kill();
            (wait_task.await.ok().and_then(Result::ok), true, false)
        }
        Outcome::Cancelled => {
            kill.kill();
            (wait_task.await.ok().and_then(Result::ok), false, true)
        }
    };

    // 第 9 步：有界 join 两个读取任务；仍无 EOF 即放弃读取句柄继续。
    let joined = tokio::time::timeout(READER_JOIN_GRACE, async {
        for handle in readers {
            let _ = handle.await;
        }
    })
    .await;
    if joined.is_err() {
        log::debug!(
            "shell readers did not reach EOF within {:?}; abandoning handles",
            READER_JOIN_GRACE
        );
    }

    // 第 10 步：`sink.finish()`——四种终态一律调用；临时目录同步删除。
    let spilled = match finish_sink(slot, &plan.scratch).await {
        Ok(spilled) => spilled,
        Err(error) => return failure(CODE_OUTPUT_SPILL_FAILED, &error),
    };
    if spill_failed.load(Ordering::Relaxed) {
        // 读取循环里写入已经失败过：此时 `finish` 的成功不能掩盖丢字节的事实。
        return failure(
            CODE_OUTPUT_SPILL_FAILED,
            "writing captured output failed; partial output was dropped",
        );
    }

    // 第 11 步：组装结果。
    let boundary = Boundary::new(probe, &plan);
    let exit_code = if timed_out || cancelled {
        // 超时 / 取消的退出状态由本工具造成，标记行已说明原因；
        // 这里不报被杀信号，避免把「我们杀的」读成「命令自己崩的」。
        None
    } else {
        status.as_ref().and_then(ExitStatus::code)
    };
    let signal = if timed_out || cancelled {
        None
    } else {
        status.as_ref().and_then(signal_of)
    };

    let denied = is_sandbox_denied(probe.enforcement, exit_code, &spilled.preview);
    let text = assemble_text(
        &spilled,
        denied,
        exit_code,
        signal,
        timed_out,
        cancelled,
        parsed.timeout_ms,
    );
    let data = json!({
        "exitCode": exit_code,
        "signal": signal,
        "outputPath": spilled.reference.as_ref().map(|reference| reference.path.clone()),
        "outputBytes": spilled.output_bytes,
        "outputLines": spilled.output_lines,
        "outputTruncated": spilled.truncated,
        "stderrTruncated": spilled.stderr_truncated,
        "durationMs": started.elapsed().as_millis() as u64,
        "outputEncoding": spilled.output_encoding.label(),
        "sandbox": {
            "backend": probe.kind.as_str(),
            "shell": plan.flavor.as_str(),
            "enforcement": probe.enforcement.as_str(),
            "network": boundary.network,
            "fsRead": boundary.fs_read,
            "denied": denied,
        },
    });
    let reference = spilled.reference.clone();

    if timed_out {
        return ToolResult::err_with_data(
            ToolErrorKind::Timeout,
            format!("timed out after {}ms", parsed.timeout_ms),
            data,
            text,
        )
        .with_persisted(reference);
    }
    if cancelled {
        return ToolResult::err_with_data(
            ToolErrorKind::Cancelled,
            "stopped: turn cancelled",
            data,
            text,
        )
        .with_persisted(reference);
    }
    ToolResult::ok_with_assistant(data, text).with_persisted(reference)
}

/// 等待竞争的结果。
enum Outcome {
    Exited(Result<std::io::Result<ExitStatus>, tokio::task::JoinError>),
    TimedOut,
    Cancelled,
}

/// 本次调用的真实边界（§8.4）：三个布尔不是注册期结论的复述，而是「命令实际能否」。
struct Boundary {
    network: bool,
    fs_read: bool,
}

impl Boundary {
    fn new(probe: &SandboxProbe, plan: &ShellSpawnPlan) -> Self {
        // `SandboxPlan.network` 是「希望怎样」，后端 `missing` 是「实际有没有」；
        // 两者求交才是命令的真实能力。Windows / Landlock ABI<4 恒 `false`。
        Self {
            network: plan.plan.network && !probe.missing.contains(&SandboxDimension::Network),
            fs_read: !probe.missing.contains(&SandboxDimension::FsRead),
        }
    }
}

/// 收尸任务：`wait()` 是同步阻塞调用，必须在 blocking 池里跑。
///
/// 子进程在函数返回时被 `Drop`：平台实现负责整树扫尾 + 再次收尸（幂等），
/// 因此本任务返回时「命令已退出」且「整棵树已回收」两件事都成立。
fn reap(mut child: SandboxChild) -> std::io::Result<ExitStatus> {
    child.wait()
}

/// 管道读取循环：逐块交给 sink，直到 EOF。
///
/// 写入失败**不中断读取**：一旦停止读管道，子进程写满缓冲区就会阻塞，
/// 父进程又在 `wait()` 上等它退出——死锁。失败只置标志并丢弃后续字节。
fn drain(
    mut reader: Box<dyn Read + Send>,
    stream: OutputStream,
    slot: Arc<Mutex<Option<Box<dyn ToolResultSink>>>>,
    spill_failed: Arc<AtomicBool>,
) {
    let mut buffer = vec![0_u8; READ_CHUNK_BYTES];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return,
            Ok(read) => {
                let mut guard = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                match guard.as_mut() {
                    // `finish` 已取走 sink：滞留线程的字节只能丢弃（§8.1 不变式 3）。
                    None => return,
                    Some(sink) => {
                        if sink.write(stream, &buffer[..read]).is_err() {
                            spill_failed.store(true, Ordering::Relaxed);
                        }
                    }
                }
            }
            Err(error) => {
                log::debug!("shell pipe read stopped: {}", error);
                return;
            }
        }
    }
}

/// 取走 sink 并收尾；调用方在 join 之后才 `take()`，**不得**用 `Arc::try_unwrap`。
async fn finish_sink(
    slot: Arc<Mutex<Option<Box<dyn ToolResultSink>>>>,
    scratch: &Path,
) -> Result<SpilledOutput, String> {
    let sink = slot
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    let scratch = scratch.to_path_buf();
    let joined = tokio::task::spawn_blocking(move || {
        let finished = match sink {
            Some(sink) => sink.finish(),
            None => Err("output sink was already taken".to_string()),
        };
        // 第 3 步建的目录在四种终态之后都不留（尽力而为，失败只记日志）。
        spawn::cleanup_scratch(&scratch);
        finished
    })
    .await;
    match joined {
        Ok(result) => result,
        Err(error) => Err(format!("output sink task failed: {}", error)),
    }
}

/// `spawn` 失败路径：已建 sink 按第 10 步同法收尾（空文件由 `finish` 删除），
/// 临时目录一并清掉。
async fn discard_sink(slot: Arc<Mutex<Option<Box<dyn ToolResultSink>>>>, scratch: &Path) {
    let _ = finish_sink(slot, scratch).await;
}

/// 失败回填：`code: detail` 前缀便于检索，`error_kind` 恒 `Business`。
fn failure(code: &str, detail: &str) -> ToolResult {
    ToolResult::err_with_kind(ToolErrorKind::Business, format!("{}: {}", code, detail))
}

/// 输入解析失败：带 `error_code` 的那一档前缀到文本里，模型才能区分「改写可过」与「不可绕过」。
fn describe(error: &ShellInputError) -> String {
    match error.code() {
        Some(code) => format!("{}: {}", code, error.message()),
        None => error.message().to_string(),
    }
}

fn spawn_code(error: &SandboxError) -> &'static str {
    match error {
        SandboxError::SetupFailed(_) => CODE_SANDBOX_SETUP_FAILED,
        SandboxError::SpawnFailed(_) | SandboxError::RestrictFailed(_) => CODE_SPAWN_FAILED,
    }
}

/// 回填文本：落盘引用（标签 + 尾窗口 + 省略标记）+ 末尾标记行。
///
/// 预览**只取尾窗口**：命令输出的信息密度集中在末尾（失败栈、测试摘要），
/// 头部多是启动横幅与进度条。标记顺序固定为「正文在前、标记在末尾」。
fn assemble_text(
    spilled: &SpilledOutput,
    denied: bool,
    exit_code: Option<i32>,
    signal: Option<i32>,
    timed_out: bool,
    cancelled: bool,
    timeout_ms: u64,
) -> String {
    let mut text = match &spilled.reference {
        Some(reference) => {
            persisted_reference_from_tail(&reference.path, &spilled.preview, spilled.total_chars)
        }
        // 空输出不落盘（§8.2 第 5 条）：给一句可判断的正文，而不是空串。
        None => "(no output)".to_string(),
    };
    if denied {
        text.push('\n');
        text.push_str(DENIED_MARKER);
    }
    if let Some(code) = exit_code.filter(|code| *code != 0) {
        text.push_str(&format!("\n[exit code: {}]", code));
    }
    if let Some(signal) = signal {
        text.push_str(&format!("\n[killed by signal: {}]", signal));
    }
    if timed_out {
        text.push_str(&format!("\n[timed out after {}ms]", timeout_ms));
    }
    if cancelled {
        text.push_str("\n[stopped: turn cancelled]");
    }
    text
}

/// Unix 侧被信号杀死时 `code()` 为 `None`，信号号另行取出；Windows 恒 `None`。
#[cfg(unix)]
fn signal_of(status: &ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn signal_of(_status: &ExitStatus) -> Option<i32> {
    None
}

/// 缺 `tool_call_id` 时的兜底序号：只用于命名临时目录，不承担任何语义。
fn next_call_seq() -> u64 {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    SEQ.fetch_add(1, Ordering::Relaxed)
}
