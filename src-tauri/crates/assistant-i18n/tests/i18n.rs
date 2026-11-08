use assistant_i18n::{current_locale, init_global, set_locale, t, t_with_args, text};
use std::collections::HashMap;

#[test]
fn i18n_full_suite() {
    init_global("zh-CN");
    assert_eq!(t("tray.new_chat"), "新建对话");
    assert_eq!(t("tray.quit"), "退出");
    assert_eq!(t("chat.empty_content"), "消息内容不能为空");
    assert_eq!(t("app.name"), "Assistant");
    assert_eq!(t("nonexistent.key"), "nonexistent.key");

    let mut args = HashMap::new();
    args.insert("label", "test-window");
    assert_eq!(
        t_with_args("window.unknown_label", &args),
        "未知窗口标签: test-window"
    );
    assert_eq!(current_locale(), "zh-CN");

    assert_eq!(t("agent_mode.builtin_cannot_delete"), "内置模式不可删除");
    assert_eq!(
        t("agent_mode.not_builtin_cannot_reset"),
        "不是内置模式，无法恢复默认"
    );
    assert_eq!(t("agent_mode.builtin.default.name"), "默认");
    assert_eq!(t("agent_mode.builtin.agent.name"), "智能体");
    assert_eq!(t("agent_mode.builtin.translate.name"), "翻译助手");
    assert_eq!(
        t("chat.compaction.summary_truncated"),
        "[摘要已截断，完整信息请通过 search_history 检索]"
    );

    set_locale("en-US");
    assert_eq!(t("tray.new_chat"), "New Chat");
    assert_eq!(t("tray.quit"), "Quit");
    assert_eq!(t("chat.empty_content"), "Message content cannot be empty");
    assert_eq!(
        t_with_args("window.unknown_label", &args),
        "Unknown window label: test-window"
    );
    assert_eq!(current_locale(), "en-US");
    assert_eq!(
        t("agent_mode.builtin_cannot_delete"),
        "Built-in modes cannot be deleted"
    );
    assert_eq!(
        t("agent_mode.not_builtin_cannot_reset"),
        "Not a built-in mode; cannot reset to default"
    );
    assert_eq!(t("agent_mode.builtin.default.name"), "Default");
    assert_eq!(t("agent_mode.builtin.agent.name"), "Agent");
    assert_eq!(t("agent_mode.builtin.translate.name"), "Translator");
    assert_eq!(
        t("chat.compaction.summary_truncated"),
        "[Summary truncated. Use search_history for full details]"
    );

    set_locale("zh-CN");
    assert_eq!(t("tray.new_chat"), "新建对话");

    init_global("fr-FR");
    assert_eq!(t("tray.new_chat"), "新建对话");

    assert_eq!(text("zh-CN", "tray.new_chat"), "新建对话");
    assert_eq!(text("en-US", "tray.new_chat"), "New Chat");
    assert_eq!(text("zh-CN", "agent_mode.builtin.default.name"), "默认");
    assert_eq!(text("en-US", "agent_mode.builtin.default.name"), "Default");
}
