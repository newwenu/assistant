use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn timestamp_str() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    chrono::DateTime::from_timestamp_millis(millis as i64)
        .map(|dt| dt.format("%Y%m%d_%H%M%S_%3f").to_string())
        .unwrap_or_else(|| format!("{}", millis))
}

/// 诊断产物目录的观测快照（I61 §3.10）：条目与账本共用同一口径。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DirStats {
    pub bytes: u64,
    pub count: u64,
    /// 文件 `mtime` 最早 / 最晚时刻（秒）；无文件时为 `None`，不是 `0`。
    pub oldest_at: Option<i64>,
    pub newest_at: Option<i64>,
}

/// 只读扫描诊断产物目录：只认三类诊断产物，跳过符号链接与非常规文件（不降级）。
pub fn dump_stats(dir: &Path) -> std::io::Result<DirStats> {
    let mut stats = DirStats::default();
    walk_stats(dir, &mut stats)?;
    Ok(stats)
}

/// 诊断产物文件名判据（I61 §3.10 E13）：`panic_*.log` / `crash_*.dmp` / `crash_*.log`。
///
/// 目录里其它文件（用户手放、历史遗留）既不计数也不使条目降级。
fn is_crash_artifact(name: &str) -> bool {
    let matches = |prefix: &str, suffix: &str| {
        name.strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(suffix))
            .is_some()
    };
    matches("panic_", ".log") || matches("crash_", ".dmp") || matches("crash_", ".log")
}

fn walk_stats(dir: &Path, stats: &mut DirStats) -> std::io::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(traversal_error(dir, error)),
    };
    for entry in entries {
        let entry = entry.map_err(|error| traversal_error(dir, error))?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|error| traversal_error(&path, error))?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            walk_stats(&path, stats)?;
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !is_crash_artifact(name) {
            continue;
        }
        stats.bytes = stats.bytes.saturating_add(meta.len());
        stats.count = stats.count.saturating_add(1);
        if let Ok(modified) = meta.modified() {
            if let Ok(since_epoch) = modified.duration_since(UNIX_EPOCH) {
                let seconds = i64::try_from(since_epoch.as_secs()).unwrap_or(i64::MAX);
                stats.oldest_at = Some(stats.oldest_at.map_or(seconds, |cur| cur.min(seconds)));
                stats.newest_at = Some(stats.newest_at.map_or(seconds, |cur| cur.max(seconds)));
            }
        }
    }
    Ok(())
}

fn traversal_error(path: &Path, error: std::io::Error) -> std::io::Error {
    std::io::Error::new(
        error.kind(),
        format!("scan '{}' failed: {}", path.display(), error),
    )
}

/// 目录由调用方（应用层）解析后传入：本 crate 不算基根、不读 `dirs`。
///
/// 准备失败时退化为 `None`（仅 stderr），不回落当前目录、不写 cwd。
fn prepare_dir(crash_dir: Option<PathBuf>) -> Option<PathBuf> {
    crash_dir.filter(|dir| match fs::create_dir_all(dir) {
        Ok(()) => true,
        Err(e) => {
            eprintln!(
                "crash dump disabled: cannot create {}: {}",
                dir.display(),
                e
            );
            false
        }
    })
}

pub fn install(crash_dir: Option<PathBuf>) {
    let dir = prepare_dir(crash_dir);

    install_panic_hook(dir.clone());
    if let Some(dir) = dir {
        install_crash_handler(&dir);
    }
}

fn install_panic_hook(dir: Option<PathBuf>) {
    std::panic::set_hook(Box::new(move |info| {
        let ts = timestamp_str();

        let mut msg = String::new();
        msg.push_str(&format!("=== PANIC at {} ===\n", ts));
        msg.push_str(&format!("payload: {}\n", info));

        if let Some(loc) = info.location() {
            msg.push_str(&format!(
                "location: {}:{}:{}\n",
                loc.file(),
                loc.line(),
                loc.column()
            ));
        }

        msg.push_str(&format!(
            "\nbacktrace:\n{}",
            std::backtrace::Backtrace::capture()
        ));

        if let Some(dir) = dir.as_ref() {
            let log_path = dir.join(format!("panic_{}.log", ts));
            if let Err(e) = fs::write(&log_path, &msg) {
                eprintln!("panic log write failed: {e}");
            }
        }
        eprintln!("{}", msg);
    }));
}

