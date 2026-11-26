//! `SandboxPlan` 推导：shell 程序候选解析、命令拼装、环境变量块（I56 §7.3 / §7.4）。
//!
//! 三条硬约束：候选集只含系统目录与 `COMSPEC`（不遍历 `PATH`）、`COMSPEC` 必须校验
//! basename 为 `cmd.exe`、解析结果**不进审批键**（只进 `data.sandbox.shell` 与日志）。

use std::path::{Path, PathBuf};

use crate::runtime::scratch::ToolScratchStore;
use crate::sandbox::SandboxPlan;

use super::schema::ShellInput;

/// 本次实际执行的 shell 种类（诊断字段，只写种类、不写路径）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellFlavor {
    Cmd,
    Sh,
}

impl ShellFlavor {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cmd => "cmd",
            Self::Sh => "sh",
        }
    }

    /// 本机方言：与 [`program_candidates`] 的平台分派同源，审批预览用它解析变量语法。
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Cmd
        } else {
            Self::Sh
        }
    }

    /// 该方言的变量展开标记：`cmd` 用 `%X%`、`sh` 用 `$X`。
    pub const fn variable_marker(self) -> char {
        match self {
            Self::Cmd => '%',
            Self::Sh => '$',
        }
    }
}

/// 一次 `execute` 的执行输入：沙箱计划 + 方言 + 已建好的会话临时区。
#[derive(Debug, Clone)]
pub struct ShellSpawnPlan {
    pub plan: SandboxPlan,
    pub flavor: ShellFlavor,
    /// 会话临时区路径（由所有者 `prepare` 返回），回填与清单都以它为准。
    pub scratch_root: PathBuf,
}

/// 推导失败的两档：程序不可用（注册期就该挡住）与临时目录建立失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpawnPlanError {
    ProgramUnavailable(String),
    ScratchDirFailed(String),
}

impl SpawnPlanError {
    /// 稳定的错误码（回填给模型的 `error` 文本用它开头）。
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProgramUnavailable(_) => "sandbox_unavailable",
            Self::ScratchDirFailed(_) => "scratch_dir_failed",
        }
    }

    pub fn detail(&self) -> &str {
        match self {
            Self::ProgramUnavailable(detail) | Self::ScratchDirFailed(detail) => detail,
        }
    }

    pub fn to_message(&self) -> String {
        format!("{}: {}", self.code(), self.detail())
    }
}

/// 推导执行计划：解析 shell 程序 → 幂等建会话临时区 → 拼装 plan 与环境块。
///
/// 临时区必须在 `spawn` 之前建好：命令的第一个写临时文件动作就要用到它，
/// 建不出来就不该起进程（否则命令会以「Access is denied」失败而原因不可见）。
/// 建目录**只能**经所有者 [`ToolScratchStore::prepare`]（I77 INV3）：本模块不再自己
/// `create_dir_all`，也不再拼会话段。临时区是**会话级单目录**，跨调用复用、
/// 不在调用结束时删除（I73 §3.8）。
pub fn build_plan(
    input: &ShellInput,
    scratch: &dyn ToolScratchStore,
    session_id: &str,
) -> Result<ShellSpawnPlan, SpawnPlanError> {
    let program = resolve_program().map_err(SpawnPlanError::ProgramUnavailable)?;

    // 目录必须经所有者创建（I77 INV3）：本模块不 `create_dir_all`、不拼会话段。
    // 但子进程的 `TEMP` / `TMP` / `TMPDIR` 与沙箱写根要用**注入点归一过**的根
    // （`ToolUseContext.tool_scratch_root`，见 `adapters::tool_exec` 的 canonicalize），
    // 不能用所有者返回的原始基根形态：macOS 上基根常经 `/var` 符号链接给出，
    // 归一前 `/var/...`、归一后 `/private/var/...`，同指一个目录但字符串不同，
    // 会让子进程的 `TMPDIR` 与 `scratch://` 引用解析出的根对不上。
    scratch.prepare(session_id).map_err(|error| {
        SpawnPlanError::ScratchDirFailed(format!("create scratch dir failed: {}", error))
    })?;
    let scratch_root = input.scratch_root.clone();

    let args = program.args_for(&input.command);

    let plan = SandboxPlan {
        program: program.path,
        args,
        cwd: input.cwd.clone(),
        // 两个可写根并列：工作区根 + 会话临时区根，逐根授权（I73 §3.8 W1）。
        write_roots: vec![input.writable_root.clone(), scratch_root.clone()],
        env: env_block(&scratch_root),
        // P14 恒 false：不代表「命令不能联网」（§8.4 的 `sandbox.network` 另行求交）。
        network: false,
        // 无 stdin 通道。
        stdin: None,
    };

    Ok(ShellSpawnPlan {
        plan,
        flavor: program.flavor,
        scratch_root,
    })
}

