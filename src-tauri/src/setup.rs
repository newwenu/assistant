// 参考: ChatGPT/src-tauri/src/core/setup.rs — setup 中初始化配置和窗口

use std::str::FromStr;
use std::sync::Arc;
use tauri::{App, Manager};

use crate::protocol::events::{emit_broadcast, event_name};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use assistant_config::shortcut::{resolve_bindings, BindingIssue};
use assistant_config::{
    app_conf::AppConf,
    constant::{
        ORB_CHROME_INSET, ORB_COLLAPSED_WINDOW_SIZE, ORB_HITBOX_SIZE, WINDOW_MAIN, WINDOW_ORB,
    },
    orb_conf::OrbConf,
    vault_conf::VaultConf,
};
use assistant_orb::placement;
use assistant_storage::attachment_store::AttachmentStore;
use assistant_storage::db;
use assistant_tools::{
    os, ApprovalPrompter, FileToolResultStorage, InMemoryReadReceiptStore, PermissionAuditStore,
    PermissionGrantStore, ReadReceiptStore, ToolGateway, ToolPipeline, ToolRegistry,
    ToolResultStorage,
};

use super::tray;
use crate::adapters::{
    ApprovalWaitRegistry, DbPermissionAuditStore, DbPermissionGrantStore, DenyAllApprovalPrompter,
    PipelineToolExecution, TimingApprovalPrompter,
};
use crate::protocol::ShortcutWarningPayloadDto;

/// 审批等待上限。P4 的审批默认实现恒 `Deny`，该值只在 P6 接入真实审批 UI 后才真正生效；
/// 超时仍归一为 `Deny`（I38a §2.8），不得默认放行。
const APPROVAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

pub struct DbState(pub std::sync::Mutex<rusqlite::Connection>);

