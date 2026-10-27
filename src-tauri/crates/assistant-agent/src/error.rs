#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("chat.attachments_not_allowed")]
    AttachmentsNotAllowed,
    #[error("chat.too_many_images")]
    TooManyImages { max: usize },
    #[error("chat.too_many_files")]
    TooManyFiles { max: usize },
    #[error("chat.image_too_large")]
    ImageTooLarge { max_mb: usize },
    #[error("chat.file_too_large")]
    FileTooLarge { max_mb: usize },
    #[error("{0}")]
    Other(String),
}

impl AgentError {
    pub(crate) fn other(msg: impl Into<String>) -> Self {
        AgentError::Other(msg.into())
    }
}

impl From<String> for AgentError {
    fn from(s: String) -> Self {
        AgentError::Other(s)
    }
}
