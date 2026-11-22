#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 崩溃目录由应用层解析后传入：崩溃 crate 不自算基根，也不回落到当前目录。
    let dirs = match assistant_app::app_dirs::AppDirs::resolve() {
        Ok(dirs) => dirs,
        Err(e) => {
            eprintln!("无法解析应用目录，退出：{}", e);
            std::process::exit(1);
        }
    };

    assistant_crash::install(Some(dirs.crashes()));

    // `E14` 接管创建与回收（I67 §13.3）：必须在 `Builder::run` 之前——
    // 配置窗口在 `build()` 里创建，WebView2 一旦打开 profile，目录就无法由应用保证存在，
    // 缓存也无法安全删除。运行期的清理请求只写标记，唯一安全执行点就是这里。
    if let Err(e) = dirs.ensure_webview_profile() {
        eprintln!("无法创建 WebView2 profile 目录：{}", e);
    }
    if let Some(report) =
        assistant_app::app_dirs::webview_profile::run_pending_purge(&dirs.webview_profile())
    {
        eprintln!(
            "WebView2 缓存清理：removed={} bytes={} failed={} skipped={}",
            report.removed, report.removed_bytes, report.failed, report.skipped
        );
    }

    #[cfg(target_os = "windows")]
    {
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            "--ignore-gpu-blocklist",
        );
    }

    assistant_app::run();
}
