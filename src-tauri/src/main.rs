#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 崩溃目录由应用层解析后传入：崩溃 crate 不自算基根，也不回落到当前目录。
    let crash_dir = match assistant_app::app_dirs::AppDirs::resolve() {
        Ok(dirs) => Some(dirs.crashes()),
        Err(e) => {
            eprintln!("无法解析应用目录，退出：{}", e);
            std::process::exit(1);
        }
    };

    assistant_crash::install(crash_dir);

    #[cfg(target_os = "windows")]
    {
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            "--ignore-gpu-blocklist",
        );
    }

    assistant_app::run();
}