pub fn init(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle();

    if let Err(e) = VaultConf::load() {
        log::error!("Failed to init vault config: {}", e);
    }
    if let Err(e) = AppConf::load() {
        log::error!("Failed to init app config: {}", e);
    }
    if let Err(e) = OrbConf::load() {
        log::error!("Failed to init orb config: {}", e);
    }

    {
        let locale = AppConf::load()
            .map(|c| c.language)
            .unwrap_or_else(|_| "zh-CN".to_string());
        assistant_i18n::init_global(&locale);
    }

    match handle.path().app_data_dir() {
        Ok(data_dir) => {
            let db_path = data_dir.join("assistant.db");
            match db::init_db(&db_path) {
                Ok(conn) => {
                    match assistant_storage::conversations::mark_streaming_messages_interrupted(
                        &conn,
                    ) {
                        Ok(0) => {}
                        Ok(count) => {
                            log::warn!("startup sweep: {count} streaming messages -> interrupted");
                        }
                        Err(e) => {
                            log::warn!("startup sweep failed: {e}");
                        }
                    }
                    handle.manage(DbState(std::sync::Mutex::new(conn)));
                }
                Err(e) => {
                    log::error!("Failed to init database: {}", e);
                }
            }
        }
        Err(e) => {
            log::error!("Failed to resolve app data dir: {}", e);
        }
    }

    {
        let attachments_dir = match handle.path().app_data_dir() {
            Ok(data_dir) => data_dir.join("attachments"),
            Err(e) => {
                log::error!("Failed to resolve app data dir for attachments: {}", e);
                return Err(Box::new(e));
            }
        };
        std::fs::create_dir_all(&attachments_dir).expect("Failed to create attachments directory");
        let store = AttachmentStore::new(attachments_dir);
        handle.manage(store);
        log::info!("AttachmentStore initialized");

        {
            let store = handle.state::<AttachmentStore>().inner().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = store.clean_tmp().await {
                    log::warn!("Failed to clean tmp attachments: {}", e);
                }
            });
        }
    }

    handle.manage(Arc::new(assistant_agent::kernel::TurnTasks::new()));
    handle.manage(Arc::new(assistant_agent::kernel::SessionStates::new()));
    handle.manage(std::sync::Arc::new(crate::clock::ClockRuntime::new()));

    {
        let registry = Arc::new(ToolRegistry::new());
        os::register_os_tools(&registry);

        // P5 注册门禁（I38c §7.1）：生产只放开只读组。修改组要求 Gateway 审批可用，
        // 危险组还要求前端审批卡片上线，两者都不得在此处调用。
        let receipts: Arc<dyn ReadReceiptStore> = Arc::new(InMemoryReadReceiptStore::new());
        os::register_readonly_tools(&registry, receipts.clone());
        handle.manage(receipts);

        // C4.18：P2 遗留端口的生产装配。顺序固定 —— 授权/审计 → 审批（fail-closed + 计时）
        // → 网关 → 流水线 → executor 端口适配器；网关只认 `ApprovalPrompter` trait，
        // 计时装饰器对内层透明。
        let grants: Arc<dyn PermissionGrantStore> =
            Arc::new(DbPermissionGrantStore::new(handle.clone()));
        let audit: Arc<dyn PermissionAuditStore> =
            Arc::new(DbPermissionAuditStore::new(handle.clone()));
        let waits = ApprovalWaitRegistry::new();
        let prompter: Arc<dyn ApprovalPrompter> = Arc::new(TimingApprovalPrompter::with_registry(
            Arc::new(DenyAllApprovalPrompter::new()),
            waits.clone(),
        ));
        let gateway = Arc::new(ToolGateway::new(grants, audit, prompter, APPROVAL_TIMEOUT));

        // 落盘根目录必须是稳定路径：它会写进 `<persisted-output path="…">`，
        // 跨进程重放一致性依赖它（§4.5），不得用临时目录。
        let results_dir = match handle.path().app_data_dir() {
            Ok(data_dir) => data_dir.join("tool-results"),
            Err(e) => {
                log::error!("Failed to resolve app data dir for tool results: {}", e);
                return Err(Box::new(e));
            }
        };
        let storage: Arc<dyn ToolResultStorage> = Arc::new(FileToolResultStorage::new(results_dir));
        let pipeline = Arc::new(ToolPipeline::new(registry.clone(), gateway, storage));

        // P4 无会话工作区绑定，`workspace_root` 恒 `None`；P6 接入后再按会话填充。
        let tool_exec: Arc<dyn assistant_agent::agent::round_executor::ToolExecutionPort> =
            Arc::new(PipelineToolExecution::new(pipeline, waits, None));
        handle.manage(tool_exec);

        handle.manage(registry);
        log::info!("OS tool registry initialized");
    }

    tray::create_tray(handle);

    position_orb_window(handle);
    if let Err(e) = crate::window::restore_main_window(handle) {
        log::warn!("Failed to restore main window geometry: {}", e);
    }
    show_orb_window(handle);

    spawn_screen_inspection(handle.clone());

    register_global_shortcuts(handle);

    log::info!("Assistant started");
    Ok(())
}

fn parse_shortcut(s: &str) -> Option<Shortcut> {
    let mut modifiers = Modifiers::empty();
    let mut code: Option<Code> = None;

    for part in s.split('+') {
        let part = part.trim();
        match part.to_lowercase().as_str() {
            "meta" | "super" | "cmd" | "win" | "windows" => modifiers |= Modifiers::META,
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "shift" => modifiers |= Modifiers::SHIFT,
            "alt" | "option" => modifiers |= Modifiers::ALT,
            _ => {
                let key_part = part.to_uppercase();
                if let Ok(c) = Code::from_str(&key_part) {
                    code = Some(c);
                } else if let Ok(c) = Code::from_str(&format!("Key{}", key_part)) {
                    code = Some(c);
                } else if let Ok(c) = Code::from_str(&format!("Digit{}", key_part)) {
                    code = Some(c);
                }
            }
        }
    }

    code.map(|c| Shortcut::new(Some(modifiers), c))
}

/// 按下快捷键时执行的回调
type ShortcutAction = fn(&tauri::AppHandle);

/// 全局快捷键的静态定义：id 用于配置查找，第二项是按下时执行的回调。
static SHORTCUT_SPECS: &[(&str, ShortcutAction)] = &[
    ("toggle_orb", toggle_orb_action),
    ("toggle_main", toggle_main_action),
    ("new_chat", new_chat_action),
];

fn toggle_orb_action(app: &tauri::AppHandle) {
    let _ = crate::window::toggle(app, WINDOW_ORB);
}

fn toggle_main_action(app: &tauri::AppHandle) {
    let _ = crate::window::toggle(app, WINDOW_MAIN);
}

