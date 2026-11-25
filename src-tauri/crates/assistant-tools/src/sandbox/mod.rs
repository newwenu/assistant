use std::fmt;
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::WindowsBackend;

/// `program` 的 basename 判定（`cmd.exe`，大小写不敏感）：`agent_shell/spawn.rs`
/// 校验 `COMSPEC` 时复用，不写第二份判定（I56 §7.3 约束 2）。
#[cfg(windows)]
pub(crate) use windows::is_cmd_program;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::LandlockBackend;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::SeatbeltBackend;

/// 后端标识。`None` 表示当前平台没有可用后端（不注册 `shell`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxKind {
    WindowsRestrictedToken,
    Landlock,
    Seatbelt,
    None,
}

impl SandboxKind {
    /// 稳定标识串：只用于日志与审批卡展示，**禁止**写入路径或命令内容。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WindowsRestrictedToken => "windows_restricted_token",
            Self::Landlock => "landlock",
            Self::Seatbelt => "seatbelt",
            Self::None => "none",
        }
    }
}

/// 隔离强度档位。判据固定为维度集合（`missing`），不由后端自报。
///
/// `Serialize` / `Deserialize` 只服务审批键 JSON（`ApprovalScope::Command.enforcement`），
/// 与 `as_str()` 同形（`full` / `partial` / `none`）；不进 wire DTO。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxEnforcement {
    /// `missing` 为空：四项维度全部系统强制。
    Full,
    /// 写边界与进程树均被强制，但至少缺 `FsRead` / `Network`。
    Partial,
    /// 不存在写边界。
    None,
}

impl SandboxEnforcement {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Partial => "partial",
            Self::None => "none",
        }
    }
}

/// 隔离维度。`missing` 里出现谁，就表示谁没有被系统强制。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxDimension {
    FsWrite,
    FsRead,
    ProcessTree,
    Network,
}

/// `enforcement` 的唯一推导入口（规格 §4.5）：各后端只填 `missing`，不自报档位。
///
/// `FsWrite` 缺失即「没有写边界」→ `None`；否则 `missing` 非空即 `Partial`，为空即 `Full`。
pub fn enforcement_from_missing(missing: &[SandboxDimension]) -> SandboxEnforcement {
    if missing.contains(&SandboxDimension::FsWrite) {
        SandboxEnforcement::None
    } else if missing.is_empty() {
        SandboxEnforcement::Full
    } else {
        SandboxEnforcement::Partial
    }
}

/// 探测结论：注册期算一次，运行期只读。
///
/// 只承载可展示的结论，**禁止**塞入 `PathBuf`、用户名或命令内容；
/// `detail` 是固定串（后端名 / 系统 API 名 / 失败类别）。
#[derive(Debug, Clone)]
pub struct SandboxProbe {
    pub kind: SandboxKind,
    pub enforcement: SandboxEnforcement,
    pub missing: Vec<SandboxDimension>,
    pub detail: Option<String>,
}

impl SandboxProbe {
    /// 探测失败 / 平台不支持的统一结论：`FsWrite` 记入 `missing`，由共用判据推导出 `None`。
    pub fn unavailable(kind: SandboxKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            enforcement: SandboxEnforcement::None,
            missing: vec![SandboxDimension::FsWrite],
            detail: Some(detail.into()),
        }
    }
}

/// 一次 `spawn` 的完整输入。
///
/// `write_roots` 是按会话解析的结果（每次 `spawn` 现传；工作区根 + 会话临时区根，I73 §3.8），
/// `program` / `args` / `cwd` / `network` **禁止**由工具输入直接指定（规格 §4.3）。
#[derive(Debug, Clone)]
pub struct SandboxPlan {
    /// 绝对路径，不经 `PATH` 解析。
    pub program: PathBuf,
    pub args: Vec<String>,
    /// 已 canonicalize，且落在 `write_roots` 之一内。
    pub cwd: PathBuf,
    /// 可写根集合（每项都已 canonicalize）：工作区根与会话临时区根并列，
    /// 逐项授权、逐项做敏感重叠检查；`cwd` 归属判定为「在任一根内」。
    pub write_roots: Vec<PathBuf>,
    pub env: Vec<(String, String)>,
    /// P14 恒 `false`。
    pub network: bool,
    /// P14 恒 `None`。
    pub stdin: Option<Vec<u8>>,
}

