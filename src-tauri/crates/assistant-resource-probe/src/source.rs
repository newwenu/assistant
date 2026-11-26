//! 快照来源抽象（I76 §2.2、§6.4）。
//!
//! 平台后端只做一件事：一轮全系统枚举，产出 `RawSnapshot`。角色判定、建树、CPU 差分、
//! 合计都在纯逻辑层完成，因而可在任意平台用假快照跑测试。

/// 快照里的一行进程。字段与 I76 §2.2 的一次枚举集合一一对应，不含任何平台调用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawProcess {
    pub pid: u32,
    /// 取不到父 PID 时为 `0`（不猜测、不填自身）。
    pub parent_pid: u32,
    /// 进程创建时刻（秒级 epoch，I76 §2.2）：建父子链与 PID 复用校验的必需字段。
    pub start_time: i64,
    /// 镜像名取基础枚举的 `PROCESSENTRY32W.szExeFile` 等值（I76 §2.3 第 ② 层）。
    pub exe_name: String,
    /// 命令行；读不到为空（空即「不可读」，不伪造 `--type=`）。
    pub cmd: Vec<String>,
    /// 平台内存口径；读不到为 `None`，禁止填 `0`（I76 §2.4）。
    pub memory_bytes: Option<u64>,
    /// 累计 CPU 时间（毫秒），差分缓存的输入（I76 §2.5）。
    pub cpu_time_ms: Option<u64>,
    /// 该行是线程还是进程（I76 §2.2 约束 1）；线程不进树。
    pub is_thread: bool,
}

impl RawProcess {
    pub fn new(pid: u32, parent_pid: u32, start_time: i64, exe_name: &str) -> Self {
        Self {
            pid,
            parent_pid,
            start_time,
            exe_name: exe_name.to_string(),
            cmd: Vec::new(),
            memory_bytes: None,
            cpu_time_ms: None,
            is_thread: false,
        }
    }

    pub fn with_cmd(mut self, cmd: &[&str]) -> Self {
        self.cmd = cmd.iter().map(|arg| arg.to_string()).collect();
        self
    }

    pub fn with_memory(mut self, bytes: u64) -> Self {
        self.memory_bytes = Some(bytes);
        self
    }

    pub fn with_cpu_time(mut self, ms: u64) -> Self {
        self.cpu_time_ms = Some(ms);
        self
    }

    /// 线程条目：与进程条目同形，仅 `is_thread` 不同，用于验证「线程不进树」（I76 §2.2 约束 1）。
    pub fn thread(pid: u32, parent_pid: u32, start_time: i64, name: &str) -> Self {
        let mut process = Self::new(pid, parent_pid, start_time, name);
        process.is_thread = true;
        process
    }
}

/// 一次枚举的原始结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSnapshot {
    /// 本次枚举的时刻（epoch 毫秒），整拍共用一个值（I76 §2.2 约束 2）。
    pub sampled_at: i64,
    pub processes: Vec<RawProcess>,
    /// 平台能直接给出宿主线程数时填此字段（Linux `tasks()` 长度、macOS `pti_threadnum`）；
    /// Windows 无此能力，填 `None` 后由建树层按被过滤的线程条目计数兜底（I76 §5.3）。
    pub host_thread_count: Option<u32>,
}

/// 一次快照的来源。
pub trait SnapshotSource: Send {
    /// 每拍只允许调用一次：实现必须用一轮全系统枚举取全所有字段，
    /// 禁止按字段或按子树分次枚举（I76 §2.2 约束 1、约束 2）。
    fn snapshot(&mut self) -> Result<RawSnapshot, String>;

    /// 宿主线程数（I76 §5.3）。默认 `None`：由建树层按被过滤的线程条目计数兜底。
    ///
    /// 只有平台能在**定向**调用里取到宿主线程数时才覆写此方法（macOS 的
    /// `proc_pidinfo(PROC_PIDTASKINFO)`）：这是对单个 PID 的定点读取，不构成
    /// 第二次全系统枚举（I76 §2.2 约束 1 的判据是「每拍总枚举次数为 1」）。
    fn host_thread_count(&self, root_pid: u32) -> Option<u32> {
        let _ = root_pid;
        None
    }
}

/// 固定的失败来源：用于命令层把「采样任务异常」并入同一条 `unreadable` 降级路径，
/// 避免在命令层复制一份快照构造逻辑。
#[derive(Debug, Clone)]
pub struct FailingSource {
    reason: String,
}

impl FailingSource {
    pub fn new(reason: &str) -> Self {
        Self {
            reason: reason.to_string(),
        }
    }
}

impl SnapshotSource for FailingSource {
    fn snapshot(&mut self) -> Result<RawSnapshot, String> {
        Err(self.reason.clone())
    }
}
