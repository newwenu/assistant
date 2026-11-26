// 参考: ChatGPT/src-tauri/src/main.rs — Builder 链式插件注册

use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

use crate::app_dirs::AppDirs;

pub fn attach(
    mut builder: tauri::Builder<tauri::Wry>,
    dirs: &AppDirs,
) -> tauri::Builder<tauri::Wry> {
    builder = builder
        // 可选功能分区的通用接收槽在 `Builder` 阶段建好：插件 `setup` 早于 app `setup`，
        // 分区注册时槽必须已存在（G18(b)）。关闭特性时槽仍存在、只是没人注册。
        .manage(std::sync::Arc::new(
            crate::cmd::storage::section::SectionRegistry::new(),
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        // single-instance 必须排在 log_plugin 之前：插件 setup 按注册顺序执行，
        // 第二个实例要尽早退出，否则日志插件会先把正在写入的日志文件轮转掉。
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            crate::window::show_or_create_main(app);
        }))
        .plugin(log_plugin(dirs))
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
fn log_plugin(dirs: &AppDirs) -> tauri::plugin::TauriPlugin<tauri::Wry> {
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
        .level_for("keyring_core", log::LevelFilter::Info)
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
    // 日志与崩溃同根（local 根），显式给出目录，不用插件的 LogDir 默认落点。
    builder
        .target(Target::new(TargetKind::Folder {
            path: dirs.logs(),
            file_name: Some(file_name.into()),
        }))
        .build()
}