/// 注册期与运行期共用的可用性判据（§7.3 第 3 步）。
pub fn program_available() -> bool {
    resolve_program().is_ok()
}

#[derive(Debug)]
struct ResolvedProgram {
    path: PathBuf,
    flavor: ShellFlavor,
}

impl ResolvedProgram {
    /// 参数形状：`cmd /d /C <整段命令>`、`sh -c <整段命令>`。
    ///
    /// `/d` 关掉 AutoRun（注册表里的 AutoRun 脚本会给每次调用塞前缀命令）。
    fn args_for(&self, command: &str) -> Vec<String> {
        match self.flavor {
            ShellFlavor::Cmd => vec!["/d".to_string(), "/C".to_string(), command.to_string()],
            ShellFlavor::Sh => vec!["-c".to_string(), command.to_string()],
        }
    }
}

/// 按平台的有序候选解析 shell 绝对路径；候选全不存在即 `Err`（fail-closed，不降级为裸名）。
fn resolve_program() -> Result<ResolvedProgram, String> {
    let candidates = program_candidates(&process_env);
    let exists = |path: &Path| path.is_file();
    resolve_from(&candidates, &exists)
}

fn resolve_from(
    candidates: &[(PathBuf, ShellFlavor)],
    exists: &dyn Fn(&Path) -> bool,
) -> Result<ResolvedProgram, String> {
    for (path, flavor) in candidates {
        if exists(path) {
            return Ok(ResolvedProgram {
                path: path.clone(),
                flavor: *flavor,
            });
        }
    }
    Err(match candidates.first().map(|(_, flavor)| *flavor) {
        Some(ShellFlavor::Cmd) => "cmd_not_found".to_string(),
        _ => "sh_not_found".to_string(),
    })
}

/// 读进程环境：只读、缺失即 `None`（空串按缺失处理）。
fn process_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

/// Windows 候选：`%SystemRoot%\System32\cmd.exe` → `%COMSPEC%`（仅当 basename 为 `cmd.exe`）。
///
/// `SystemRoot` 缺失时按 `C:\Windows` 兜底；`COMSPEC` 是用户可改写变量，
/// 不校验 basename 等于「shell 种类可被环境变量替换」。
#[cfg(windows)]
fn program_candidates(get: &dyn Fn(&str) -> Option<String>) -> Vec<(PathBuf, ShellFlavor)> {
    use crate::sandbox::is_cmd_program;

    let mut candidates = Vec::new();
    let system_root = get("SystemRoot").unwrap_or_else(|| r"C:\Windows".to_string());
    candidates.push((
        Path::new(&system_root).join("System32").join("cmd.exe"),
        ShellFlavor::Cmd,
    ));
    if let Some(comspec) = get("COMSPEC") {
        if is_cmd_program(&comspec) {
            candidates.push((PathBuf::from(comspec), ShellFlavor::Cmd));
        }
    }
    candidates
}

