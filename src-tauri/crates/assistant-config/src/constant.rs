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

pub const COMPACTION_THRESHOLD_PERCENT: u8 = 80;
pub const MAX_SUMMARY_REQUEST_TOKENS: u32 = 4096;
pub const MAX_SUMMARY_OUTPUT_TOKENS: u32 = 2000;

/// 工具结果落盘文件的保留天数（I46 compact §2.9 D9）：`0` 表示完全禁用清理。
pub const RESULT_RETENTION_DAYS: u32 = 30;

/// 审计 JSONL 日文件的保留天数（I50 §5 G12）：`0` 表示完全禁用清理。
/// 只有「已补录进加密库且早于游标」的文件才会被删，故该值不构成数据丢失风险。
pub const AUDIT_RETENTION_DAYS: u32 = 90;
