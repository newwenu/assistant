mod acl;

use std::ffi::c_void;
use std::io::Read;
use std::os::windows::io::FromRawHandle;
use std::os::windows::process::ExitStatusExt;
use std::path::Path;
use std::process::ExitStatus;

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, SetHandleInformation, HANDLE, HANDLE_FLAGS, HANDLE_FLAG_INHERIT,
};
use windows::Win32::Security::Authorization::ConvertStringSidToSidW;
use windows::Win32::Security::{
    CreateRestrictedToken, GetTokenInformation, TokenGroups, DISABLE_MAX_PRIVILEGE, PSID, SID,
    SID_AND_ATTRIBUTES, TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE, TOKEN_GROUPS, TOKEN_QUERY,
    WRITE_RESTRICTED,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicUIRestrictions,
    JobObjectExtendedLimitInformation, SetInformationJobObject, TerminateJobObject,
    JOBOBJECT_BASIC_UI_RESTRICTIONS, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_UILIMIT_DESKTOP,
    JOB_OBJECT_UILIMIT_DISPLAYSETTINGS, JOB_OBJECT_UILIMIT_GLOBALATOMS,
    JOB_OBJECT_UILIMIT_READCLIPBOARD, JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS,
    JOB_OBJECT_UILIMIT_WRITECLIPBOARD,
};
#[cfg(test)]
use windows::Win32::System::JobObjects::{
    JobObjectBasicAccountingInformation, QueryInformationJobObject,
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CreateProcessAsUserW, GetCurrentProcess, GetExitCodeProcess, OpenProcessToken, ResumeThread,
    TerminateProcess, WaitForSingleObject, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT,
    PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOW,
};

use crate::sandbox::{
    SandboxBackend, SandboxChild, SandboxChildOps, SandboxDimension, SandboxError,
    SandboxHostPaths, SandboxKind, SandboxPlan, SandboxProbe,
};

const WAIT_INFINITE: u32 = u32::MAX;

/// 持有型句柄：早退路径自动 `CloseHandle`，成功路径用 `into_raw` 移交所有权。
struct OwnedHandle(HANDLE);

impl OwnedHandle {
    fn new(handle: HANDLE) -> Self {
        Self(handle)
    }

    fn raw(&self) -> HANDLE {
        self.0
    }