/// 沙箱侧的建立 / 创建 / 限制失败。
#[derive(Debug, Clone)]
pub enum SandboxError {
    /// 建立边界失败（ACL / ruleset / profile 生成）：不 spawn，不降级。
    SetupFailed(String),
    /// 进程无法创建。
    SpawnFailed(String),
    /// 子进程侧限制失败（Unix `pre_exec` 内）。
    RestrictFailed(String),
}

impl SandboxError {
    pub fn detail(&self) -> &str {
        match self {
            Self::SetupFailed(d) | Self::SpawnFailed(d) | Self::RestrictFailed(d) => d,
        }
    }
}

impl fmt::Display for SandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::SetupFailed(_) => "setup failed",
            Self::SpawnFailed(_) => "spawn failed",
            Self::RestrictFailed(_) => "restrict failed",
        };
        write!(f, "sandbox {kind}: {}", self.detail())
    }
}

impl std::error::Error for SandboxError {}

/// 由上层（`tool_setup`）注入的主机路径。
///
/// `assistant-tools` 不依赖 tauri / `app_dirs`，因此敏感目录列表必须注入
/// （规格 §4.4 / A-S6）。`write_roots` **不在其中**：它按会话解析、每次 `spawn` 现传。
#[derive(Debug, Clone, Default)]
pub struct SandboxHostPaths {
    /// 固定敏感目录（§5.4）：沙箱层的读拒绝只针对这份列表。
    pub sensitive_read_deny: Vec<PathBuf>,
}

/// 平台后端统一接口。
pub trait SandboxBackend: Send + Sync {
    fn kind(&self) -> SandboxKind;

    /// 探测当前系统能力，结论只决定「是否注册 `shell`」。
    ///
    /// 探测必须**不产生业务副作用**：不改 ACL、不落文件、不创建业务子进程。
    /// 例外是 macOS：Seatbelt profile 是否被内核接受只能靠一次受限试跑判定
    /// （`sandbox-exec` 在 `sandbox_init` 拒绝 profile 时非 0 退出），
    /// 因此该平台允许在探测期拉起一个 `true` 并在 profile 内立即退出。
    fn probe(&self) -> SandboxProbe;

    /// 在边界内创建并接管一个进程。
    fn spawn(&self, plan: &SandboxPlan) -> Result<SandboxChild, SandboxError>;
}

/// 子进程句柄：生命周期即「回收 + 收尸」的边界。
///
/// 对外只暴露四个方法；平台差异全部收在 `SandboxChildOps` 后面，
/// 避免把 Windows 裸句柄与 Unix `std::process::Child` 的形态泄漏到调用方。
pub struct SandboxChild {
    inner: Box<dyn SandboxChildOps>,
}

impl SandboxChild {
    pub(crate) fn new(inner: Box<dyn SandboxChildOps>) -> Self {
        Self { inner }
    }

    /// 交出 stdout 读端；调用方接管后自行在 `spawn_blocking` 内读取。
    pub fn stdout(&mut self) -> Option<Box<dyn Read + Send>> {
        self.inner.take_stdout()
    }

    pub fn stderr(&mut self) -> Option<Box<dyn Read + Send>> {
        self.inner.take_stderr()
    }

    pub fn id(&self) -> u32 {
        self.inner.id()
    }

    /// 同步阻塞到退出；调用方必须置于 `spawn_blocking`（I52 §5.7）。
    ///
    /// 不声明为 `async`：Windows 侧进程由裸 `CreateProcess` 创建，无法转成 tokio `Child`。
    pub fn wait(&mut self) -> std::io::Result<ExitStatus> {
        self.inner.wait()
    }

