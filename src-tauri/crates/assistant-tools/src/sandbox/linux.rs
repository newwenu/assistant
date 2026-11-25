//! Linux 后端：Landlock LSM 文件系统规则 + 独立进程组。
//!
//! 规则形状见 I53 沙箱准备规格（已归档） §5.2（OQ10 已定稿，形状 a）：
//! 根 `/` 只授 `EXECUTE`，读权按固定白名单逐条授，写权只授 `write_roots` 各根。
//! 未列入白名单的路径（含用户主目录、敏感目录）默认不可读，因此敏感目录无需额外规则。

use std::io;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};

use landlock::{
    Access, AccessFs, AccessNet, BitFlags, PathBeneath, PathFd, Ruleset, RulesetAttr,
    RulesetCreated, RulesetCreatedAttr, RulesetStatus, ABI,
};

use crate::sandbox::{
    enforcement_from_missing, is_executable, paths_overlap, KillHandle, SandboxBackend,
    SandboxChild, SandboxChildOps, SandboxDimension, SandboxError, SandboxHostPaths, SandboxKind,
    SandboxPlan, SandboxProbe,
};

/// 根只授执行权，读权不在这里给。
const EXEC_ROOT: &str = "/";

/// 系统读白名单：命令解释器、动态库、配置、伪文件系统与设备信息。
/// 不含 `$HOME`，用户目录下的任何路径默认不可读（OQ10 形状 a 的收窄结论）。
const SYSTEM_READ_ROOTS: &[&str] = &[
    "/usr", "/bin", "/sbin", "/lib", "/lib32", "/lib64", "/etc", "/proc", "/sys", "/run", "/var",
    "/dev",
];

/// 需要写权的设备节点：shell 里 `> /dev/null`、`2>/dev/tty` 之类的重定向是常态。
const DEVICE_WRITE_FILES: &[&str] = &[
    "/dev/null",
    "/dev/zero",
    "/dev/full",
    "/dev/random",
    "/dev/urandom",
    "/dev/tty",
];

/// `landlock_create_ruleset` 的「只查 ABI」标志位。
const LANDLOCK_CREATE_RULESET_VERSION: libc::c_uint = 1;

pub struct LandlockBackend {
    host: SandboxHostPaths,
}

impl LandlockBackend {
    pub fn new(host: SandboxHostPaths) -> Self {
        Self { host }
    }
}

impl SandboxBackend for LandlockBackend {
    fn kind(&self) -> SandboxKind {
        SandboxKind::Landlock
    }

    fn probe(&self) -> SandboxProbe {
        // 敏感列表为空时判负：读边界本身来自白名单，但 `write_roots` 一旦罩住主目录，
        // 授全量 `AccessFs` 会连带把 `~/.ssh` 变成可读，而此时没有列表可做相交拦截。
        // 宁可让 `shell` 不注册，也不接受一份「无法判断是否越界」的授权。
        if self.host.sensitive_read_deny.is_empty() {
            return SandboxProbe::unavailable(SandboxKind::Landlock, "sensitive_list_empty");
        }
        let abi = match kernel_abi() {
            Ok(abi) => abi,
            Err(err) => return SandboxProbe::unavailable(SandboxKind::Landlock, err.detail()),
        };
        if abi <= ABI::Unsupported {
            return SandboxProbe::unavailable(SandboxKind::Landlock, "probe_landlock_unsupported");
        }
        if build_ruleset(&self.host, &[], abi).is_err() {
            return SandboxProbe::unavailable(SandboxKind::Landlock, "probe_ruleset_create");
        }
        let mut missing = Vec::new();
        if !network_supported(abi) {
            missing.push(SandboxDimension::Network);
        }
        SandboxProbe {
            kind: SandboxKind::Landlock,
            enforcement: enforcement_from_missing(&missing),
            missing,
            detail: None,
        }
    }