/// Unix 候选：只取 `/bin/sh`，不跟随 `$SHELL`（shell 种类不得随机器漂移）。
#[cfg(not(windows))]
fn program_candidates(_get: &dyn Fn(&str) -> Option<String>) -> Vec<(PathBuf, ShellFlavor)> {
    vec![(PathBuf::from("/bin/sh"), ShellFlavor::Sh)]
}

/// 透传白名单（不存在则跳过）。两条刻意排除见 §7.4：`TEMP` / `TMP` / `TMPDIR`
/// 不透传（父进程临时目录在可写根之外），工具链路径变量一律不注入。
#[cfg(windows)]
const PASSTHROUGH: &[&str] = &[
    "SystemRoot",
    "SystemDrive",
    "PATH",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "COMSPEC",
    "PATHEXT",
    "OS",
    "PROCESSOR_ARCHITECTURE",
    "NUMBER_OF_PROCESSORS",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "ProgramData",
];

#[cfg(not(windows))]
const PASSTHROUGH: &[&str] = &["PATH", "HOME", "LANG", "LC_ALL"];

/// 敏感名片段（大小写不敏感）：命中一律不透传。
const SENSITIVE_FRAGMENTS: &[&str] = &["KEY", "TOKEN", "SECRET", "PASSWORD", "CREDENTIAL"];

/// 环境块：白名单透传 + 强制注入（强制项覆盖同名透传项）。
fn env_block(scratch: &Path) -> Vec<(String, String)> {
    let forced = forced_env(scratch);
    let mut entries: Vec<(String, String)> = Vec::new();
    for key in PASSTHROUGH {
        if is_sensitive_name(key) {
            continue;
        }
        if forced
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(key))
        {
            continue;
        }
        if let Some(value) = process_env(key) {
            entries.push((key.to_string(), value));
        }
    }
    entries.extend(forced);
    entries
}

