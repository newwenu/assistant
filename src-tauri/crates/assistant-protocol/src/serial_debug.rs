use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 端口类型（封闭集合）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum SerialPortKindDto {
    Usb,
    Bluetooth,
    Pci,
    Unknown,
}

/// 数据位（封闭集合）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum SerialDataBitsDto {
    Five,
    Six,
    Seven,
    Eight,
}

/// 校验位（封闭集合）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum SerialParityDto {
    None,
    Odd,
    Even,
    Mark,
    Space,
}

/// 停止位（封闭集合）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum SerialStopBitsDto {
    One,
    Two,
}

/// 流控（封闭集合）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum SerialFlowControlDto {
    None,
    Software,
    Hardware,
}

/// 串口错误码（封闭集合）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum SerialDebugErrorCodeDto {
    NotOpen,
    AlreadyOpen,
    PortNotFound,
    PermissionDenied,
    Busy,
    Io,
    Unsupported,
    InvalidInput,
}

/// 端口枚举结果。元数据取不到时为 `null`（Linux 无 libudev）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct SerialPortInfoDto {
    /// 对外裸名（Windows 为 `COM10`，不含 `\\.\` 前缀）
    pub port_name: String,
    pub kind: SerialPortKindDto,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub serial_number: Option<String>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
}

/// 打开会话入参。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SerialOpenInputDto {
    pub port_name: String,
    pub baud_rate: u32,
    pub data_bits: SerialDataBitsDto,
    pub parity: SerialParityDto,
    pub stop_bits: SerialStopBitsDto,
    pub flow_control: SerialFlowControlDto,
}

/// 已生效的串口参数（出参专用：全部必填，`config` 非空即参数齐全）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct SerialConfigDto {
    pub port_name: String,
    pub baud_rate: u32,
    pub data_bits: SerialDataBitsDto,
    pub parity: SerialParityDto,
    pub stop_bits: SerialStopBitsDto,
    pub flow_control: SerialFlowControlDto,
}

/// 信号线入参：只改显式给出的字段。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SerialSetSignalsInputDto {
    #[serde(default)]
    #[ts(optional = nullable)]
    pub dtr: Option<bool>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub rts: Option<bool>,
}

/// 会话快照：前端唯一真源。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct SerialSessionDto {
    /// 会话代际号；关闭态保留最后一次成功 `open` 的值（从未打开为 0）
    pub session_id: u64,
    pub open: bool,
    pub port_name: Option<String>,
    pub config: Option<SerialConfigDto>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub error: Option<SerialDebugErrorDto>,
}

/// 一次接收批事件：批首字节到达时刻的 epoch 毫秒。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct SerialRxBatchDto {
    pub session_id: u64,
    /// 会话内从 1 单调递增；跨会话不保证单调
    pub seq: u64,
    pub received_at_ms: u64,
    pub data_base64: String,
}

/// 命令失败载荷：`code` 决定 i18n 文案，`message` 只进调试日志。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct SerialDebugErrorDto {
    pub code: SerialDebugErrorCodeDto,
    pub message: String,
}