    fn spawn(&self, plan: &SandboxPlan) -> Result<SandboxChild, SandboxError> {
        if !plan
            .write_roots
            .iter()
            .any(|root| plan.cwd.starts_with(root))
        {
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
        // 可写根与敏感列表相交时无解：授全量写权必然连带读权，只能拒绝启动。
        if plan.write_roots.iter().any(|root| {
            self.host
                .sensitive_read_deny
                .iter()
                .any(|sensitive| paths_overlap(root, sensitive))
        }) {
            return Err(SandboxError::SetupFailed(
                "writable_root_overlaps_sensitive".to_string(),
            ));
        }
        let abi = kernel_abi()?;
        let ruleset = build_ruleset(&self.host, &plan.write_roots, abi)?;

        let mut command = Command::new(&plan.program);
        command.args(&plan.args).current_dir(&plan.cwd);
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

        let mut ruleset = Some(ruleset);
        // SAFETY: 闭包只做 `setsid(2)` 与 `restrict_self(2)` 两个 async-signal-safe 的系统调用，
        // 不分配堆内存、不加锁；失败即返回 `Err`，由 std 在子进程内 `_exit(127)` 收口。
        unsafe {
            command.pre_exec(move || {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                let ruleset = ruleset
                    .take()
                    .ok_or_else(|| io::Error::other("ruleset_consumed"))?;
                let status = ruleset
                    .restrict_self()
                    .map_err(|_| io::Error::other("restrict_self"))?;
                if !matches!(status.ruleset, RulesetStatus::FullyEnforced) {
                    return Err(io::Error::other("landlock_not_enforced"));
                }
                Ok(())
            });
        }

        let child = command
            .spawn()
            .map_err(|_| SandboxError::RestrictFailed("pre_exec_restrict".to_string()))?;
        // `setsid(2)` 后子进程 pid 即进程组 id，回收整棵树只需要负号 kill。
        let pgid = child.id() as libc::pid_t;
        Ok(SandboxChild::new(Box::new(LinuxChild { child, pgid })))
    }
}

struct LinuxChild {
    child: Child,
    pgid: libc::pid_t,
}

impl SandboxChildOps for LinuxChild {
    fn id(&self) -> u32 {
        self.child.id()
    }

    fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait()
    }

    fn kill_tree(&mut self) {
        // 进程组已被回收时 `kill(-pgid)` 返回 ESRCH，按幂等语义忽略。
        unsafe {
            libc::kill(-self.pgid, libc::SIGKILL);
        }
    }

    fn kill_handle(&self) -> KillHandle {
        let pgid = self.pgid;
        KillHandle::new(move || unsafe {
            libc::kill(-pgid, libc::SIGKILL);
        })
    }

    fn take_stdout(&mut self) -> Option<Box<dyn io::Read + Send>> {
        self.child
            .stdout
            .take()
            .map(|stream| Box::new(stream) as Box<dyn io::Read + Send>)
    }

    fn take_stderr(&mut self) -> Option<Box<dyn io::Read + Send>> {
        self.child
            .stderr
            .take()
            .map(|stream| Box::new(stream) as Box<dyn io::Read + Send>)
    }
}

impl Drop for LinuxChild {
    fn drop(&mut self) {
        self.kill_tree();
        let _ = self.child.wait();
    }
}

/// 查内核 Landlock ABI 版本；`0` 表示内核未启用 Landlock。
fn kernel_abi() -> Result<ABI, SandboxError> {
    // SAFETY: 只读查询，空指针 + 零长度 + VERSION 标志位不会创建任何规则集。
    let version = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<libc::c_void>(),
            0_usize,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    };
    if version < 0 {
        return Err(SandboxError::SetupFailed("landlock_abi_query".to_string()));
    }
    Ok(ABI::from(version as i32))
}

/// ABI 4 起支持网络规则；更低版本把 `network` 记入 `missing`（档位降为 `partial`）。
fn network_supported(abi: ABI) -> bool {
    abi >= ABI::V4
}

