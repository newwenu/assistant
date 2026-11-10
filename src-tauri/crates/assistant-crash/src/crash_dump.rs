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
            let _ = fs::write(&log_path, &msg);
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

            let mut info = String::new();
            info.push_str(&format!("=== CRASH at {} ===\n", ts));
            info.push_str(&format!(
                "exception_code: 0x{:08X}\n",
                context.exception_code
            ));
            info.push_str(&format!("process_id: {}\n", context.process_id));
            info.push_str(&format!("thread_id: {}\n", context.thread_id));
            info.push_str(&format!("crash_dump: {}\n", dump_path.display()));

            let _ = fs::write(&log_path, &info);

            match fs::File::create(&dump_path) {
                Ok(mut file) => {
                    let cc = crash_handler::CrashContext {
                        exception_pointers: context.exception_pointers,
                        exception_code: context.exception_code,
                        process_id: context.process_id,
                        thread_id: context.thread_id,
                    };
                    match minidump_writer::minidump_writer::MinidumpWriter::dump_crash_context(
                        cc, None, &mut file,
                    ) {
                        Ok(()) => eprintln!("crash dump written to {}", dump_path.display()),
                        Err(e) => eprintln!("failed to write crash dump: {}", e),
                    }
                }
                Err(e) => eprintln!("failed to create dump file: {}", e),
            }

            crash_handler::CrashEventResult::Handled(true)
        })
    });

    if let Ok(h) = handler {
        std::mem::forget(h);
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
}
