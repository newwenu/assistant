// 参考: ChatGPT/src-tauri/src/core/cmd.rs — 命令集中管理
// 参考: WindowPet/cmd.rs — 鼠标位置获取

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use log::error;
use mouse_position::mouse_position::Mouse;
use rusqlite::params;
use serde_json::json;
use tauri::{command, AppHandle, Emitter, Manager, PhysicalPosition};
use uuid::Uuid;
use chrono::Utc;

use assistant_config::{
    app_conf::AppConf,
    constant::{ORB_SNAP_ANIMATION_MS, WINDOW_EDGE_BUFFER, WINDOW_ORB},
    orb_conf::OrbConf,
};
use assistant_orb::placement;
use assistant_providers::{
    base::{self, ChatMessage, ChatOptions, MessageRole, ProviderConfig, ProviderError, ProviderErrorPayload},
    crypto,
};
use assistant_storage::{
    conversations::{self, ConversationRow, MessageRow},
    providers::{self as provider_store, ProviderRow},
    templates::{self as template_store, PromptTemplateRow},
};
use crate::request_exit;
use crate::chat_tasks::ChatTasks;

static ORB_CLICK_THROUGH: AtomicBool = AtomicBool::new(false);
static ORB_SNAP_CANCEL: AtomicBool = AtomicBool::new(false);

#[command]
pub fn get_mouse_position() -> serde_json::Value {
    let position = Mouse::get_mouse_position();
    match position {
        Mouse::Position { x, y } => json!({
            "clientX": x,
            "clientY": y
        }),
        Mouse::Error => {
            error!("Error getting mouse position");
            json!(null)
        }
    }
}

#[command]
pub fn get_app_conf() -> serde_json::Value {
    match AppConf::load() {
        Ok(conf) => serde_json::to_value(&conf).unwrap_or(json!(null)),
        Err(e) => {
            error!("Failed to load app config: {}", e);
            json!(null)
        }
    }
}

#[command]
pub fn set_app_conf(conf: AppConf) -> Result<(), String> {
    conf.save().map_err(|e| e.to_string())
}

#[command]
pub fn get_orb_conf() -> serde_json::Value {
    match OrbConf::load() {
        Ok(conf) => serde_json::to_value(&conf).unwrap_or(json!(null)),
        Err(e) => {
            error!("Failed to load orb config: {}", e);
            json!(null)
        }
    }
}

#[command]
pub fn set_orb_conf(conf: OrbConf) -> Result<(), String> {
    conf.save().map_err(|e| e.to_string())
}

#[command]
pub fn set_theme(theme: String) -> Result<(), String> {
    let mut conf = AppConf::load().map_err(|e| e.to_string())?;
    conf.theme = theme;
    conf.save().map_err(|e| e.to_string())
}

#[command]
pub fn emit_theme_changed(app: AppHandle, resolved: String) -> Result<(), String> {
    app.emit("theme-changed", serde_json::json!({ "resolved": resolved }))
        .map_err(|e| e.to_string())
}

#[command]
pub fn toggle_orb_visibility(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(WINDOW_ORB)
        .ok_or_else(|| "Orb window not found".to_string())?;
    if window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false) {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.set_focus();
    }
    Ok(())
}

#[command]
pub fn set_orb_click_through(app: AppHandle, enabled: bool) -> Result<(), String> {
    if ORB_CLICK_THROUGH.load(Ordering::Relaxed) == enabled {
        return Ok(());
    }
    let window = app
        .get_webview_window(WINDOW_ORB)
        .ok_or_else(|| "Orb window not found".to_string())?;
    window
        .set_ignore_cursor_events(enabled)
        .map_err(|e| e.to_string())?;
    ORB_CLICK_THROUGH.store(enabled, Ordering::Relaxed);
    log::debug!("Orb click-through set to {}", enabled);
    Ok(())
}

