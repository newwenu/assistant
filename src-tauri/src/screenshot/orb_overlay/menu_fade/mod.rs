// 菜单淡出排空：Win32 弹出菜单是独立顶层窗口（`#32768`），不受 Orb 隐藏影响；
// 抓屏读的是当前合成结果，关闭动画未播完就会被冻进底图。
// 平台分派：Windows 枚举菜单窗口并按合成帧排空，其它平台没有这类顶层窗口，全部空实现。

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub(crate) use windows::{dismiss_menus_now, hide_leftover_menus, menu_anchor, settle_menu_fade};

#[cfg(not(target_os = "windows"))]
mod fallback;

#[cfg(not(target_os = "windows"))]
pub(crate) use fallback::{dismiss_menus_now, hide_leftover_menus, menu_anchor, settle_menu_fade};
