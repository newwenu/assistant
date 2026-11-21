use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 命令级失败的错误码：`provider` 域 13 个码 + 命令域新增 6 个码。
///
/// 这是错误码的**唯一来源**：流内事件（`ProviderErrorPayloadDto` /
/// `RetryNotificationDto`）与命令返回（`CommandErrorDto`）共用同一枚举，
/// 前端类型由 `npm run contract:gen` 生成，不再手写副本。
///
/// 前 13 个变体与已删除的 `ProviderErrorCodeDto` 逐字对齐：wire 上是
/// 字符串，改名即破坏前端既有分类表。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "PascalCase")]
pub enum CommandErrorCodeDto {
    ApiKeyMissing,
    AuthFailed,
    RateLimited,
    ModelNotFound,
    ContextOverflow,
    NetworkError,
    Timeout,
    ServerError,
    RequestRejected,
    ParseError,
    Cancelled,
    StreamError,
    InternalError,
    SessionBusy,
    ConversationNotFound,
    ProviderNotFound,
    Validation,
    StorageFailure,
    NoPendingInteraction,
}

impl CommandErrorCodeDto {
    /// 可重试语义由码推导，调用点不得自由传入。
    ///
    /// `Cancelled` 是用户主动停止；`Validation` / `*NotFound` /
    /// `NoPendingInteraction` / `StorageFailure` 重试必然再次失败。
    pub fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::RateLimited
                | Self::NetworkError
                | Self::Timeout
                | Self::ServerError
                | Self::StreamError
                | Self::SessionBusy
        )
    }
}

/// 命令级失败的统一结构。
///
/// 有意不承载 `details` / `httpStatus` / `provider`：命令级失败发生在
/// provider 调用之前，不存在 HTTP 元信息；诊断信息由前端本地注入。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CommandErrorDto {
    pub code: CommandErrorCodeDto,
    pub message: String,
    pub retryable: bool,
    #[serde(default)]
    pub retry_after_ms: Option<u64>,
}

impl CommandErrorDto {
    pub fn new(code: CommandErrorCodeDto, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: code.is_retryable(),
            retry_after_ms: None,
        }
    }

    /// 无法判别具体码时的显式降级入口。
    ///
    /// 刻意不提供 `From<String>`：隐式把任意字符串吞成 `InternalError`
    /// 会让本该分类的失败静默退化。
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(CommandErrorCodeDto::InternalError, message)
    }

    pub fn with_retry_after_ms(mut self, ms: Option<u64>) -> Self {
        self.retry_after_ms = ms;
        self
    }
}