fn new_chat_action(app: &tauri::AppHandle) {
    emit_broadcast(app, event_name::ORB_NEW_CONVERSATION, ());
}

fn emit_shortcut_warning(handle: &tauri::AppHandle, issue: &BindingIssue) {
    log::warn!(
        "Shortcut {} ({}) unavailable: {}",
        issue.id,
        issue.binding,
        issue.error
    );
    emit_broadcast(
        handle,
        event_name::SHORTCUT_WARNING,
        ShortcutWarningPayloadDto {
            id: issue.id.to_string(),
            binding: issue.binding.clone(),
            error: issue.error.clone(),
        },
    );
}

pub fn register_global_shortcuts(handle: &tauri::AppHandle) {
    let conf = match AppConf::load() {
        Ok(c) => c,
        Err(_) => return,
    };

    let defaults = assistant_config::app_conf::default_keybindings();
    let (resolved, issues) =
        resolve_bindings(SHORTCUT_SPECS, &conf.keybindings, &defaults, parse_shortcut);
    for issue in &issues {
        emit_shortcut_warning(handle, issue);
    }

    let gs = handle.global_shortcut();
    let _ = gs.unregister_all();

    for item in resolved {
        let action = item.action;
        let id = item.id;
        let binding = item.binding.clone();
        if let Err(e) = gs.on_shortcut(item.shortcut, move |app, _shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            action(app);
        }) {
            emit_shortcut_warning(
                handle,
                &BindingIssue {
                    id,
                    binding,
                    error: e.to_string(),
                },
            );
        }
    }
}

fn position_orb_window(handle: &tauri::AppHandle) {
    let Some(window) = handle.get_webview_window(WINDOW_ORB) else {
        return;
    };

    let screens = placement::get_available_screens(handle);
    let scale = window.scale_factor().unwrap_or(1.0);
    let collapsed_phys = (ORB_COLLAPSED_WINDOW_SIZE as f64 * scale) as u32;

    if let Ok(conf) = OrbConf::load() {
        if let (Some(x), Some(y)) = (conf.position_x, conf.position_y) {
            if let Some(snapped) = placement::ensure_orb_on_screen(
                &screens,
                x,
                y,
                collapsed_phys,
                collapsed_phys,
                ORB_CHROME_INSET,
                ORB_HITBOX_SIZE,
            ) {
                let _ = window.set_position(tauri::PhysicalPosition::new(snapped.x, snapped.y));
            } else {
                let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
            }
            return;
        }
    }

    let Some(primary) = screens.first() else {
        return;
    };
    let edge_margin = 20;
    let x = primary.x + primary.width as i32 - collapsed_phys as i32 - edge_margin;
    let y = primary.y + primary.height as i32 - collapsed_phys as i32 - edge_margin;
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

/// 两个窗口在 `tauri.conf.json` 中均为 `visible: false`：
/// 主窗口只在托盘/Orb/快捷键入口唤起时显示，Orb 则在坐标收敛完成后再显示，
/// 避免"先出现在系统默认位置、再跳到保存位置"的启动跳动。
fn show_orb_window(handle: &tauri::AppHandle) {
    let Some(window) = handle.get_webview_window(WINDOW_ORB) else {
        return;
    };
    if let Err(e) = window.show() {
        log::warn!("Failed to show orb window: {}", e);
    }
}

fn spawn_screen_inspection(handle: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(placement::PLACEMENT_INSPECTION_INTERVAL);
        if crate::lifecycle::should_exit() {
            break;
        }
        let Some(window) = handle.get_webview_window(WINDOW_ORB) else {
            continue;
        };
        let Ok(pos) = window.outer_position() else {
            continue;
        };
        let Ok(size) = window.outer_size() else {
            continue;
        };
        let screens = placement::get_available_screens(&handle);
        let Some(snapped) = placement::ensure_orb_on_screen(
            &screens,
            pos.x,
            pos.y,
            size.width,
            size.height,
            ORB_CHROME_INSET,
            ORB_HITBOX_SIZE,
        ) else {
            continue;
        };
        let _ = window.set_position(tauri::PhysicalPosition::new(snapped.x, snapped.y));
        if let Ok(mut conf) = OrbConf::load() {
            conf.position_x = Some(snapped.x);
            conf.position_y = Some(snapped.y);
            let _ = conf.save();
        }
    });
}