/// 建规则集：调用方在 `pre_exec` 里 `restrict_self`，`probe` 里只验证能否建起来。
///
/// `write_roots` 为空时得到「只读」形态（供 `probe()` 使用）；非空时逐根授全量访问权。
fn build_ruleset(
    host: &SandboxHostPaths,
    write_roots: &[PathBuf],
    abi: ABI,
) -> Result<RulesetCreated, SandboxError> {
    let mut ruleset = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))
        .map_err(|_| SandboxError::SetupFailed("ruleset_handle_fs".to_string()))?;
    if network_supported(abi) {
        ruleset = ruleset
            .handle_access(AccessNet::from_all(abi))
            .map_err(|_| SandboxError::SetupFailed("ruleset_handle_net".to_string()))?;
    }
    let mut ruleset = ruleset
        .create()
        .map_err(|_| SandboxError::SetupFailed("ruleset_create".to_string()))?;

    ruleset = add_path_rule(ruleset, Path::new(EXEC_ROOT), AccessFs::Execute)?;
    for root in SYSTEM_READ_ROOTS {
        let path = Path::new(root);
        // 白名单目录若罩住敏感目录，整条跳过（fail-closed：宁可少授权，不可越界授权）。
        if host
            .sensitive_read_deny
            .iter()
            .any(|sensitive| paths_overlap(path, sensitive))
        {
            continue;
        }
        if !path.exists() {
            continue;
        }
        ruleset = add_path_rule(ruleset, path, AccessFs::from_read(abi))?;
    }
    for device in DEVICE_WRITE_FILES {
        let path = Path::new(device);
        if !path.exists() {
            continue;
        }
        ruleset = add_path_rule(ruleset, path, AccessFs::from_file(abi))?;
    }
    for root in write_roots {
        ruleset = add_path_rule(ruleset, root, AccessFs::from_all(abi))?;
    }
    Ok(ruleset)
}

fn add_path_rule<A>(
    ruleset: RulesetCreated,
    path: &Path,
    access: A,
) -> Result<RulesetCreated, SandboxError>
where
    A: Into<BitFlags<AccessFs>>,
{
    let fd =
        PathFd::new(path).map_err(|_| SandboxError::SetupFailed("rule_path_open".to_string()))?;
    ruleset
        .add_rule(PathBeneath::new(fd, access))
        .map_err(|_| SandboxError::SetupFailed("rule_add".to_string()))
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use tempfile::TempDir;

    use super::*;
    use crate::sandbox::SandboxPlan;

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
            write_roots: vec![root.to_path_buf()],
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

    fn assert_sandbox_ready(backend: &LandlockBackend) {
        let probe = backend.probe();
        assert!(
            probe.enforcement != crate::sandbox::SandboxEnforcement::None,
            "Landlock 不可用，跳过：{:?}",
            probe.detail
        );
    }

    #[test]
    fn a_s1_write_boundary() {
        let (temp, root) = workspace();
        let sensitive = temp.path().join("secret");
        std::fs::create_dir_all(&sensitive).expect("secret");
        let backend = LandlockBackend::new(host_with_sensitive(vec![sensitive]));
        assert_sandbox_ready(&backend);

        let outside = PathBuf::from(format!("/tmp/a_s1_outside_{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&outside);
        let child = backend
            .spawn(&SandboxPlan {
                env: vec![
                    ("PATH".to_string(), "/usr/bin:/bin".to_string()),
                    ("OUT".to_string(), outside.display().to_string()),
                ],
                // 越界重定向会被 Landlock 拒绝，最后一条命令因此非零；
                // 追加 `:` 让脚本以 0 退出，退出码断言只校验「沙箱正常收尸」。
                ..plan_for(
                    &root,
                    "echo inside > inside.txt; echo outside > \"$OUT\"; :",
                )
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
        let backend = LandlockBackend::new(host_with_sensitive(vec![sensitive.clone()]));
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
        let backend = LandlockBackend::new(host_with_sensitive(vec![sensitive.clone()]));
        assert_sandbox_ready(&backend);

        let plan = SandboxPlan {
            cwd: sensitive.clone(),
            write_roots: vec![sensitive],
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
        let backend = LandlockBackend::new(host_with_sensitive(vec![sensitive]));
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
            if !Path::new(&format!("/proc/{pid}")).exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("后台进程 {pid} 仍存活，进程组回收失败");
    }

    #[test]
    fn probe_reports_network_missing_below_abi4() {
        let (temp, _root) = workspace();
        let sensitive = temp.path().join("secret");
        std::fs::create_dir_all(&sensitive).expect("secret");
        let backend = LandlockBackend::new(host_with_sensitive(vec![sensitive]));
        let probe = backend.probe();
        if probe.enforcement == crate::sandbox::SandboxEnforcement::None {
            return;
        }
        let abi = kernel_abi().expect("abi");
        if network_supported(abi) {
            assert!(!probe.missing.contains(&SandboxDimension::Network));
        } else {
            assert!(probe.missing.contains(&SandboxDimension::Network));
            assert_eq!(
                probe.enforcement,
                crate::sandbox::SandboxEnforcement::Partial
            );
        }
    }
}