    /// 回收**整棵**进程树；可重复调用，幂等。
    pub fn kill_tree(&mut self) {
        self.inner.kill_tree();
    }

    /// 独立杀树句柄：`wait()` 独占 `&mut self` 时，等待方仍能在超时 / 取消路径杀树。
    ///
    /// 与 [`SandboxChild::kill_tree`] 是同一底层调用（Windows `TerminateJobObject`、
    /// Unix `kill(-pgid)`），只把句柄（Job handle / pgid）复制进闭包，不改变沙箱行为。
    /// `agent_shell/run.rs` 把 `wait()` 移进 `spawn_blocking` 后，靠它打断等待。
    pub fn kill_handle(&self) -> KillHandle {
        self.inner.kill_handle()
    }

    /// Job 内仍存活的进程数；只有 Windows 后端能给出（验收 A-S3 用）。
    #[cfg(all(test, windows))]
    pub(crate) fn active_processes(&self) -> Option<u32> {
        self.inner.active_processes()
    }
}

impl fmt::Debug for SandboxChild {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SandboxChild")
            .field("id", &self.id())
            .finish()
    }
}

/// 整树回收句柄：可克隆、可在任意线程调用，幂等（与 `kill_tree` 同一底层调用）。
#[derive(Clone)]
pub struct KillHandle {
    kill: Arc<dyn Fn() + Send + Sync>,
}

impl KillHandle {
    pub(crate) fn new(kill: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            kill: Arc::new(kill),
        }
    }

    /// 杀整棵树；重复调用无副作用。
    pub fn kill(&self) {
        (self.kill)();
    }
}

/// 平台实现面：只给 `SandboxChild` 转发用，不对外暴露。
pub(crate) trait SandboxChildOps: Send {
    fn take_stdout(&mut self) -> Option<Box<dyn Read + Send>>;
    fn take_stderr(&mut self) -> Option<Box<dyn Read + Send>>;
    fn id(&self) -> u32;
    fn wait(&mut self) -> std::io::Result<ExitStatus>;
    fn kill_tree(&mut self);
    fn kill_handle(&self) -> KillHandle;

    /// 默认无此能力；只有 Windows 后端覆写（验收 A-S3）。
    #[cfg(all(test, windows))]
    fn active_processes(&self) -> Option<u32> {
        None
    }
}

/// 注册期探测入口（规格 §4.4）。
///
/// 结论在进程内只算一次、之后只读缓存（A-S8）：运行期不得再调 `probe()`，
/// 否则同一会话前后会出现两套边界。返回 `enforcement == None` 时调用方
/// **不得**注册 `shell`——这里不做降级，也不提供无沙箱执行路径。
pub fn detect(host: &SandboxHostPaths) -> &'static SandboxProbe {
    // 运行期 `spawn` 需要同一份 host 才能重建后端；注册期一次性写入，之后只读。
    let _ = RUNTIME_HOST.set(host.clone());
    let candidates = platform_candidates(host);
    let refs: Vec<&dyn SandboxBackend> = candidates.iter().map(|b| b.as_ref()).collect();
    DETECTED.get(&refs)
}

/// 运行期 host 缓存：`detect` 写入，`spawn` 只读。
static RUNTIME_HOST: OnceLock<SandboxHostPaths> = OnceLock::new();

/// 运行期入口：用注册期同一候选链创建子进程（`agent_shell/run.rs` 的唯一调用点）。
///
/// 不重新 `probe()`：探测只做一次、结论只由 `detect` 产出（A-S8）。P14 每平台只有一个
/// 候选，因此「首个候选」即注册期的胜出者；候选列表为空（平台无后端）时 fail-closed。
/// `detect` 尚未执行（未初始化 host）同样 fail-closed，不存在「无沙箱执行」路径。
pub fn spawn(plan: &SandboxPlan) -> Result<SandboxChild, SandboxError> {
    let host = RUNTIME_HOST
        .get()
        .ok_or_else(|| SandboxError::SetupFailed("sandbox_host_uninitialized".to_string()))?;
    let candidates = platform_candidates(host);
    match candidates.first() {
        Some(backend) => backend.spawn(plan),
        None => Err(SandboxError::SetupFailed("no_backend".to_string())),
    }
}

