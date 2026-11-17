pub const APP_IDENTIFIER: &str = "com.assistant";

pub const WINDOW_MAIN: &str = "main";
pub const WINDOW_ORB: &str = "orb";

pub const ORB_COLLAPSED_SIZE: u32 = 84;
pub const ORB_EXPANDED_WIDTH: u32 = 420;
pub const ORB_EXPANDED_HEIGHT: u32 = 600;
pub const ORB_CHROME_INSET: u32 = 2;
pub const ORB_COLLAPSED_WINDOW_SIZE: u32 = ORB_COLLAPSED_SIZE + 2 * ORB_CHROME_INSET;
pub const ORB_EXPANDED_WINDOW_WIDTH: u32 = ORB_EXPANDED_WIDTH + 2 * ORB_CHROME_INSET;
pub const ORB_EXPANDED_WINDOW_HEIGHT: u32 = ORB_EXPANDED_HEIGHT + 2 * ORB_CHROME_INSET;

pub const MAIN_WINDOW_MIN_WIDTH: f64 = 300.0;
pub const MAIN_WINDOW_MIN_HEIGHT: f64 = 200.0;
pub const MAIN_WINDOW_DEFAULT_WIDTH: f64 = 960.0;
pub const MAIN_WINDOW_DEFAULT_HEIGHT: f64 = 640.0;

pub const MAIN_WINDOW_MINI_WIDTH: f64 = 280.0;
pub const MAIN_WINDOW_MINI_HEIGHT: f64 = 420.0;
pub const MAIN_WINDOW_MINI_MIN_WIDTH: f64 = 240.0;
pub const MAIN_WINDOW_MINI_MIN_HEIGHT: f64 = 320.0;

pub const WINDOW_EDGE_BUFFER: u32 = 4;

pub const ORB_HITBOX_SIZE: u32 = ORB_COLLAPSED_WINDOW_SIZE;
pub const ORB_SNAP_BORDER_X: i32 = 24;
pub const ORB_SNAP_ANIMATION_MS: u64 = 250;

/// 隐藏覆盖层宿主 `orb` 后的兜底等待上限（毫秒）；正常路径靠可见性校验 + DwmFlush 提前返回。
pub const SCREENSHOT_HIDE_SETTLE_MS: u64 = 150;
/// 会话事件发出后、显示覆盖层前的兜底等待上限（毫秒）；前端以 `img.decode()` 完成为准。
pub const SCREENSHOT_OVERLAY_SETTLE_MS: u64 = 100;
/// 弹出菜单关闭淡出动画的帧数：菜单窗口在 `TrackPopupMenu` 返回时已被销毁，无法再
/// `ShowWindow`，只能从点击那一刻起把动画帧排空，否则淡出帧会被冻进底图。
/// 实测（Win11 默认动画）淡出是 **19 个 DWM 合成帧的线性过渡**，与菜单项数量无关；
/// 因此按帧而非毫秒计：60Hz 下约 317ms、144Hz 下约 132ms。固定毫秒常量在高刷屏上白等、
/// 在低刷屏上漏帧，是这套逻辑此前「时长对不上」的根因。这里取 19 帧 + 1 帧余量。
pub const SCREENSHOT_MENU_FADE_FRAMES: u32 = 20;
/// 排空淡出帧的兜底上限（毫秒）：远程桌面、窗口被遮挡等场景下 `DwmFlush` 可能提前返回，
/// 用时间上限防止帧数循环退化成忙等（30Hz 下 20 帧约 667ms，故上限取 800）。
pub const SCREENSHOT_MENU_FADE_CAP_MS: u64 = 800;
/// 无覆盖层宿主时的孤儿会话存活时间。
pub const SCREENSHOT_SESSION_TTL_MS: u64 = 10 * 60 * 1000;
/// 与 assistant-agent / 前端一致的图片单张上限。
pub const SCREENSHOT_MAX_BYTES: u64 = 8 * 1024 * 1024;
/// 底图预览的长边上限（CSS px）；只影响底图清晰度，不影响裁剪产物。
pub const SCREENSHOT_PREVIEW_LONG_EDGE: u32 = 2560;
/// 底图预览的 JPEG 质量。
pub const SCREENSHOT_PREVIEW_JPEG_QUALITY: u8 = 80;

pub const COMPACTION_THRESHOLD_PERCENT: u8 = 80;
pub const MAX_SUMMARY_REQUEST_TOKENS: u32 = 4096;
pub const MAX_SUMMARY_OUTPUT_TOKENS: u32 = 2000;

/// 工具结果落盘文件的保留天数（I46 compact §2.9 D9）：`0` 表示完全禁用清理。
pub const RESULT_RETENTION_DAYS: u32 = 30;

/// 审计 JSONL 日文件的保留天数（I50 §5 G12）：`0` 表示完全禁用清理。
/// 只有「已补录进加密库且早于游标」的文件才会被删，故该值不构成数据丢失风险。
pub const AUDIT_RETENTION_DAYS: u32 = 90;
