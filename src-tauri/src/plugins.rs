// 参考: ChatGPT/src-tauri/src/main.rs — Builder 链式插件注册

use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

pub fn attach(mut builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder = builder
        // 可选能力 provider 的通用槽在 `Builder` 阶段建好：插件 `setup` 早于 app `setup`，
        // provider 注册时槽必须已存在（G18(b)）。关闭特性时槽仍存在、只是没人注册。
        .manage(std::sync::Arc::new(
            crate::cmd::storage::probe::ProbeRegistry::new(),
        ))
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
        ));
    for mount in crate::feature_mounts::mounts() {
        builder = builder.plugin((mount.plugin)());
    }
    builder
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

    let file_name = if is_dev { "assistant-dev" } else { "assistant" };
    match crate::app_dirs::AppDirs::resolve() {
        // 日志与崩溃同根（local 根），显式给出目录，不用插件的 LogDir 默认落点。
        Ok(dirs) => builder.target(Target::new(TargetKind::Folder {
            path: dirs.logs(),
            file_name: Some(file_name.into()),
        })),
        Err(e) => {
            eprintln!("file logging disabled: {}", e);
            builder
        }
    }
    .build()
}