#[command]
pub fn get_screen_size(app: AppHandle, center_x: f64, center_y: f64, scale_factor: f64) -> serde_json::Value {
    let cx_phys = center_x * scale_factor;
    let cy_phys = center_y * scale_factor;
    let screens = placement::get_available_screens(&app);
    let best = screens.iter().min_by_key(|s| {
        let sx = s.x + s.width as i32 / 2;
        let sy = s.y + s.height as i32 / 2;
        let dx = cx_phys as i64 - sx as i64;
        let dy = cy_phys as i64 - sy as i64;
        dx * dx + dy * dy
    });
    match best {
        Some(s) => json!({
            "screen": { "x": s.x, "y": s.y, "width": s.width, "height": s.height }
        }),
        None => json!({ "screen": { "x": 0, "y": 0, "width": 1920, "height": 1080 } }),
    }
}

#[command]
pub fn snap_orb_to_screen(app: AppHandle) -> Result<(), String> {
    ORB_SNAP_CANCEL.store(true, Ordering::Relaxed);

    let window = app
        .get_webview_window(WINDOW_ORB)
        .ok_or_else(|| "Orb window not found".to_string())?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.outer_size().map_err(|e| e.to_string())?;

    let screens = placement::get_available_screens(&app);
    let bounds = placement::WindowBounds::new(pos.x, pos.y, size.width, size.height);
    let target = placement::snap_orb_to_edge(&screens, bounds, WINDOW_EDGE_BUFFER as i32);

    if target.x == bounds.x && target.y == bounds.y {
        let _ = save_orb_position_sync(&app);
        return Ok(());
    }

    ORB_SNAP_CANCEL.store(false, Ordering::Relaxed);

    let start_x = bounds.x as f64;
    let start_y = bounds.y as f64;
    let end_x = target.x as f64;
    let end_y = target.y as f64;
    let duration = Duration::from_millis(ORB_SNAP_ANIMATION_MS);
    let step = Duration::from_millis(16);

    std::thread::spawn(move || {
        let start = Instant::now();
        loop {
            if ORB_SNAP_CANCEL.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(step);
            let elapsed = start.elapsed();
            if elapsed >= duration {
                let _ = window.set_position(PhysicalPosition::new(target.x, target.y));
                break;
            }
            let t = elapsed.as_secs_f64() / duration.as_secs_f64();
            let t = 1.0 - (1.0 - t).powi(3);
            let x = (start_x + (end_x - start_x) * t).round() as i32;
            let y = (start_y + (end_y - start_y) * t).round() as i32;
            let _ = window.set_position(PhysicalPosition::new(x, y));
        }
        if let Ok(pos) = window.outer_position() {
            if let Ok(mut conf) = OrbConf::load() {
                conf.position_x = Some(pos.x);
                conf.position_y = Some(pos.y);
                let _ = conf.save();
            }
        }
    });

    Ok(())
}

fn save_orb_position_sync(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(WINDOW_ORB)
        .ok_or_else(|| "Orb window not found".to_string())?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let mut conf = OrbConf::load().map_err(|e| e.to_string())?;
    conf.position_x = Some(pos.x);
    conf.position_y = Some(pos.y);
    conf.save().map_err(|e| e.to_string())
}

#[command]
pub fn save_orb_position(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(WINDOW_ORB)
        .ok_or_else(|| "Orb window not found".to_string())?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let mut conf = OrbConf::load().map_err(|e| e.to_string())?;
    conf.position_x = Some(pos.x);
    conf.position_y = Some(pos.y);
    conf.save().map_err(|e| e.to_string())
}

#[command]
pub fn app_quit(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(WINDOW_ORB) {
        if let Ok(pos) = w.outer_position() {
            if let Ok(mut conf) = OrbConf::load() {
                conf.position_x = Some(pos.x);
                conf.position_y = Some(pos.y);
                let _ = conf.save();
            }
        }
        let _ = w.destroy();
    }
    request_exit();
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.destroy();
    }
    app.exit(0);
    Ok(())
}

fn get_db_conn<'a>(state: &'a tauri::State<'a, crate::setup::DbState>) -> Result<std::sync::MutexGuard<'a, rusqlite::Connection>, String> {
    state.0.lock().map_err(|e| format!("DB lock: {}", e))
}

