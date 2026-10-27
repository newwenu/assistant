#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("conversation.not_found")]
    ConversationNotFound,
    #[error("agent_mode.builtin_cannot_delete")]
    BuiltinModeCannotDelete,
    #[error("agent_mode.not_builtin_cannot_reset")]
    NotBuiltinModeCannotReset,
    #[error("{0}")]
    Other(String),
}

impl StorageError {
    pub(crate) fn other(msg: impl Into<String>) -> Self {
        StorageError::Other(msg.into())
    }
}

impl From<String> for StorageError {
    fn from(s: String) -> Self {
        StorageError::Other(s)
    }
}