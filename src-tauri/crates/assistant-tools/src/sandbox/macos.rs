//! macOS 后端：外部 `sandbox-exec` + SBPL profile（规格 §5.3）。
//!
//! profile 形状固定为「`(deny default)` 起手 → 最小放行 → 敏感目录后置 `deny` → 网络整拒」。
//! 与参考实现（`deepseek-harness` 的 `seatbeltProfileArgs`）的差异在于起点：
//! 参考实现以 `(allow default)` 起手、只收写边界；本规格要求四个维度全部强制
//! （§5.2 平台表），因此必须从全拒起手并显式列出可放行的最小集。
//!
//! SBPL 是 last-match-wins：后置的 `deny file-read*` 覆盖前面的 `allow file-read*`，
//! 「全盘可读、敏感目录除外」因此可以用正向规则表达（Landlock 的 allow-list
//! 反过来，白名单项罩住敏感目录时只能整条跳过）。

use std::io;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};

use crate::sandbox::{
    enforcement_from_missing, is_executable, paths_overlap, SandboxBackend, SandboxChild,
    SandboxChildOps, SandboxError, SandboxHostPaths, SandboxKind, SandboxPlan, SandboxProbe,
};

/// 启动器路径硬编码：不经 `PATH` 查找，防同名劫持（§5.3）。
const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

/// 探测用的被包裹程序：由系统提供，`probe()` 只需确认 profile 被内核接受。
const PROBE_PROGRAM: &str = "/usr/bin/true";

/// 设备写白名单：`> /dev/null`、`2>/dev/tty` 是 shell 常态；
/// `/dev/stdout`、`/dev/stderr` 是 `/dev/fd/*` 的符号链接，故授 `/dev/fd` 子树。
const DEVICE_WRITE_CLAUSES: &str = concat!(
    r#"(literal "/dev/null") "#,
    r#"(literal "/dev/zero") "#,
    r#"(literal "/dev/random") "#,
    r#"(literal "/dev/urandom") "#,
    r#"(literal "/dev/tty") "#,
    r#"(subpath "/dev/fd")"#,
);

pub struct SeatbeltBackend {
    host: SandboxHostPaths,
}

impl SeatbeltBackend {
    pub fn new(host: SandboxHostPaths) -> Self {
        Self { host }
    }
}

impl SandboxBackend for SeatbeltBackend {
    fn kind(&self) -> SandboxKind {
        SandboxKind::Seatbelt
    }

    fn probe(&self) -> SandboxProbe {
        if self.host.sensitive_read_deny.is_empty() {
            return SandboxProbe::unavailable(SandboxKind::Seatbelt, "sensitive_list_empty");
        }
        if !is_executable(Path::new(SANDBOX_EXEC)) {
            return SandboxProbe::unavailable(SandboxKind::Seatbelt, "sandbox_exec_missing");
        }
        // 无写根时生成「只读 + 敏感拒绝」的 profile：结构与 spawn 完全同源，
        // 差别仅在于没有 `writable_root` 那一条。
        let profile = match build_profile(&self.host, None) {
            Ok(profile) => profile,
            Err(err) => return SandboxProbe::unavailable(SandboxKind::Seatbelt, err.detail()),
        };
        // SBPL 是否被内核接受只能靠试跑判定：`sandbox_init` 拒绝 profile 时
        // `sandbox-exec` 以非 0 退出。这是 probe 允许创建进程的唯一例外。
        let accepted = Command::new(SANDBOX_EXEC)
            .arg("-p")
            .arg(&profile)
            .arg("--")
            .arg(PROBE_PROGRAM)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if !accepted {
            return SandboxProbe::unavailable(SandboxKind::Seatbelt, "probe_profile_rejected");
        }
        // Seatbelt 同时强制写边界、敏感读拒绝、进程组与网络，四维度齐备。
        SandboxProbe {
            kind: SandboxKind::Seatbelt,
            enforcement: enforcement_from_missing(&[]),
            missing: Vec::new(),
            detail: None,
        }
    }