fn mask_api_key_encrypted(encrypted: &str) -> String {
    if encrypted.is_empty() {
        return String::new();
    }
    let machine_id = match crypto::get_machine_id() {
        Ok(id) => id,
        Err(_) => return "••••".to_string(),
    };
    match crypto::decrypt_api_key_b64(encrypted, &machine_id) {
        Ok(plain) => {
            if plain.len() <= 7 {
                "••••".to_string()
            } else {
                format!("{}****{}", &plain[..3], &plain[plain.len() - 4..])
            }
        }
        Err(_) => "••••".to_string(),
    }
}

fn build_provider_config(row: &ProviderRow) -> ProviderConfig {
    ProviderConfig {
        kind: base::provider_kind_from_str(&row.type_, row.base_url.clone()),
        api_key_encrypted: row.api_key_encrypted.clone(),
        base_url: Some(row.base_url.clone()),
        default_model: row.default_model.clone(),
        max_tokens: 4096,
        temperature: 0.7,
    }
}

#[command]
pub fn list_providers(app: AppHandle) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    let rows = provider_store::get_providers(&conn)?;
    let masked: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.id,
                "name": r.name,
                "type": r.type_,
                "base_url": r.base_url,
                "api_key_masked": mask_api_key_encrypted(&r.api_key_encrypted),
                "models": r.models_json,
                "default_model": r.default_model,
                "updated_at": r.updated_at,
            })
        })
        .collect();
    Ok(json!(masked))
}

#[command]
pub fn save_provider(app: AppHandle, provider: serde_json::Value) -> Result<(), String> {
    let id = provider["id"].as_str().unwrap_or("").to_string();
    let name = provider["name"].as_str().ok_or("name required")?.to_string();
    let type_ = provider["type"].as_str().ok_or("type required")?.to_string();
    let base_url = provider["base_url"].as_str().ok_or("base_url required")?.to_string();
    let api_key_plain = provider["api_key"].as_str().unwrap_or("").to_string();
    let models_json = provider["models"].as_str().unwrap_or("[]").to_string();
    let default_model = provider["default_model"].as_str().unwrap_or("").to_string();

    let machine_id = crypto::get_machine_id()?;
    let now = Utc::now().timestamp_millis();

    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;

    if id.is_empty() {
        let new_id = Uuid::new_v4().to_string();
        let api_key_encrypted = if !api_key_plain.is_empty() {
            crypto::encrypt_api_key_b64(&api_key_plain, &machine_id)?
        } else {
            String::new()
        };
        let row = ProviderRow {
            id: new_id,
            name,
            type_,
            base_url,
            api_key_encrypted,
            models_json,
            default_model,
            updated_at: now,
        };
        provider_store::insert_provider(&conn, &row)?;
    } else {
        let existing = provider_store::get_provider_by_id(&conn, &id)?;
        let api_key_encrypted = if api_key_plain.is_empty() || api_key_plain.starts_with("••••") || api_key_plain.contains("***") {
            existing.map(|e| e.api_key_encrypted).unwrap_or_default()
        } else {
            crypto::encrypt_api_key_b64(&api_key_plain, &machine_id)?
        };
        let row = ProviderRow {
            id,
            name,
            type_,
            base_url,
            api_key_encrypted,
            models_json,
            default_model,
            updated_at: now,
        };
        provider_store::update_provider(&conn, &row)?;
    }

    let _ = app.emit("setting:provider-changed", json!({
        "action": "provider-saved",
    }));
    Ok(())
}

#[command]
pub fn delete_provider(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    provider_store::delete_provider(&conn, &id)?;
    let _ = app.emit("setting:provider-changed", json!({
        "action": "provider-deleted",
        "id": id,
    }));
    Ok(())
}

#[command]
pub fn list_models_cmd(app: AppHandle, provider_id: String) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    let row = provider_store::get_provider_by_id(&conn, &provider_id)?
        .ok_or_else(|| "Provider not found".to_string())?;

    let config = build_provider_config(&row);
    let adapter = base::create_provider_adapter(config);
    let models = adapter.list_models().map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(&models).map_err(|e| e.to_string())?)
}

