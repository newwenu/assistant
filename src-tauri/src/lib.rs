// 参考: ChatGPT/src-tauri/src/main.rs — Builder 链式配置 + setup + invoke_handler
// 参考: BongoCat/crates/bongocat-app/src/lib.rs — 模块化组织

pub mod adapters;
pub mod app_dirs;
pub mod boot;
pub mod cmd;
pub mod db_state;
mod feature_mounts;
mod feature_plugins;
mod ipc;
mod lifecycle;
#[doc(hidden)]
pub mod mcp;
mod menu;
mod plugins;
pub mod protocol;
mod screenshot;
mod shortcuts;
mod tool_setup;
mod tray;
mod window;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(clock: boot::BootClock, dirs: app_dirs::AppDirs) {
    let report = boot::StartupReport::new();
    plugins::attach(tauri::Builder::default(), &dirs)
        .manage(clock)
        .manage(report)
        .manage(boot::Registry::new())
        .setup(move |app| boot::run_boot(app.handle(), dirs))
        .invoke_handler(ipc::handler())
        .on_menu_event(|app, event| menu::on_menu_event(app, event.id.as_ref()))
        .on_window_event(window::on_window_event)
        .build(tauri::generate_context!("tauri.conf.json"))
        .expect("error while building tauri application")
        .run(lifecycle::on_run_event);
}