/// 探测结论缓存。`get` 的探测闭包只在首次调用时执行。
struct ProbeCache {
    cell: OnceLock<SandboxProbe>,
}

impl ProbeCache {
    const fn new() -> Self {
        Self {
            cell: OnceLock::new(),
        }
    }

    fn get<'a>(&'a self, candidates: &[&dyn SandboxBackend]) -> &'a SandboxProbe {
        self.cell.get_or_init(|| pick(candidates))
    }
}

static DETECTED: ProbeCache = ProbeCache::new();

/// 每平台一个候选（P14），按强度降序；S1 才追加降级候选（如 Linux bwrap）。
fn platform_candidates(host: &SandboxHostPaths) -> Vec<Box<dyn SandboxBackend>> {
    #[cfg(windows)]
    {
        vec![Box::new(WindowsBackend::new(host.clone()))]
    }
    #[cfg(target_os = "linux")]
    {
        vec![Box::new(LandlockBackend::new(host.clone()))]
    }
    #[cfg(target_os = "macos")]
    {
        vec![Box::new(SeatbeltBackend::new(host.clone()))]
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        // 其它 Unix / 未知平台无后端：空列表即无候选，由 `pick` 归一为 `None`（fail-closed）。
        let _ = host;
        Vec::new()
    }
}

/// 第一个 `enforcement != None` 的候选胜出；全败时保留首个候选的 `detail`（诊断用）。
///
/// 候选列表为空（Unix 后端尚未落地）时结论为 `kind = None`：此时本平台确实
/// 「没有可用后端」，不得用候选的类型标识冒充（`SandboxKind::None` 的语义即此）。
fn pick(candidates: &[&dyn SandboxBackend]) -> SandboxProbe {
    let mut rejected: Option<SandboxProbe> = None;
    for backend in candidates {
        let probe = backend.probe();
        if probe.enforcement != SandboxEnforcement::None {
            return probe;
        }
        if rejected.is_none() {
            rejected = Some(probe);
        }
    }
    rejected.unwrap_or_else(|| SandboxProbe::unavailable(SandboxKind::None, "no_backend"))
}