#[command]
pub fn preview_models_cmd(
    provider_type: String,
    base_url: String,
    api_key: String,
) -> Result<serde_json::Value, String> {
    let api_key_encrypted = crypto::encrypt_api_key_b64(&api_key, &crypto::get_machine_id()?)?;
    let config = ProviderConfig {
        kind: base::provider_kind_from_str(&provider_type, base_url.clone()),
        api_key_encrypted,
        base_url: Some(base_url),
        default_model: String::new(),
        max_tokens: 4096,
        temperature: 0.7,
    };
    let adapter = base::create_provider_adapter(config);
    let models = adapter.list_models().map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(&models).map_err(|e| e.to_string())?)
}

#[command]
pub fn create_conversation(app: AppHandle, provider_id: String, model_id: String) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp_millis();
    let row = ConversationRow {
        id: id.clone(),
        title: "新对话".to_string(),
        provider_id: Some(provider_id),
        model_id,
        system_prompt: None,
        created_at: now,
        updated_at: now,
    };
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    conversations::insert_conversation(&conn, &row)?;
    Ok(id)
}

#[command]
pub fn list_conversations(app: AppHandle, provider_id: Option<String>) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    let rows = conversations::get_conversations(&conn, provider_id.as_deref())?;
    Ok(serde_json::to_value(&rows).map_err(|e| e.to_string())?)
}

#[command]
pub fn get_conversation(app: AppHandle, id: String) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    let detail = conversations::get_conversation_detail(&conn, &id)?;
    Ok(serde_json::to_value(&detail).map_err(|e| e.to_string())?)
}

#[command]
pub fn delete_conversation(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    conversations::delete_conversation(&conn, &id)
}

#[command]
pub fn rename_conversation(app: AppHandle, id: String, title: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    let now = Utc::now().timestamp_millis();
    let mut detail = conversations::get_conversation_detail(&conn, &id)?
        .ok_or_else(|| "Conversation not found".to_string())?;
    detail.conversation.title = title;
    detail.conversation.updated_at = now;
    conversations::update_conversation(&conn, &detail.conversation)
}

#[command]
pub fn update_conversation_model(app: AppHandle, id: String, model_id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    let now = Utc::now().timestamp_millis();
    let mut detail = conversations::get_conversation_detail(&conn, &id)?
        .ok_or_else(|| "Conversation not found".to_string())?;
    detail.conversation.model_id = model_id;
    detail.conversation.updated_at = now;
    conversations::update_conversation(&conn, &detail.conversation)
}

#[command]
pub fn update_conversation_provider(app: AppHandle, id: String, provider_id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    let now = Utc::now().timestamp_millis();
    let mut detail = conversations::get_conversation_detail(&conn, &id)?
        .ok_or_else(|| "Conversation not found".to_string())?;
    detail.conversation.provider_id = Some(provider_id);
    detail.conversation.updated_at = now;
    conversations::update_conversation(&conn, &detail.conversation)
}

#[command]
pub fn list_prompt_templates(app: AppHandle) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    let rows = template_store::get_templates(&conn)?;
    Ok(serde_json::to_value(&rows).map_err(|e| e.to_string())?)
}

#[command]
pub fn save_prompt_template(app: AppHandle, template: serde_json::Value) -> Result<(), String> {
    let id = template["id"].as_str().unwrap_or("").to_string();
    let name = template["name"].as_str().ok_or("name required")?.to_string();
    let category = template["category"].as_str().ok_or("category required")?.to_string();
    let content = template["content"].as_str().ok_or("content required")?.to_string();
    let variables_json = template["variables"].as_str().map(|s| s.to_string());
    let is_builtin = template["is_builtin"].as_bool().unwrap_or(false);

    let row = PromptTemplateRow {
        id: if id.is_empty() { Uuid::new_v4().to_string() } else { id },
        name,
        category,
        content,
        variables_json,
        is_builtin,
    };
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    template_store::insert_template(&conn, &row)
}

