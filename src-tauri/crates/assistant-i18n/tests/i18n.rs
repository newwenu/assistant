use assistant_i18n::{current_locale, init_global, set_locale, t, t_with_args};
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

    set_locale("en-US");
    assert_eq!(t("tray.new_chat"), "New Chat");
    assert_eq!(t("tray.quit"), "Quit");
    assert_eq!(t("chat.empty_content"), "Message content cannot be empty");
    assert_eq!(
        t_with_args("window.unknown_label", &args),
        "Unknown window label: test-window"
    );
    assert_eq!(current_locale(), "en-US");

    set_locale("zh-CN");
    assert_eq!(t("tray.new_chat"), "新建对话");

    init_global("fr-FR");
    assert_eq!(t("tray.new_chat"), "新建对话");
}