    fn into_raw(self) -> HANDLE {
        let handle = self.0;
        std::mem::forget(self);
        handle
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

unsafe impl Send for OwnedHandle {}

fn to_wide(value: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    let mut units: Vec<u16> = value.as_os_str().encode_wide().collect();
    units.push(0);
    units
}

fn wide_str(value: &str) -> Vec<u16> {
    let mut units: Vec<u16> = value.encode_utf16().collect();
    units.push(0);
    units
}

/// 交给子进程的工作目录：去掉 `\\?\` / `\\?\UNC\` 前缀。
///
/// `writable_root` / `cwd` 按规格是 canonicalize 后的路径，落在 NT 的扩展长度形式
/// （`\\?\C:\…`）。`cmd.exe` 会把该前缀当成 UNC 起点，回退到 `Windows` 目录后再执行，
/// 于是相对路径写入全部越界失败。两个形式指向同一目录，剥前缀只影响 cmd 的解析。
/// ACL 侧仍用原路径（`acl::prepare` 不经过这里）。
fn plain_path_wide(value: &Path) -> Vec<u16> {
    let text = value.to_string_lossy();
    let plain = match text.strip_prefix(r"\\?\UNC\") {
        Some(rest) => format!(r"\\{rest}"),
        None => match text.strip_prefix(r"\\?\") {
            Some(rest) => rest.to_string(),
            None => text.into_owned(),
        },
    };
    wide_str(&plain)
}

fn sid_from_string(value: &str) -> Result<PSID, SandboxError> {
    let wide = wide_str(value);
    let mut sid = PSID::default();
    unsafe { ConvertStringSidToSidW(PCWSTR::from_raw(wide.as_ptr()), &mut sid) }
        .map_err(|_| SandboxError::SetupFailed("token_convert_sid".to_string()))?;
    Ok(sid)
}

/// `S-1-5-5-X-Y` 形状的 logon SID：NT 权威、子权威首位为 5。
fn is_logon_sid(sid: PSID) -> bool {
    if sid.0.is_null() {
        return false;
    }
    unsafe {
        let raw = &*(sid.0 as *const SID);
        raw.Revision == 1
            && raw.SubAuthorityCount >= 3
            && raw.IdentifierAuthority.Value[5] == 5
            && raw.SubAuthority[0] == 5
    }
}

/// 从令牌的 `TokenGroups` 里取出 logon SID。
///
/// 返回的缓冲区必须活到 `CreateRestrictedToken` 调用结束——`PSID` 指向它内部。
/// 缓冲区用 `Vec<u64>` 保证 8 字节对齐（`TOKEN_GROUPS` 含指针字段）。
fn logon_sid(token: HANDLE) -> Result<(Vec<u64>, PSID), SandboxError> {
    unsafe {
        let mut size: u32 = 0;
        let _ = GetTokenInformation(token, TokenGroups, None, 0, &mut size);
        if size == 0 {
            return Err(SandboxError::SetupFailed("token_groups_size".to_string()));
        }
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        GetTokenInformation(
            token,
            TokenGroups,
            Some(buffer.as_mut_ptr() as *mut c_void),
            size,
            &mut size,
        )
        .map_err(|_| SandboxError::SetupFailed("token_groups_read".to_string()))?;

        let groups = &*(buffer.as_ptr() as *const TOKEN_GROUPS);
        let base = groups.Groups.as_ptr();
        for index in 0..groups.GroupCount {
            let sid = (*base.add(index as usize)).Sid;
            if is_logon_sid(sid) {
                return Ok((buffer, sid));
            }
        }
        Err(SandboxError::SetupFailed(
            "token_logon_sid_missing".to_string(),
        ))
    }
}

/// 受限令牌：`DISABLE_MAX_PRIVILEGE | WRITE_RESTRICTED` + restricting SID 表。
///
/// `WRITE_RESTRICTED` 是写边界的本体：写类访问会额外用**仅含该表**做一次检查，
/// 因此「只有可写根给合成 SID 授了写」就等价于「可写根之外写不进去」；读、网络、
/// 进程可见性都走第一次检查，不受影响（规格 §5.1）。
///
/// 表必须是 `[logon SID, Everyone, 合成 SID]`：
/// - 缺 `logon SID` / `Everyone` 时子进程在 DLL 初始化阶段以 `0xC0000142` 死亡；
/// - 不用 `SidsToDisable` 把 Everyone / Users / Authenticated Users 置 deny-only：
///   实测该配方下 `cmd.exe` 启动即 `0xC0000022`（deny-only 的 SID 不参与 allow）。
fn create_restricted_token() -> Result<OwnedHandle, SandboxError> {
    unsafe {
        let mut current = HANDLE::default();
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY | TOKEN_QUERY,
            &mut current,
        )
        .map_err(|_| SandboxError::SetupFailed("token_open_process_token".to_string()))?;
        let current = OwnedHandle::new(current);

        let (logon_buffer, logon) = logon_sid(current.raw())?;
        let everyone = sid_from_string("S-1-1-0")?;
        let sandbox = acl::sandbox_sid()?;

        let restrict = [
            SID_AND_ATTRIBUTES {
                Sid: logon,
                Attributes: 0,
            },
            SID_AND_ATTRIBUTES {
                Sid: everyone,
                Attributes: 0,
            },
            SID_AND_ATTRIBUTES {
                Sid: sandbox,
                Attributes: 0,
            },
        ];

        let mut restricted = HANDLE::default();
        let result = CreateRestrictedToken(
            current.raw(),
            DISABLE_MAX_PRIVILEGE | WRITE_RESTRICTED,
            None,
            None,
            Some(&restrict),
            &mut restricted,
        );

        drop(logon_buffer);
        acl::free_sid(sandbox);
        acl::free_sid(everyone);

        result.map_err(|_| SandboxError::SetupFailed("token_create_restricted".to_string()))?;
        Ok(OwnedHandle::new(restricted))
    }
}

/// Job 对象：`KILL_ON_JOB_CLOSE` + UI 限制全开。
///
/// **不设** `ACTIVE_PROCESS` 上限：`cmd /C` 需要派生子进程。
fn create_job() -> Result<OwnedHandle, SandboxError> {
    unsafe {
        let job = CreateJobObjectW(None, PCWSTR::null())
            .map_err(|_| SandboxError::SetupFailed("job_create".to_string()))?;
        let job = OwnedHandle::new(job);

        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job.raw(),
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .map_err(|_| SandboxError::SetupFailed("job_set_limits".to_string()))?;

        let ui = JOBOBJECT_BASIC_UI_RESTRICTIONS {
            UIRestrictionsClass: JOB_OBJECT_UILIMIT_DESKTOP
                | JOB_OBJECT_UILIMIT_DISPLAYSETTINGS
                | JOB_OBJECT_UILIMIT_GLOBALATOMS
                | JOB_OBJECT_UILIMIT_READCLIPBOARD
                | JOB_OBJECT_UILIMIT_WRITECLIPBOARD
                | JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS,
        };
        SetInformationJobObject(
            job.raw(),
            JobObjectBasicUIRestrictions,
            &ui as *const _ as *const c_void,
            std::mem::size_of::<JOBOBJECT_BASIC_UI_RESTRICTIONS>() as u32,
        )
        .map_err(|_| SandboxError::SetupFailed("job_set_ui_limits".to_string()))?;

        Ok(job)
    }
}

/// 一根管道：读端留在父进程，写端在 `ResumeThread` 之前关掉。
struct Pipe {
    read: OwnedHandle,
    write: OwnedHandle,
}

fn create_pipe() -> Result<Pipe, SandboxError> {
    unsafe {
        let mut read = HANDLE::default();
        let mut write = HANDLE::default();
        CreatePipe(&mut read, &mut write, None, 0)
            .map_err(|_| SandboxError::SpawnFailed("pipe_create".to_string()))?;
        // 读端必须不可继承：否则子进程会多持有一份读句柄，句柄计数永远不归零。
        SetHandleInformation(read, 1, HANDLE_FLAGS(0))
            .map_err(|_| SandboxError::SpawnFailed("pipe_set_read".to_string()))?;
        SetHandleInformation(write, 1, HANDLE_FLAG_INHERIT)
            .map_err(|_| SandboxError::SpawnFailed("pipe_set_write".to_string()))?;
        Ok(Pipe {
            read: OwnedHandle::new(read),
            write: OwnedHandle::new(write),
        })
    }
}

fn quote_arg(value: &str) -> String {
    if !value.is_empty() && !value.contains([' ', '\t', '"']) {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    let mut backslashes = 0usize;
    for ch in value.chars() {
        match ch {
            '\\' => backslashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.push_str(&"\\".repeat(backslashes));
                backslashes = 0;
                out.push(ch);
            }
        }
    }
    out.push_str(&"\\".repeat(backslashes * 2));
    out.push('"');
    out
}

/// `/C` / `/K` 之后的脚本：固定 shell 形态下 `plan.args = [flag, command]`。
fn split_cmd_script(args: &[String]) -> Option<(&str, &str)> {
    match args {
        [flag, script] if flag.eq_ignore_ascii_case("/c") || flag.eq_ignore_ascii_case("/k") => {
            Some((flag.as_str(), script.as_str()))
        }
        _ => None,
    }
}

fn is_cmd_program(program: &str) -> bool {
    match Path::new(program).file_name() {
        Some(name) => name.eq_ignore_ascii_case("cmd.exe"),
        None => false,
    }
}

/// 命令行拼装。
///
/// `cmd.exe` 的 `/C` 之后是**一段脚本**而不是参数（I52 §5.7 第 3 步：
/// `args = ["/C", command]`）。CRT 的 `\"` 转义 cmd 不认，`type \"path\"` 会被当成
/// 字面反斜杠 + 未闭合引号（报「文件名、目录名或卷标语法不正确」）。因此固定 shell
/// 形态下脚本原样交给 cmd，引号与元字符由它自己解析——I52 §12 R2 已把
/// 「argv 语义 ≠ 实际执行」列为已知取舍，边界由沙箱承担，不由字符串承担。
fn build_command_line(plan: &SandboxPlan) -> String {
    let program = plan.program.to_string_lossy();
    let mut parts = vec![quote_arg(&program)];
    if is_cmd_program(&program) {
        if let Some((flag, script)) = split_cmd_script(&plan.args) {
            parts.push(flag.to_string());
            parts.push(script.to_string());
            return parts.join(" ");
        }
    }
    parts.extend(plan.args.iter().map(|arg| quote_arg(arg)));
    parts.join(" ")
}

/// Unicode 环境块：`KEY=VALUE\0…\0`，末尾再补一个 `\0`。
///
/// `SystemRoot` / `SystemDrive` 缺失时从父进程补上：它们不敏感，但缺了会让
/// 子进程的加载器找不到系统 DLL（与「禁止透传敏感名」不冲突）。
fn build_env_block(plan: &SandboxPlan) -> Vec<u16> {
    let mut entries: Vec<(String, String)> = plan.env.clone();
    for key in ["SystemRoot", "SystemDrive"] {
        if !entries.iter().any(|(k, _)| k.eq_ignore_ascii_case(key)) {
            if let Ok(value) = std::env::var(key) {
                entries.push((key.to_string(), value));
            }
        }
    }

    let mut block: Vec<u16> = Vec::new();
    for (key, value) in &entries {
        for unit in format!("{key}={value}").encode_utf16() {
            block.push(unit);
        }
        block.push(0);
    }
    block.push(0);
    if entries.is_empty() {
        block.push(0);
    }
    block
}

pub struct WindowsBackend {
    /// Windows 没有读边界（规格 §5.1），`sensitive_read_deny` 在此平台不参与
    /// 探测也不产生 ACL 改动；保留注入形态只为让上层按平台无关方式构造后端。
    _host: SandboxHostPaths,
}

impl WindowsBackend {
    pub fn new(host: SandboxHostPaths) -> Self {
        Self { _host: host }
    }
}

impl SandboxBackend for WindowsBackend {
    fn kind(&self) -> SandboxKind {
        SandboxKind::WindowsRestrictedToken
    }