#[command]
pub fn send_chat_message(app: AppHandle, conversation_id: String, content: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    let now = Utc::now().timestamp_millis();

    let user_msg = MessageRow {
        id: Uuid::new_v4().to_string(),
        conversation_id: conversation_id.clone(),
        role: "user".to_string(),
        content: content.clone(),
        tokens: None,
        timestamp: now,
    };
    conversations::insert_message(&conn, &user_msg)?;

    let detail = conversations::get_conversation_detail(&conn, &conversation_id)?
        .ok_or_else(|| "Conversation not found".to_string())?;
    let provider_id = detail.conversation.provider_id
        .ok_or_else(|| "此对话的 Provider 已被删除，请重新选择 Provider 后再发送".to_string())?;
    let provider_row = provider_store::get_provider_by_id(&conn, &provider_id)?
        .ok_or_else(|| "Provider not found".to_string())?;

    let assistant_msg_id = Uuid::new_v4().to_string();
    let assistant_msg = MessageRow {
        id: assistant_msg_id.clone(),
        conversation_id: conversation_id.clone(),
        role: "assistant".to_string(),
        content: String::new(),
        tokens: None,
        timestamp: now + 1,
    };
    conversations::insert_message(&conn, &assistant_msg)?;

    let chat_messages: Vec<ChatMessage> = detail
        .messages
        .iter()
        .map(|m| ChatMessage {
            role: base::message_role_from_str(&m.role),
            content: m.content.clone(),
        })
        .chain(std::iter::once(ChatMessage {
            role: MessageRole::User,
            content,
        }))
        .collect();

    let config = build_provider_config(&provider_row);
    let adapter = base::create_provider_adapter(config);
    let model = detail.conversation.model_id.clone();
    let options = ChatOptions {
        temperature: 0.7,
        max_tokens: 4096,
        stream: true,
    };

    let chat_tasks: tauri::State<'_, ChatTasks> = app.state::<ChatTasks>();
    let cancel_token = chat_tasks.start(conversation_id.clone());

    let app_clone = app.clone();
    let conv_id = conversation_id.clone();
    let msg_id = assistant_msg_id.clone();

    let (db_tx, mut db_rx) = tokio::sync::mpsc::unbounded_channel::<(String, String)>();
    let db_app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some((msg_id, content)) = db_rx.recv().await {
            let state = db_app.state::<crate::setup::DbState>();
            match get_db_conn(&state) {
                Ok(conn) => {
                    if let Err(db_err) = conversations::update_message_content(&conn, &msg_id, &content, None) {
                        error!("Failed to save content: {}", db_err);
                    }
                }
                Err(_) => {
                    error!("DB lock failed, content not saved ({} bytes)", content.len());
                }
            };
        }
    });

    let (finish_tx, mut finish_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let finish_app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(conv_id) = finish_rx.recv().await {
            let tasks = finish_app.state::<ChatTasks>();
            tasks.finish(&conv_id);
        }
    });

    let stream_future = async move {
        let stream = match adapter.chat_completion_stream(&chat_messages, &model, &options) {
            Ok(s) => s,
            Err(e) => {
                let payload = ProviderErrorPayload::from(&e);
                if let Err(emit_err) = app_clone.emit("chat-delta", json!({
                    "conversation_id": conv_id,
                    "message_id": msg_id,
                    "error": payload,
                })) {
                    error!("Failed to emit stream error event: {}", emit_err);
                }
                let _ = finish_tx.send(conv_id);
                return;
            }
        };
        use futures::StreamExt;
        let mut stream = stream;
        let mut full_content = String::new();
        while let Some(result) = stream.next().await {
            if cancel_token.is_cancelled() {
                let _ = db_tx.send((msg_id.clone(), full_content.clone()));
                if let Err(emit_err) = app_clone.emit("chat-delta", json!({
                    "conversation_id": conv_id,
                    "message_id": msg_id,
                    "error": ProviderErrorPayload::from(&ProviderError::Cancelled),
                })) {
                    error!("Failed to emit cancel event: {}", emit_err);
                }
                let _ = finish_tx.send(conv_id);
                break;
            }
            match result {
                Ok(delta) => {
                    if let Some(text) = &delta.delta_text {
                        full_content.push_str(text);
                    }
                    if let Err(emit_err) = app_clone.emit("chat-delta", json!({
                        "conversation_id": conv_id,
                        "message_id": msg_id,
                        "delta_text": delta.delta_text,
                        "delta_reasoning": delta.delta_reasoning,
                        "delta_tool_call_id": delta.delta_tool_call_id,
                        "delta_tool_call_name": delta.delta_tool_call_name,
                        "delta_tool_call_arguments": delta.delta_tool_call_arguments,
                        "finish_reason": delta.finish_reason,
                        "usage": delta.usage,
                    })) {
                        error!("Failed to emit chat-delta: {}", emit_err);
                    }
                    if delta.finish_reason.is_some() {
                        let _ = db_tx.send((msg_id.clone(), full_content.clone()));
                        let _ = finish_tx.send(conv_id);
                        break;
                    }
                }
                Err(e) => {
                    let _ = db_tx.send((msg_id.clone(), full_content.clone()));
                    let payload = ProviderErrorPayload::from(&e);
                    if let Err(emit_err) = app_clone.emit("chat-delta", json!({
                        "conversation_id": conv_id,
                        "message_id": msg_id,
                        "error": payload,
                    })) {
                        error!("Failed to emit error event: {}", emit_err);
                    }
                    let _ = finish_tx.send(conv_id);
                    break;
                }
            }
        }
    };

    tauri::async_runtime::spawn(Box::pin(stream_future));

    Ok(())
}

