// 参考: docs/store/I75-startup-layering-spec.md §6.2 — 窗口配置需求（纯数据，无 tauri 依赖）

use crate::constant::{
    MAIN_WINDOW_DEFAULT_HEIGHT, MAIN_WINDOW_DEFAULT_WIDTH, MAIN_WINDOW_MIN_HEIGHT,
    MAIN_WINDOW_MIN_WIDTH, ORB_COLLAPSED_WINDOW_SIZE, WINDOW_MAIN, WINDOW_ORB,
};

pub struct WindowSpec {
    pub label: &'static str,
    pub url: &'static str,
    pub title: &'static str,
    pub width: f64,
    pub height: f64,
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
    pub center: bool,
    pub resizable: bool,
    pub decorations: bool,
    pub transparent: bool,
    pub always_on_top: bool,
    pub skip_taskbar: bool,
    pub visible: bool,
    pub shadow: bool,
    pub maximizable: bool,
    pub minimizable: bool,
    pub accept_first_mouse: bool,
    pub zoom_hotkeys_enabled: bool,
    pub focus: bool,
    pub background_color: Option<&'static str>,
    pub confs: &'static [&'static str],
}

/// 表序即生成顺序：`git diff` 稳定，生成物不随机抖动。
pub static WINDOW_SPECS: &[WindowSpec] = &[
    WindowSpec {
        label: WINDOW_MAIN,
        url: "/",
        title: "Assistant",
        width: MAIN_WINDOW_DEFAULT_WIDTH,
        height: MAIN_WINDOW_DEFAULT_HEIGHT,
        min_width: Some(MAIN_WINDOW_MIN_WIDTH),
        min_height: Some(MAIN_WINDOW_MIN_HEIGHT),
        center: true,
        resizable: true,
        decorations: false,
        transparent: false,
        always_on_top: false,
        skip_taskbar: false,
        visible: false,
        shadow: true,
        maximizable: true,
        minimizable: true,
        accept_first_mouse: false,
        zoom_hotkeys_enabled: true,
        focus: true,
        background_color: None,
        confs: &["app"],
    },
    WindowSpec {
        label: WINDOW_ORB,
        url: "/",
        title: "Orb",
        width: ORB_COLLAPSED_WINDOW_SIZE as f64,
        height: ORB_COLLAPSED_WINDOW_SIZE as f64,
        min_width: None,
        min_height: None,
        center: false,
        resizable: false,
        decorations: false,
        transparent: true,
        always_on_top: true,
        skip_taskbar: true,
        visible: false,
        shadow: false,
        maximizable: false,
        minimizable: false,
        accept_first_mouse: true,
        zoom_hotkeys_enabled: false,
        focus: true,
        background_color: None,
        confs: &["app", "orb", "mascot"],
    },
];

/// 按 label 线性查找：表只有两项，不值得建索引。
pub fn spec(label: &str) -> Option<&'static WindowSpec> {
    WINDOW_SPECS.iter().find(|s| s.label == label)
}