    /// 三项能力试跑，全部只读：不改 ACL、不建文件、不 spawn 进程（§6「探测只读」）。
    fn probe(&self) -> SandboxProbe {
        if create_restricted_token().is_err() {
            return SandboxProbe::unavailable(
                SandboxKind::WindowsRestrictedToken,
                "probe_create_restricted_token",
            );
        }

        if create_job().is_err() {
            return SandboxProbe::unavailable(
                SandboxKind::WindowsRestrictedToken,
                "probe_create_job_object",
            );
        }

        // 读一次 DACL 证明「可读 + 可写 ACL」这条路径通；只读，不落任何改动。
        let target = std::env::var("TEMP")
            .ok()
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_dir())
            .or_else(|| {
                std::env::var("SystemRoot")
                    .ok()
                    .map(std::path::PathBuf::from)
            });
        let acl_readable = match target {
            Some(path) => acl::read_entries(&path).is_ok(),
            None => false,
        };
        if !acl_readable {
            return SandboxProbe::unavailable(
                SandboxKind::WindowsRestrictedToken,
                "probe_read_named_security_info",
            );
        }

        // Windows 无读边界（机制不可达，§5.1 / §10 OQ1）：`fsRead` 恒记 missing。
        let missing = vec![SandboxDimension::FsRead, SandboxDimension::Network];
        SandboxProbe {
            kind: SandboxKind::WindowsRestrictedToken,
            enforcement: crate::sandbox::enforcement_from_missing(&missing),
            missing,
            detail: None,
        }
    }