#[command]
pub fn cancel_chat(app: AppHandle, conversation_id: String) -> Result<(), String> {
    let chat_tasks: tauri::State<'_, ChatTasks> = app.state::<ChatTasks>();
    chat_tasks.cancel(&conversation_id);
    Ok(())
}

#[command]
pub fn get_conversation_paginated(
    app: AppHandle,
    id: String,
    limit: Option<i64>,
    before_timestamp: Option<i64>,
) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;

    let conversation = {
        let mut stmt = conn
            .prepare("SELECT id, title, provider_id, model_id, system_prompt, created_at, updated_at FROM conversations WHERE id = ?1")
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query(params![id]).map_err(|e| e.to_string())?;
        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            Some(ConversationRow {
                id: row.get(0).map_err(|e| e.to_string())?,
                title: row.get(1).map_err(|e| e.to_string())?,
                provider_id: row.get(2).map_err(|e| e.to_string())?,
                model_id: row.get(3).map_err(|e| e.to_string())?,
                system_prompt: row.get(4).map_err(|e| e.to_string())?,
                created_at: row.get(5).map_err(|e| e.to_string())?,
                updated_at: row.get(6).map_err(|e| e.to_string())?,
            })
        } else {
            None
        }
    };

    match conversation {
        Some(conv) => {
            let page_limit = limit.unwrap_or(50);
            let messages = conversations::get_messages_paginated(&conn, &id, page_limit, before_timestamp)?;
            let total_count = conversations::get_message_count(&conn, &id)?;
            Ok(json!({
                "conversation": conv,
                "messages": messages,
                "total_count": total_count,
            }))
        }
        None => Ok(json!(null)),
    }
}

#[command]
pub fn delete_message_persist(app: AppHandle, message_id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    conversations::delete_message(&conn, &message_id)
}

#[command]
pub fn search_conversations(app: AppHandle, query: String) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    let rows = conversations::search_conversations(&conn, &query)?;
    Ok(serde_json::to_value(&rows).map_err(|e| e.to_string())?)
}

#[command]
pub fn export_conversation(app: AppHandle, id: String, format: String) -> Result<String, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    match format.as_str() {
        "markdown" | "md" => conversations::export_conversation_markdown(&conn, &id),
        _ => conversations::export_conversation_markdown(&conn, &id),
    }
}