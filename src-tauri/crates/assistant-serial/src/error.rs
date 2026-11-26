use std::io;

use assistant_protocol::{SerialDebugErrorCodeDto, SerialDebugErrorDto};

/// 领域错误：只携带前端可判别的错误码与调试日志用的描述
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SerialError {
    #[error("端口未打开")]
    NotOpen,
    #[error("端口已打开，请先关闭")]
    AlreadyOpen,
    #[error("端口不存在：{0}")]
    PortNotFound(String),
    #[error("无权限访问端口：{0}")]
    PermissionDenied(String),
    #[error("端口被占用：{0}")]
    Busy(String),
    #[error("串口 I/O 失败：{0}")]
    Io(String),
    #[error("平台不支持该操作：{0}")]
    Unsupported(String),
    #[error("参数非法：{0}")]
    InvalidInput(String),
}

impl SerialError {
    pub fn code(&self) -> SerialDebugErrorCodeDto {
        match self {
            SerialError::NotOpen => SerialDebugErrorCodeDto::NotOpen,
            SerialError::AlreadyOpen => SerialDebugErrorCodeDto::AlreadyOpen,
            SerialError::PortNotFound(_) => SerialDebugErrorCodeDto::PortNotFound,
            SerialError::PermissionDenied(_) => SerialDebugErrorCodeDto::PermissionDenied,
            SerialError::Busy(_) => SerialDebugErrorCodeDto::Busy,
            SerialError::Io(_) => SerialDebugErrorCodeDto::Io,
            SerialError::Unsupported(_) => SerialDebugErrorCodeDto::Unsupported,
            SerialError::InvalidInput(_) => SerialDebugErrorCodeDto::InvalidInput,
        }
    }

    pub fn to_dto(&self) -> SerialDebugErrorDto {
        SerialDebugErrorDto {
            code: self.code(),
            message: self.to_string(),
        }
    }
}

impl From<SerialError> for SerialDebugErrorDto {
    fn from(value: SerialError) -> Self {
        value.to_dto()
    }
}

impl From<io::Error> for SerialError {
    fn from(value: io::Error) -> Self {
        SerialError::Io(value.to_string())
    }
}
