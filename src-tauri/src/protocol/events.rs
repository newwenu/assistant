pub mod event_name {
    pub const CHAT_MESSAGES_CHANGED: &str = "chat-messages-changed";
    pub const CHAT_DELTA: &str = "chat-delta";
    pub const CHAT_STREAM_START: &str = "chat-stream-start";
    pub const SESSION_STATE_CHANGED: &str = "session-state-changed";
    pub const CHAT_USER_MESSAGE_CONFIRMED: &str = "chat-user-message-confirmed";
    pub const AGENT_TOOL_CALL: &str = "agent-tool-call";
    pub const AGENT_NODE_EVENT: &str = "agent-node-event";
    pub const CONVERSATION_LIST_CHANGED: &str = "conversation-list-changed";
    pub const CONVERSATION_TERMINATED: &str = "conversation-terminated";
    pub const CONVERSATION_UNLOCKED: &str = "conversation-unlocked";
    pub const ORB_NEW_CONVERSATION: &str = "orb-new-conversation";
    pub const ORB_BLUR: &str = "orb-blur";
    pub const NAVIGATE: &str = "navigate";
    pub const THEME_CHANGED: &str = "theme-changed";
    pub const SETTING_PROVIDER_CHANGED: &str = "setting:provider-changed";
    pub const SHORTCUT_WARNING: &str = "shortcut-warning";
    pub const WINDOW_MODE_CHANGED: &str = "ui:window-mode-changed";
    pub const VAULT_LOCKED: &str = "vault:locked";
    pub const VAULT_UNLOCKED: &str = "vault:unlocked";
    pub const BIOMETRIC_ENROLL_STEP: &str = "vault:biometric-enroll-step";
    pub const SCREENSHOT_SESSION: &str = "screenshot-session";
    pub const SCREENSHOT_PREPARING: &str = "screenshot-preparing";
    pub const SCREENSHOT_ERROR: &str = "screenshot-error";
    /// 对账余量快照更新（I61 §3.11.4）：只通知「可重拉」，不携带数字。
    pub const STORAGE_INVENTORY_UPDATED: &str = "storage:inventory-updated";
    /// 能力状态变更（I75 §4.4 C4）：载荷 `CapabilityStateDto`，仅迁移处发。
    pub const STARTUP_CAPABILITY_CHANGED: &str = "startup:capability-changed";
}

pub fn emit_broadcast(app: &tauri::AppHandle, event: &str, payload: impl serde::Serialize + Clone) {
    use tauri::Emitter;
    if let Err(e) = app.emit(event, payload) {
        log::error!("Failed to emit broadcast '{}': {}", event, e);
    }
}

pub fn emit_orb_only(app: &tauri::AppHandle, event: &str, payload: impl serde::Serialize + Clone) {
    use tauri::Manager;
    if let Some(win) = app.get_webview_window(assistant_config::constant::WINDOW_ORB) {
        use tauri::Emitter;
        if let Err(e) = win.emit(event, payload) {
            log::error!("Failed to emit orb-only '{}': {}", event, e);
        }
    }
}