/// 强制注入：消除交互式挂起与渲染噪声、把临时文件收进可写根、切断 git 从外部拉程序。
///
/// 三者都不改变命令语义；**不得**注入 `CI=1` 一类会改变构建行为的变量。
fn forced_env(scratch: &Path) -> Vec<(String, String)> {
    let scratch = plain_path_text(scratch);
    let mut entries = vec![
        ("TERM".to_string(), "dumb".to_string()),
        ("NO_COLOR".to_string(), "1".to_string()),
        ("PAGER".to_string(), "cat".to_string()),
        ("GIT_PAGER".to_string(), "cat".to_string()),
        ("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()),
        ("TEMP".to_string(), scratch.clone()),
        ("TMP".to_string(), scratch.clone()),
        ("TMPDIR".to_string(), scratch),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
        ("GIT_CONFIG_COUNT".to_string(), "1".to_string()),
        ("GIT_CONFIG_KEY_0".to_string(), "core.fsmonitor".to_string()),
        ("GIT_CONFIG_VALUE_0".to_string(), "false".to_string()),
    ];
    // 白名单结果已按同名过滤，这里再排序只为日志与断言稳定。
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

/// 名字（大小写不敏感）命中敏感片段或 `ASSISTANT_` 前缀即不透传。
fn is_sensitive_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    if upper.starts_with("ASSISTANT_") {
        return true;
    }
    SENSITIVE_FRAGMENTS
        .iter()
        .any(|fragment| upper.contains(fragment))
}

/// 交给子进程的路径文本：剥掉 `\\?\` / `\\?\UNC\` 前缀。
///
/// 临时目录按 canonicalize 后的可写根拼出，Windows 上带扩展长度前缀；
/// `cmd.exe` 会把该前缀当成 UNC 起点。ACL 侧仍用原路径，剥前缀只影响子进程的解析。
fn plain_path_text(path: &Path) -> String {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\UNC\") {
        Some(rest) => format!(r"\\{rest}"),
        None => match text.strip_prefix(r"\\?\") {
            Some(rest) => rest.to_string(),
            None => text.into_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_path_strips_extended_prefix() {
        assert_eq!(plain_path_text(Path::new(r"\\?\C:\work")), r"C:\work");
        assert_eq!(
            plain_path_text(Path::new(r"\\?\UNC\srv\share\work")),
            r"\\srv\share\work"
        );
        assert_eq!(plain_path_text(Path::new(r"C:\work")), r"C:\work");
    }

    #[test]
    fn sensitive_names_are_not_passed_through() {
        for name in [
            "API_KEY",
            "GITHUB_TOKEN",
            "DB_PASSWORD",
            "AWS_CREDENTIAL",
            "ASSISTANT_X",
        ] {
            assert!(is_sensitive_name(name), "{name}");
        }
        for name in ["PATH", "HOME", "SystemRoot", "ProgramFiles"] {
            assert!(!is_sensitive_name(name), "{name}");
        }
    }

    #[test]
    fn forced_env_carries_scratch_dir() {
        let env = forced_env(Path::new("/work/.assistant-tmp/c1"));
        let temp = env.iter().find(|(key, _)| key == "TEMP").expect("TEMP");
        assert_eq!(temp.1, "/work/.assistant-tmp/c1");
        for key in ["TMP", "TMPDIR"] {
            let entry = env.iter().find(|(name, _)| name == key).expect(key);
            assert_eq!(entry.1, "/work/.assistant-tmp/c1");
        }
        assert!(env
            .iter()
            .any(|(key, value)| key == "TERM" && value == "dumb"));
        assert!(env
            .iter()
            .any(|(key, value)| key == "GIT_TERMINAL_PROMPT" && value == "0"));
        assert!(env
            .iter()
            .any(|(key, value)| key == "GIT_CONFIG_KEY_0" && value == "core.fsmonitor"));
    }

    #[cfg(windows)]
    #[test]
    fn cmd_candidates_prefer_system32_then_comspec() {
        let get = |key: &str| match key {
            "SystemRoot" => Some(r"C:\Windows".to_string()),
            "COMSPEC" => Some(r"C:\Windows\System32\cmd.exe".to_string()),
            _ => None,
        };
        let candidates = program_candidates(&get);
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].1, ShellFlavor::Cmd);
        assert!(candidates[0].0.ends_with("System32/cmd.exe"));
    }

    #[cfg(windows)]
    #[test]
    fn comspec_is_rejected_when_not_cmd() {
        let get = |key: &str| match key {
            "COMSPEC" => Some(r"C:\Windows\System32\powershell.exe".to_string()),
            _ => None,
        };
        let candidates = program_candidates(&get);
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].0.ends_with("System32/cmd.exe"));
    }

    #[cfg(windows)]
    #[test]
    fn missing_system_root_falls_back_to_comspec() {
        let get = |key: &str| match key {
            "COMSPEC" => Some(r"D:\bin\cmd.exe".to_string()),
            _ => None,
        };
        let candidates = program_candidates(&get);
        let resolved = resolve_from(&candidates, &|path: &Path| {
            path == Path::new(r"D:\bin\cmd.exe")
        })
        .expect("comspec fallback");
        assert_eq!(resolved.path, PathBuf::from(r"D:\bin\cmd.exe"));
        assert_eq!(resolved.flavor, ShellFlavor::Cmd);
    }

    #[cfg(windows)]
    #[test]
    fn no_candidate_is_unavailable() {
        let get = |_key: &str| None;
        let candidates = program_candidates(&get);
        let error = resolve_from(&candidates, &|_path: &Path| false).expect_err("unavailable");
        assert_eq!(error, "cmd_not_found");
    }

    #[cfg(windows)]
    #[test]
    fn cmd_args_are_slash_d_slash_c_script() {
        let resolved = resolve_from(
            &[(
                PathBuf::from(r"C:\Windows\System32\cmd.exe"),
                ShellFlavor::Cmd,
            )],
            &|_path: &Path| true,
        )
        .expect("cmd");
        assert_eq!(
            resolved.args_for("echo hi"),
            vec!["/d".to_string(), "/C".to_string(), "echo hi".to_string()]
        );
    }
}