    fn spawn(&self, plan: &SandboxPlan) -> Result<SandboxChild, SandboxError> {
        // 硬约束 1：准备失败不 spawn。
        acl::prepare(plan)?;

        let token = create_restricted_token()?;
        let job = create_job()?;
        let stdout_pipe = create_pipe()?;
        let stderr_pipe = create_pipe()?;

        let command_line = build_command_line(plan);
        let mut command_wide = wide_str(&command_line);
        let program_wide = to_wide(&plan.program);
        let cwd_wide = plain_path_wide(&plan.cwd);
        let env_block = build_env_block(plan);

        let startup = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            dwFlags: STARTF_USESTDHANDLES,
            // stdin 恒 None：不建 stdin 管道，子进程侧置空句柄。
            hStdInput: HANDLE::default(),
            hStdOutput: stdout_pipe.write.raw(),
            hStdError: stderr_pipe.write.raw(),
            ..Default::default()
        };

        let mut process_info = PROCESS_INFORMATION::default();
        unsafe {
            CreateProcessAsUserW(
                Some(token.raw()),
                PCWSTR::from_raw(program_wide.as_ptr()),
                Some(PWSTR::from_raw(command_wide.as_mut_ptr())),
                None,
                None,
                true,
                CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT,
                Some(env_block.as_ptr() as *const c_void),
                PCWSTR::from_raw(cwd_wide.as_ptr()),
                &startup,
                &mut process_info,
            )
        }
        .map_err(|_| SandboxError::SpawnFailed("create_process_as_user".to_string()))?;

