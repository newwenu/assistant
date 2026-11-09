// 参考: ChatGPT/src-tauri/src/main.rs — Builder 链式插件注册

use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

pub fn attach(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(log_plugin())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            crate::window::show_or_create_main(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(crate::vault_plugin::init())
}

/// 日志插件配置
///
/// 档位由 `debug_assertions` 决定，不读配置：日志配置错误会让排障链本身失效。
fn log_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let is_dev = cfg!(debug_assertions);

    let mut builder = tauri_plugin_log::Builder::new()
        .level(if is_dev {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .level_for("ignore", log::LevelFilter::Off)
        .level_for("ignore::walk", log::LevelFilter::Off)
        .level_for("globset", log::LevelFilter::Off)
        .level_for("grep_searcher", log::LevelFilter::Warn)
        .level_for("hyper_util", log::LevelFilter::Info)
        .level_for("h2", log::LevelFilter::Info)
        .level_for("reqwest", log::LevelFilter::Info)
        .level_for("tao", log::LevelFilter::Warn)
        .level_for("wry", log::LevelFilter::Warn)
        .rotation_strategy(RotationStrategy::KeepSome(3))
        .max_file_size(10 * 1024 * 1024)
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .file_open_strategy(tauri_plugin_log::FileOpenStrategy::Rotate)
        .clear_targets();

    if is_dev {
        builder = builder.target(Target::new(TargetKind::Stdout));
    }

    builder
        .target(Target::new(TargetKind::LogDir {
            file_name: Some(if is_dev { "assistant-dev" } else { "assistant" }.into()),
        }))
        .build()
}
