use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn crash_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("com.assistant").join("crashes")
}

fn timestamp_str() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    chrono::DateTime::from_timestamp(secs as i64, 0)
        .map(|dt| dt.format("%Y%m%d_%H%M%S").to_string())
        .unwrap_or_else(|| format!("{}", secs))
}

pub fn install() {
    let dir = crash_dir();
    let _ = fs::create_dir_all(&dir);

    install_panic_hook(&dir);
    install_crash_handler(&dir);
}

fn install_panic_hook(crash_dir: &Path) {
    let dir = crash_dir.to_path_buf();
    std::panic::set_hook(Box::new(move |info| {
        let ts = timestamp_str();
        let log_path = dir.join(format!("panic_{}.log", ts));

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

        let _ = fs::write(&log_path, &msg);
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