fn install_crash_handler(crash_dir: &Path) {
    let dir = crash_dir.to_path_buf();
    let handler = crash_handler::CrashHandler::attach(unsafe {
        crash_handler::make_crash_event(move |context: &crash_handler::CrashContext| {
            let ts = timestamp_str();
            let dump_path = dir.join(format!("crash_{}.dmp", ts));
            let log_path = dir.join(format!("crash_{}.log", ts));

            if let Err(e) = fs::write(&log_path, crash_report(context, &ts, &dump_path)) {
                eprintln!("crash log write failed: {e}");
            }

            match fs::File::create(&dump_path) {
                Ok(mut file) => write_dump(context, &mut file, &dump_path),
                Err(e) => eprintln!("failed to create dump file: {}", e),
            }

            crash_handler::CrashEventResult::Handled(true)
        })
    });

    if let Ok(h) = handler {
        std::mem::forget(h);
    }
}

fn crash_report(context: &crash_handler::CrashContext, ts: &str, dump_path: &Path) -> String {
    let mut info = format!("=== CRASH at {} ===\n", ts);
    info.push_str(&crash_fields(context));
    info.push_str(&format!("crash_dump: {}\n", dump_path.display()));
    info
}

#[cfg(target_os = "windows")]
fn crash_fields(context: &crash_handler::CrashContext) -> String {
    format!(
        "exception_code: 0x{:08X}\nprocess_id: {}\nthread_id: {}\n",
        context.exception_code, context.process_id, context.thread_id
    )
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn crash_fields(context: &crash_handler::CrashContext) -> String {
    format!(
        "signal: {} (0x{:X})\nsignal_code: {}\nsignal_address: 0x{:016X}\nprocess_id: {}\nthread_id: {}\n",
        context.siginfo.ssi_signo,
        context.siginfo.ssi_signo,
        context.siginfo.ssi_code,
        context.siginfo.ssi_addr,
        context.pid,
        context.tid
    )
}

#[cfg(target_os = "macos")]
fn crash_fields(context: &crash_handler::CrashContext) -> String {
    let mut info = format!(
        "task: {}\nthread: {}\nhandler_thread: {}\n",
        context.task, context.thread, context.handler_thread
    );
    if let Some(exc) = context.exception.as_ref() {
        info.push_str(&format!(
            "exception_kind: {}\nexception_code: {}\nexception_subcode: {:?}\n",
            exc.kind, exc.code, exc.subcode
        ));
    }
    info
}

#[cfg(target_os = "windows")]
fn write_dump(context: &crash_handler::CrashContext, file: &mut fs::File, dump_path: &Path) {
    let cc = crash_handler::CrashContext {
        exception_pointers: context.exception_pointers,
        exception_code: context.exception_code,
        process_id: context.process_id,
        thread_id: context.thread_id,
    };
    match minidump_writer::minidump_writer::MinidumpWriter::dump_crash_context(cc, None, file) {
        Ok(()) => eprintln!("crash dump written to {}", dump_path.display()),
        Err(e) => eprintln!("failed to write crash dump: {}", e),
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn write_dump(context: &crash_handler::CrashContext, file: &mut fs::File, dump_path: &Path) {
    let mut writer =
        minidump_writer::minidump_writer::MinidumpWriter::new(context.pid, context.tid);
    writer.set_crash_context(minidump_writer::crash_context::CrashContext {
        inner: context.clone(),
    });
    match writer.dump(file) {
        Ok(_) => eprintln!("crash dump written to {}", dump_path.display()),
        Err(e) => eprintln!("failed to write crash dump: {}", e),
    }
}

#[cfg(target_os = "macos")]
fn write_dump(context: &crash_handler::CrashContext, file: &mut fs::File, dump_path: &Path) {
    let cc = crash_handler::CrashContext {
        task: context.task,
        thread: context.thread,
        handler_thread: context.handler_thread,
        exception: context.exception,
    };
    let mut writer = minidump_writer::minidump_writer::MinidumpWriter::with_crash_context(cc);
    match writer.dump(file) {
        Ok(_) => eprintln!("crash dump written to {}", dump_path.display()),
        Err(e) => eprintln!("failed to write crash dump: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("assistant-crash-{}-{}", name, timestamp_str()))
    }

    #[test]
    fn none_means_no_directory() {
        assert_eq!(prepare_dir(None), None);
    }

    #[test]
    fn creates_missing_nested_directory() {
        let root = scratch("nested");
        let target = root.join("a").join("b");
        let prepared = prepare_dir(Some(target.clone())).expect("目录应可创建");
        assert_eq!(prepared, target);
        assert!(target.is_dir());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unusable_path_falls_back_to_stderr() {
        let root = scratch("blocked");
        fs::create_dir_all(&root).expect("临时根应可创建");
        let blocker = root.join("blocker");
        fs::write(&blocker, b"x").expect("应可写入占位文件");
        assert_eq!(prepare_dir(Some(blocker.join("crashes"))), None);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn panic_hook_writes_into_given_directory() {
        let root = scratch("hook");
        let dir = prepare_dir(Some(root.clone())).expect("目录应可创建");
        install_panic_hook(Some(dir.clone()));
        let _ = std::panic::catch_unwind(|| panic!("t11 probe"));
        let written = fs::read_dir(&dir)
            .expect("目录应可读")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("panic_"))
            .count();
        assert_eq!(written, 1);
        let _ = fs::remove_dir_all(&root);
    }

    /// 诊断产物判据：`panic_*.log` / `crash_*.dmp` / `crash_*.log` 各命中一份，
    /// 无关文件既不计数也不降级（I61 §3.10 E13）。
    #[test]
    fn dump_stats_counts_only_crash_artifacts() {
        let root = scratch("artifacts");
        let dir = prepare_dir(Some(root.clone())).expect("目录应可创建");
        fs::write(dir.join("panic_20260101_000000_000.log"), b"1234").expect("写入应成功");
        fs::write(dir.join("crash_20260101_000000_000.dmp"), b"123456").expect("写入应成功");
        fs::write(dir.join("crash_20260101_000001_000.log"), b"12").expect("写入应成功");
        fs::write(dir.join("notes.txt"), b"ignored").expect("写入应成功");
        fs::write(dir.join("other.log"), b"ignored").expect("写入应成功");

        let stats = dump_stats(&dir).expect("扫描应成功");
        assert_eq!(stats.count, 3, "只认三类诊断产物");
        assert_eq!(stats.bytes, 4 + 6 + 2, "无关文件不计入字节");
        assert!(stats.oldest_at.is_some());
        assert!(stats.newest_at.is_some());
        let _ = fs::remove_dir_all(&root);
    }

    /// 符号链接不计入、也不使整条降级；建链接失败时（未开特权）显式跳过。
    #[test]
    fn dump_stats_skips_symlinks() {
        let root = scratch("symlink");
        let dir = prepare_dir(Some(root.clone())).expect("目录应可创建");
        let outside = root.join("outside.log");
        fs::write(&outside, b"0123456789").expect("写入应成功");
        if !file_link(&outside, &dir.join("panic_linked.log")) {
            eprintln!("skip dump_stats_skips_symlinks: cannot create file symlink");
            let _ = fs::remove_dir_all(&root);
            return;
        }

        let stats = dump_stats(&dir).expect("扫描应成功");
        assert_eq!(stats.count, 0, "符号链接不计入");
        assert_eq!(stats.bytes, 0);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn dump_stats_missing_directory_is_empty_state() {
        let root = scratch("missing");
        let stats = dump_stats(&root.join("does-not-exist")).expect("缺失目录应为空态");
        assert_eq!(stats, DirStats::default(), "无文件时全零、两端为 None");
    }

    fn file_link(target: &Path, link: &Path) -> bool {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(target, link).is_ok()
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (target, link);
            false
        }
    }
}