        let process = OwnedHandle::new(process_info.hProcess);
        let thread = OwnedHandle::new(process_info.hThread);

        // 写端必须在这里关：晚于 ResumeThread 会让读侧等不到 EOF，
        // 进而把 I52 §5.7 第 9 步的 join 拖死。
        drop(stdout_pipe.write);
        drop(stderr_pipe.write);

        // 时序固定：CreateProcess(suspended) → Assign → Resume。
        let assigned = unsafe { AssignProcessToJobObject(job.raw(), process.raw()) };
        if assigned.is_err() {
            unsafe {
                let _ = TerminateProcess(process.raw(), 1);
            }
            return Err(SandboxError::SetupFailed("job_assign".to_string()));
        }

        if unsafe { ResumeThread(thread.raw()) } == 0 {
            unsafe {
                let _ = TerminateProcess(process.raw(), 1);
            }
            return Err(SandboxError::SpawnFailed("resume_thread".to_string()));
        }
        // 主线程已跑起来，线程句柄不再有用，立刻释放。
        drop(thread);

        let stdout = Some(unsafe {
            Box::new(std::fs::File::from_raw_handle(
                stdout_pipe.read.into_raw().0 as *mut _,
            )) as Box<dyn Read + Send>
        });
        let stderr = Some(unsafe {
            Box::new(std::fs::File::from_raw_handle(
                stderr_pipe.read.into_raw().0 as *mut _,
            )) as Box<dyn Read + Send>
        });

        Ok(SandboxChild::new(Box::new(WindowsChild {
            job,
            process,
            pid: process_info.dwProcessId,
            stdout,
            stderr,
            killed: false,
        })))
    }
}

struct WindowsChild {
    job: OwnedHandle,
    process: OwnedHandle,
    pid: u32,
    stdout: Option<Box<dyn Read + Send>>,
    stderr: Option<Box<dyn Read + Send>>,
    killed: bool,
}

unsafe impl Send for WindowsChild {}

impl SandboxChildOps for WindowsChild {
    fn take_stdout(&mut self) -> Option<Box<dyn Read + Send>> {
        self.stdout.take()
    }

    fn take_stderr(&mut self) -> Option<Box<dyn Read + Send>> {
        self.stderr.take()
    }

    fn id(&self) -> u32 {
        self.pid
    }

    fn wait(&mut self) -> std::io::Result<ExitStatus> {
        unsafe {
            WaitForSingleObject(self.process.raw(), WAIT_INFINITE);
            let mut code: u32 = 0;
            GetExitCodeProcess(self.process.raw(), &mut code)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            Ok(ExitStatus::from_raw(code))
        }
    }

    /// 一次杀光 Job 内全部进程；幂等，可重复调用。
    fn kill_tree(&mut self) {
        if self.killed {
            return;
        }
        self.killed = true;
        unsafe {
            let _ = TerminateJobObject(self.job.raw(), 1);
        }
    }

    #[cfg(all(test, windows))]
    fn active_processes(&self) -> Option<u32> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        unsafe {
            QueryInformationJobObject(
                Some(self.job.raw()),
                JobObjectBasicAccountingInformation,
                &mut info as *mut _ as *mut c_void,
                std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                None,
            )
        }
        .ok()?;
        Some(info.ActiveProcesses)
    }
}

