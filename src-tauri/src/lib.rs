// 参考: ChatGPT/src-tauri/src/main.rs — Builder 链式配置 + setup + invoke_handler
// 参考: BongoCat/crates/bongocat-app/src/lib.rs — 模块化组织

mod cmd;
mod ipc;
mod lifecycle;
mod menu;
mod plugins;
mod probe_plugin;
pub mod protocol;
mod setup;
mod tray;
mod vault_plugin;
mod window;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    plugins::attach(tauri::Builder::default())
        .setup(setup::init)
        .invoke_handler(ipc::handler())
        .on_menu_event(|app, event| menu::on_menu_event(app, event.id.as_ref()))
        .on_window_event(window::on_window_event)
        .build(tauri::generate_context!("tauri.conf.json"))
        .expect("error while building tauri application")
        .run(lifecycle::on_run_event);
}