/// 沙箱规则层的路径相交判定：任一方向包含即视为相交。
///
/// Unix 两个后端共用同一判据：可写根一旦罩住敏感目录就无解（授写权必然连带读权），
/// 只能拒绝启动；判据留在契约层，避免两个平台各写一份而漂移。
/// Windows 不走该判据（写边界由 ACL 在可写根上单独授予），故只在 Unix 编译。
#[cfg(unix)]
pub(crate) fn paths_overlap(left: &std::path::Path, right: &std::path::Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

/// Unix 可执行位检查：`program` 必须是普通文件且带执行位。
#[cfg(unix)]
pub(crate) fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(path) {
        Ok(metadata) => metadata.is_file() && metadata.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// 探测计数桩：只用于断言「探测次数」，`spawn` 一律拒绝（A-S8）。
    struct CountingBackend {
        probe: SandboxProbe,
        calls: AtomicUsize,
    }

    impl CountingBackend {
        fn new(probe: SandboxProbe) -> Self {
            Self {
                probe,
                calls: AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl SandboxBackend for CountingBackend {
        fn kind(&self) -> SandboxKind {
            self.probe.kind
        }

        fn probe(&self) -> SandboxProbe {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.probe.clone()
        }

        fn spawn(&self, _plan: &SandboxPlan) -> Result<SandboxChild, SandboxError> {
            Err(SandboxError::SetupFailed("stub".to_string()))
        }
    }

    fn probe_with(missing: &[SandboxDimension], detail: &str) -> SandboxProbe {
        SandboxProbe {
            kind: SandboxKind::Landlock,
            enforcement: enforcement_from_missing(missing),
            missing: missing.to_vec(),
            detail: Some(detail.to_string()),
        }
    }

    #[test]
    fn enforcement_follows_missing() {
        assert_eq!(enforcement_from_missing(&[]), SandboxEnforcement::Full);
        assert_eq!(
            enforcement_from_missing(&[SandboxDimension::Network]),
            SandboxEnforcement::Partial
        );
        assert_eq!(
            enforcement_from_missing(&[SandboxDimension::FsRead]),
            SandboxEnforcement::Partial
        );
        assert_eq!(
            enforcement_from_missing(&[SandboxDimension::FsWrite]),
            SandboxEnforcement::None
        );
        assert_eq!(
            enforcement_from_missing(&[
                SandboxDimension::FsWrite,
                SandboxDimension::FsRead,
                SandboxDimension::ProcessTree,
                SandboxDimension::Network,
            ]),
            SandboxEnforcement::None
        );
    }

    #[test]
    fn unavailable_probe_derives_none() {
        let probe =
            SandboxProbe::unavailable(SandboxKind::WindowsRestrictedToken, "no_write_boundary");
        assert_eq!(probe.enforcement, SandboxEnforcement::None);
        assert_eq!(enforcement_from_missing(&probe.missing), probe.enforcement);
        assert_eq!(probe.detail.as_deref(), Some("no_write_boundary"));
    }

    /// A-S8：注册期只探测一次，之后只读缓存。
    #[test]
    fn cache_probes_only_once() {
        let cache = ProbeCache::new();
        let backend = CountingBackend::new(probe_with(&[SandboxDimension::FsRead], "one"));
        let candidates: [&dyn SandboxBackend; 1] = [&backend];

        let first = cache.get(&candidates);
        let second = cache.get(&candidates);
        assert_eq!(backend.calls(), 1, "第二次调用不得再探测");
        assert!(std::ptr::eq(first, second), "两次读到的必须是同一份结论");
        assert_eq!(first.enforcement, SandboxEnforcement::Partial);
    }

    #[test]
    fn pick_takes_first_enforcing_candidate_and_stops() {
        let rejected = CountingBackend::new(probe_with(
            &[SandboxDimension::FsWrite],
            "no_write_boundary",
        ));
        let winner = CountingBackend::new(probe_with(&[SandboxDimension::FsRead], "missing_read"));
        let tail = CountingBackend::new(probe_with(&[], "unused"));
        let candidates: [&dyn SandboxBackend; 3] = [&rejected, &winner, &tail];

        let probe = pick(&candidates);
        assert_eq!(probe.detail.as_deref(), Some("missing_read"));
        assert_eq!(rejected.calls(), 1);
        assert_eq!(winner.calls(), 1);
        assert_eq!(tail.calls(), 0, "胜出后不再探测后续候选");
    }

    #[test]
    fn pick_reports_none_when_every_candidate_rejects() {
        let first = CountingBackend::new(probe_with(&[SandboxDimension::FsWrite], "first"));
        let second = CountingBackend::new(probe_with(&[SandboxDimension::FsWrite], "second"));
        let candidates: [&dyn SandboxBackend; 2] = [&first, &second];

        let probe = pick(&candidates);
        assert_eq!(probe.enforcement, SandboxEnforcement::None);
        assert_eq!(
            probe.detail.as_deref(),
            Some("first"),
            "保留首个结论的 detail"
        );
        assert_eq!(first.calls(), 1);
        assert_eq!(second.calls(), 1, "无胜出者时全部候选都要探完");
    }

    /// 候选列表为空（Unix 后端尚未落地）：结论必须是 `kind = None`，不得冒充候选类型。
    #[test]
    fn pick_without_candidates_is_kind_none() {
        let probe = pick(&[]);
        assert_eq!(probe.kind, SandboxKind::None);
        assert_eq!(probe.enforcement, SandboxEnforcement::None);
        assert_eq!(probe.detail.as_deref(), Some("no_backend"));
    }
}