impl Drop for WindowsChild {
    fn drop(&mut self) {
        // 兜底：整树回收 + 收尸；三个句柄随后由 `OwnedHandle` 各自关闭
        // （Job 句柄关闭时 `KILL_ON_JOB_CLOSE` 再兜一层）。
        self.kill_tree();
        let _ = self.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::SandboxDimension;
    use std::io::Read;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn cmd_program() -> PathBuf {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:/Windows".to_string());
        PathBuf::from(root).join("System32").join("cmd.exe")
    }

    fn plan_for(root: &Path, script: &str) -> SandboxPlan {
        SandboxPlan {
            program: cmd_program(),
            args: vec!["/C".to_string(), script.to_string()],
            cwd: root.to_path_buf(),
            writable_root: root.to_path_buf(),
            env: Vec::new(),
            network: false,
            stdin: None,
        }
    }

    fn run(backend: &WindowsBackend, plan: &SandboxPlan) -> (i32, String, String) {
        let mut child = backend.spawn(plan).expect("spawn");
        let stdout = child.stdout();
        let reader = std::thread::spawn(move || {
            let mut buffer = Vec::new();
            if let Some(mut handle) = stdout {
                let _ = handle.read_to_end(&mut buffer);
            }
            buffer
        });
        let mut stderr_buffer = Vec::new();
        if let Some(mut handle) = child.stderr() {
            let _ = handle.read_to_end(&mut stderr_buffer);
        }
        let status = child.wait().expect("wait");
        let stdout_buffer = reader.join().expect("stdout reader");
        (
            status.code().unwrap_or(-1),
            String::from_utf8_lossy(&stdout_buffer).into_owned(),
            String::from_utf8_lossy(&stderr_buffer).into_owned(),
        )
    }

    /// 测试可写根 guard：临时目录必须活到语句结束，并在归还时显式回收。
    ///
    /// 沙箱测试让受限令牌子进程把 workspace 当 cwd，进程退出与句柄释放可能滞后；
    /// 直接依赖 `TempDir::drop` 会在删除失败时静默留下 `%TEMP%/.tmpXXXX/workspace`。
    struct Workspace {
        dir: tempfile::TempDir,
        root: PathBuf,
    }

    impl Workspace {
        fn new() -> Self {
            let dir = tempfile::tempdir().expect("tempdir");
            let root = dir.path().join("workspace");
            std::fs::create_dir_all(&root).expect("create workspace");
            Self {
                root: root.canonicalize().expect("canonicalize workspace"),
                dir,
            }
        }

        fn path(&self) -> &Path {
            self.dir.path()
        }

        fn root(&self) -> &Path {
            &self.root
        }

        /// 显式回收并断言成功，避免失败被 `TempDir::drop` 静默吞掉。
        fn close(self) {
            if !remove_all_with_retry(self.dir.path(), Duration::from_secs(5)) {
                panic!("测试临时目录无法回收: {}", self.dir.path().display());
            }
        }
    }

    impl Drop for Workspace {
        fn drop(&mut self) {
            // panic / 断言失败路径的兜底：不 panic，避免 double panic abort。
            remove_all_with_retry(self.dir.path(), Duration::from_secs(5));
        }
    }

    fn remove_all_with_retry(path: &Path, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            match std::fs::remove_dir_all(path) {
                Ok(()) => return true,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return true,
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => return false,
            }
        }
    }

    /// A-S2 的 Windows 口径（§8）：读侧不可强制，`missing` 必须诚实含 `FsRead`。
    #[test]
    fn probe_reports_fs_read_missing() {
        let probe = WindowsBackend::new(SandboxHostPaths::default()).probe();
        assert_eq!(probe.kind, SandboxKind::WindowsRestrictedToken);
        assert_eq!(
            probe.missing,
            vec![SandboxDimension::FsRead, SandboxDimension::Network]
        );
        assert_eq!(probe.enforcement.as_str(), "partial");
        assert_eq!(probe.detail, None);
    }

    /// C2.3：退出码透传 + 写端已关（`read_to_end` 能到 EOF 且 `wait` 不挂）。
    #[test]
    fn exit_code_and_stdout_eof() {
        let ws = Workspace::new();
        let backend = WindowsBackend::new(SandboxHostPaths::default());
        let (code, out, _) = run(&backend, &plan_for(ws.root(), "echo hello & exit 7"));
        assert_eq!(code, 7, "退出码必须透传");
        assert!(out.contains("hello"), "stdout 应可读到内容，实际: {out:?}");
        ws.close();
    }