    fn spawn(&self, plan: &SandboxPlan) -> Result<SandboxChild, SandboxError> {
        if !plan.cwd.starts_with(&plan.writable_root) {
            return Err(SandboxError::SetupFailed("cwd_outside_root".to_string()));
        }
        if !plan.cwd.is_dir() {
            return Err(SandboxError::SpawnFailed("cwd_missing".to_string()));
        }
        if !is_executable(&plan.program) {
            return Err(SandboxError::SpawnFailed(
                "program_not_executable".to_string(),
            ));
        }
        if !is_executable(Path::new(SANDBOX_EXEC)) {
            return Err(SandboxError::SpawnFailed(
                "sandbox_exec_missing".to_string(),
            ));
        }
        // profile 生成阶段已含相交判定（授写权必然连带读权，相交即无解）。
        let profile = build_profile(&self.host, Some(&plan.writable_root))?;

        let mut command = Command::new(SANDBOX_EXEC);
        command
            .arg("-p")
            .arg(&profile)
            .arg("--")
            .arg(&plan.program)
            .args(&plan.args)
            .current_dir(&plan.cwd);
        command.env_clear();
        for (key, value) in &plan.env {
            command.env(key, value);
        }
        if !plan.env.iter().any(|(key, _)| key == "PATH") {
            if let Ok(path) = std::env::var("PATH") {
                command.env("PATH", path);
            }
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // SAFETY: 闭包只做 `setsid(2)`，async-signal-safe，不分配堆内存、不加锁；
        // 失败即返回 `Err`，由 std 在子进程内 `_exit(127)` 收口，父进程拿不到未隔离的子进程。
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }

        let child = command
            .spawn()
            .map_err(|_| SandboxError::RestrictFailed("pre_exec_setsid".to_string()))?;
        // `setsid(2)` 后子进程 pid 即进程组 id，回收整棵树只需负号 kill。
        let pgid = child.id() as libc::pid_t;
        Ok(SandboxChild::new(Box::new(SeatbeltChild { child, pgid })))
    }
}

struct SeatbeltChild {
    child: Child,
    pgid: libc::pid_t,
}

impl SandboxChildOps for SeatbeltChild {
    fn take_stdout(&mut self) -> Option<Box<dyn std::io::Read + Send>> {
        self.child.stdout.take().map(|out| Box::new(out) as _)
    }

    fn take_stderr(&mut self) -> Option<Box<dyn std::io::Read + Send>> {
        self.child.stderr.take().map(|err| Box::new(err) as _)
    }

    fn id(&self) -> u32 {
        self.child.id()
    }

    fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait()
    }

    fn kill_tree(&mut self) {
        // SAFETY: 负 pid 表示「向整个进程组发信号」；进程组可能已空，
        // 返回 `ESRCH` 属于幂等路径，忽略返回值即可。
        unsafe {
            libc::kill(-self.pgid, libc::SIGKILL);
        }
        let _ = self.child.kill();
    }
}

impl Drop for SeatbeltChild {
    fn drop(&mut self) {
        self.kill_tree();
        let _ = self.child.wait();
    }
}

/// 生成 SBPL profile；`writable_root` 为 `None` 时得到「只读」形态（供 `probe()` 使用）。
fn build_profile(
    host: &SandboxHostPaths,
    writable_root: Option<&Path>,
) -> Result<String, SandboxError> {
    let sensitive: Vec<PathBuf> = host
        .sensitive_read_deny
        .iter()
        .map(|path| profile_path(path))
        .collect();
    let writable = writable_root.map(profile_path);

    if let Some(root) = writable.as_deref() {
        let overlaps_sensitive = sensitive.iter().any(|item| paths_overlap(root, item));
        if overlaps_sensitive {
            return Err(SandboxError::SetupFailed(
                "writable_root_overlaps_sensitive".to_string(),
            ));
        }
    }
    for path in writable.iter().chain(sensitive.iter()) {
        reject_control_chars(path)?;
    }

    let mut forms: Vec<String> = Vec::with_capacity(10);
    // `(version 1)` 必须是第一条：SBPL 解析器要求先声明版本。
    forms.push("(version 1)".to_string());
    forms.push("(deny default)".to_string());
    // 最小放行集：派生子进程、执行目标程序、同沙箱内互发信号（作业控制）、
    // 读 sysctl（libSystem / dyld 初始化）、查找 Mach 服务。
    // `mach-lookup` 无法收敛到服务名粒度（依赖的服务名随系统版本变化），
    // 整类放开只影响 IPC 名字空间，不构成文件或网络边界缺口。
    forms.push("(allow process-fork)".to_string());
    forms.push("(allow process-exec*)".to_string());
    forms.push("(allow signal (target same-sandbox))".to_string());
    forms.push("(allow sysctl-read)".to_string());
    forms.push("(allow mach-lookup)".to_string());
    // 读：全盘可读，敏感目录随后被 `deny` 覆盖（last-match-wins）。
    forms.push("(allow file-read*)".to_string());
    if !sensitive.is_empty() {
        // 不按 exists 过滤：探测时点不存在、之后被创建的敏感目录同样必须被拒。
        let clauses: Vec<String> = sensitive
            .iter()
            .map(|path| format!("(subpath {})", sbpl_string(path)))
            .collect();
        forms.push(format!("(deny file-read* {})", clauses.join(" ")));
    }
    forms.push(format!("(allow file-write* {DEVICE_WRITE_CLAUSES})"));
    if let Some(root) = writable.as_deref() {
        forms.push(format!(
            "(allow file-write* (subpath {}))",
            sbpl_string(root)
        ));
    }
    // 网络整类拒绝：`(deny default)` 已覆盖，显式写出以固定语义、防止前置 allow 放宽。
    forms.push("(deny network*)".to_string());

    Ok(forms.join(" "))
}

