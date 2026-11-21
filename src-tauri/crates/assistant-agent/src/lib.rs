pub mod agent;
// 共享层，非 chat 独占：`kernel::request` 仍引用。
pub mod chat_options;
pub mod compaction;
pub mod error;
pub mod kernel;
pub mod multimodal;
// 共享层，非 chat 独占：`kernel::request` 仍引用。
mod dto_bridge;
pub mod prompt_builder;
pub mod provider_config;
pub mod request_projection;
pub mod token_estimate;