    /// A-S1：可写根内可写 / 可删，`%USERPROFILE%` 下不可写。
    #[test]
    fn a_s1_write_boundary() {
        let ws = Workspace::new();
        let backend = WindowsBackend::new(SandboxHostPaths::default());

        let (code, _, _) = run(&backend, &plan_for(ws.root(), "echo inside> inside.txt"));
        assert_eq!(code, 0, "可写根内写入应成功");
        assert!(ws.root().join("inside.txt").exists());

        // 授权掩码含 DELETE：根内删除是工具的正常操作，必须不被 pass-2 拦掉。
        let (code, _, err) = run(&backend, &plan_for(ws.root(), "del inside.txt"));
        assert_eq!(code, 0, "可写根内删除应成功，stderr: {err}");
        assert!(!ws.root().join("inside.txt").exists());

        let profile = PathBuf::from(std::env::var("USERPROFILE").expect("USERPROFILE"));
        let target = profile.join(format!("sandbox_probe_{}.tmp", std::process::id()));
        let script = format!("echo outside> \"{}\"", target.display());
        let (code, _, err) = run(&backend, &plan_for(ws.root(), &script));
        assert!(!target.exists(), "写边界失效：用户主目录下写出了文件");
        assert_ne!(code, 0, "越界写入必须失败，stderr: {err}");
        ws.close();
    }

    /// Windows 没有读边界（§5.1）：敏感目录的读取**必须仍然成功**。
    ///
    /// 这条断言同时是防回归护栏：任何「给敏感目录加 deny ACE」的实现都会让它失败，
    /// 因为那类 ACE 对合成 SID 零作用、落在用户自身 SID 上又会打掉宿主自己的访问。
    #[test]
    fn read_side_is_not_confined_on_windows() {
        let ws = Workspace::new();
        let secret_dir = ws.path().join("secret");
        std::fs::create_dir_all(&secret_dir).expect("create secret");
        std::fs::write(secret_dir.join("secret.txt"), "top-secret").expect("write secret");

        let backend = WindowsBackend::new(SandboxHostPaths {
            sensitive_read_deny: vec![secret_dir.clone()],
        });
        let script = format!("type \"{}\"", secret_dir.join("secret.txt").display());
        let (code, out, err) = run(&backend, &plan_for(ws.root(), &script));
        assert_eq!(code, 0, "Windows 上读侧不受限，stderr: {err}");
        assert!(out.contains("top-secret"), "读侧应可读，实际: {out:?}");
        ws.close();
    }

    /// A-S3：派生后台进程后 `kill_tree()`，Job 内进程计数归 0。
    #[test]
    fn a_s3_process_tree_reclaimed() {
        let ws = Workspace::new();
        let backend = WindowsBackend::new(SandboxHostPaths::default());

        let mut child = backend
            .spawn(&plan_for(
                ws.root(),
                "start /b %SystemRoot%\\System32\\ping.exe -n 30 127.0.0.1 >nul & exit 0",
            ))
            .expect("spawn");
        let _ = child.wait().expect("wait cmd");

        let alive_before = child.active_processes().expect("job accounting");
        assert!(alive_before >= 1, "派生进程应仍在 Job 内");

        child.kill_tree();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut alive = alive_before;
        while Instant::now() < deadline {
            alive = child.active_processes().expect("job accounting");
            if alive == 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(alive, 0, "kill_tree 后 Job 内不得残留进程");
        ws.close();
    }

    /// A-S5：准备失败不降级、不产生子进程。
    #[test]
    fn a_s5_setup_failure_does_not_spawn() {
        let ws = Workspace::new();
        let missing_root = ws.path().join("no-such-root");
        let backend = WindowsBackend::new(SandboxHostPaths::default());
        let plan = plan_for(&missing_root, "echo never> never.txt");
        let error = backend
            .spawn(&plan)
            .expect_err("不存在的可写根必须建立失败");
        assert!(
            matches!(error, SandboxError::SetupFailed(_)),
            "应为 SetupFailed: {error}"
        );
        assert!(!missing_root.exists(), "失败路径不得创建可写根");
        ws.close();
    }

    /// A-S7：残留 ACE 下重复执行命令不失败。
    #[test]
    fn a_s7_rerun_with_residual_ace() {
        let ws = Workspace::new();
        let backend = WindowsBackend::new(SandboxHostPaths::default());

        for round in 0..3 {
            let script = format!("echo round{round}> round{round}.txt");
            let (code, _, err) = run(&backend, &plan_for(ws.root(), &script));
            assert_eq!(code, 0, "第 {round} 轮失败: {err}");
        }
        for round in 0..3 {
            assert!(ws.root().join(format!("round{round}.txt")).exists());
        }
        ws.close();
    }
}