/// 敏感路径的 profile 形式：存在时取 canonical。
///
/// Seatbelt 在 vnode 层匹配真实路径，`~/.aws -> /mnt/x` 这类符号链接目录
/// 必须展开成真实子树才覆盖得到；不存在时保留原样（fail-closed 仍成立）。
fn profile_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// 控制字符会截断 / 破坏 SBPL 字符串字面量，含此类字符的路径直接拒绝生成 profile。
fn reject_control_chars(path: &Path) -> Result<(), SandboxError> {
    if path.to_string_lossy().chars().any(char::is_control) {
        return Err(SandboxError::SetupFailed(
            "profile_path_control_char".to_string(),
        ));
    }
    Ok(())
}

/// 把路径渲染成 SBPL 字符串字面量：`\` 与 `"` 必须转义，否则 profile 会被注入或截断。
fn sbpl_string(path: &Path) -> String {
    let text = path.to_string_lossy();
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use tempfile::TempDir;

    use super::*;
    use crate::sandbox::SandboxEnforcement;

    fn host_with_sensitive(sensitive: Vec<PathBuf>) -> SandboxHostPaths {
        SandboxHostPaths {
            sensitive_read_deny: sensitive,
        }
    }

    fn workspace() -> (TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("workspace");
        std::fs::create_dir_all(&root).expect("workspace");
        let root = std::fs::canonicalize(&root).expect("canonicalize");
        (dir, root)
    }

    fn plan_for(root: &Path, script: &str) -> SandboxPlan {
        SandboxPlan {
            program: PathBuf::from("/bin/sh"),
            args: vec!["-c".to_string(), script.to_string()],
            cwd: root.to_path_buf(),
            writable_root: root.to_path_buf(),
            env: vec![("PATH".to_string(), "/usr/bin:/bin".to_string())],
            network: false,
            stdin: None,
        }
    }

    fn collect(mut child: SandboxChild) -> (Option<i32>, String, String) {
        let mut stdout = String::new();
        let mut stderr = String::new();
        if let Some(mut stream) = child.stdout() {
            let _ = stream.read_to_string(&mut stdout);
        }
        if let Some(mut stream) = child.stderr() {
            let _ = stream.read_to_string(&mut stderr);
        }
        let code = child.wait().ok().and_then(|status| status.code());
        (code, stdout, stderr)
    }

    fn assert_sandbox_ready(backend: &SeatbeltBackend) {
        let probe = backend.probe();
        assert_eq!(
            probe.enforcement,
            SandboxEnforcement::Full,
            "Seatbelt 不可用，跳过：{:?}",
            probe.detail
        );
    }

    #[test]
    fn sbpl_string_escapes_quotes_and_backslashes() {
        assert_eq!(sbpl_string(Path::new("/a\"b")), "\"/a\\\"b\"");
        assert_eq!(sbpl_string(Path::new("/a\\b")), "\"/a\\\\b\"");
        assert_eq!(sbpl_string(Path::new("/plain")), "\"/plain\"");
    }

    #[test]
    fn profile_denies_sensitive_reads_after_allow() {
        let profile = build_profile(
            &host_with_sensitive(vec![PathBuf::from("/Users/a/.ssh")]),
            None,
        )
        .expect("profile");
        let allow = profile
            .find("(allow file-read*)")
            .expect("allow file-read*");
        let deny = profile.find("(deny file-read*").expect("deny file-read*");
        assert!(allow < deny, "敏感目录 deny 必须后置才生效：{profile}");
        assert!(profile.contains("(subpath \"/Users/a/.ssh\")"));
        assert!(profile.starts_with("(version 1) (deny default)"));
    }

    #[test]
    fn profile_grants_writable_root_and_devices() {
        let profile = build_profile(
            &host_with_sensitive(vec![PathBuf::from("/Users/a/.ssh")]),
            Some(Path::new("/Users/a/work")),
        )
        .expect("profile");
        assert!(profile.contains("(subpath \"/Users/a/work\")"));
        assert!(profile.contains("(literal \"/dev/null\")"));
        assert!(profile.contains("(deny network*)"));
    }

    #[test]
    fn profile_refuses_overlapping_root() {
        let host = host_with_sensitive(vec![PathBuf::from("/Users/a/work/secret")]);
        match build_profile(&host, Some(Path::new("/Users/a/work"))) {
            Err(SandboxError::SetupFailed(detail)) => {
                assert_eq!(detail, "writable_root_overlaps_sensitive");
            }
            other => panic!("预期 SetupFailed，实际 {other:?}"),
        }
    }

    #[test]
    fn profile_refuses_control_chars() {
        let host = host_with_sensitive(vec![PathBuf::from("/Users/a/we\nird")]);
        match build_profile(&host, None) {
            Err(SandboxError::SetupFailed(detail)) => {
                assert_eq!(detail, "profile_path_control_char");
            }
            other => panic!("预期 SetupFailed，实际 {other:?}"),
        }
    }

    #[test]
    fn a_s1_write_boundary() {
        let (temp, root) = workspace();
        let sensitive = temp.path().join("secret");
        std::fs::create_dir_all(&sensitive).expect("secret");
        let backend = SeatbeltBackend::new(host_with_sensitive(vec![sensitive]));
        assert_sandbox_ready(&backend);

        let outside = temp.path().join("outside.txt");
        let _ = std::fs::remove_file(&outside);
        let child = backend
            .spawn(&SandboxPlan {
                env: vec![
                    ("PATH".to_string(), "/usr/bin:/bin".to_string()),
                    ("OUT".to_string(), outside.display().to_string()),
                ],
                ..plan_for(&root, "echo inside > inside.txt; echo outside > \"$OUT\"")
            })
            .expect("spawn");
        let (code, stdout, stderr) = collect(child);
        assert!(
            root.join("inside.txt").exists(),
            "可写根内写失败：code={code:?} stdout={stdout} stderr={stderr}"
        );
        assert!(!outside.exists(), "可写根外写成功，边界失效");
        assert_eq!(code, Some(0));
    }

    #[test]
    fn a_s2_sensitive_read_deny() {
        let (temp, root) = workspace();
        let sensitive = temp.path().join("secret");
        std::fs::create_dir_all(&sensitive).expect("secret");
        std::fs::write(sensitive.join("token.txt"), "s3cret").expect("token");
        let backend = SeatbeltBackend::new(host_with_sensitive(vec![sensitive.clone()]));
        assert_sandbox_ready(&backend);

        let child = backend
            .spawn(&SandboxPlan {
                env: vec![
                    ("PATH".to_string(), "/usr/bin:/bin".to_string()),
                    (
                        "TOKEN".to_string(),
                        sensitive.join("token.txt").display().to_string(),
                    ),
                ],
                ..plan_for(&root, "cat \"$TOKEN\"")
            })
            .expect("spawn");
        let (code, stdout, stderr) = collect(child);
        assert_ne!(code, Some(0), "敏感目录读取未被拒绝");
        assert!(!stdout.contains("s3cret"), "敏感内容泄漏：{stdout}");
        assert!(!stderr.is_empty(), "拒绝读时 stderr 应可读");
    }

    #[test]
    fn a_s2_sensitive_overlap_refuses_to_spawn() {
        let (temp, _root) = workspace();
        let sensitive = temp.path().join("secret");
        std::fs::create_dir_all(&sensitive).expect("secret");
        let backend = SeatbeltBackend::new(host_with_sensitive(vec![sensitive.clone()]));
        assert_sandbox_ready(&backend);

        let plan = SandboxPlan {
            cwd: sensitive.clone(),
            writable_root: sensitive,
            ..plan_for(&temp.path().join("workspace"), "true")
        };
        match backend.spawn(&plan) {
            Err(SandboxError::SetupFailed(detail)) => {
                assert_eq!(detail, "writable_root_overlaps_sensitive");
            }
            other => panic!("预期 SetupFailed，实际 {other:?}"),
        }
    }

    #[test]
    fn a_s3_process_tree_reclaimed() {
        let (temp, root) = workspace();
        let sensitive = temp.path().join("secret");
        std::fs::create_dir_all(&sensitive).expect("secret");
        let backend = SeatbeltBackend::new(host_with_sensitive(vec![sensitive]));
        assert_sandbox_ready(&backend);

        let mut child = backend
            .spawn(&plan_for(&root, "sleep 30 & echo $! > bg.pid; wait"))
            .expect("spawn");
        let pid_file = root.join("bg.pid");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut bg_pid = String::new();
        while Instant::now() < deadline {
            if let Ok(text) = std::fs::read_to_string(&pid_file) {
                bg_pid = text.trim().to_string();
                if !bg_pid.is_empty() {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!bg_pid.is_empty(), "后台进程未启动");

        child.kill_tree();
        let status = child.wait().expect("wait");
        assert!(!status.success(), "被 SIGKILL 回收后不应是正常退出");
        child.kill_tree();

        let pid: i32 = bg_pid.parse().expect("pid");
        let gone_deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < gone_deadline {
            // SAFETY: 信号 0 只做存活探测，不向进程投递任何动作。
            if unsafe { libc::kill(pid, 0) } != 0 {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("后台进程 {pid} 仍存活，进程组回收失败");
    }
}
