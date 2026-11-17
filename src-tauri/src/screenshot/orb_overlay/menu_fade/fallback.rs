// 非 Windows 平台：没有 `#32768` 顶层菜单窗口，清理与排空都是空实现。

use std::time::Instant;

pub(crate) fn dismiss_menus_now() {}

pub(crate) fn menu_anchor() -> Option<Instant> {
    None
}

pub(crate) fn hide_leftover_menus() -> Result<(), String> {
    Ok(())
}

pub(crate) fn settle_menu_fade() -> Result<(), String> {
    Ok(())
}
