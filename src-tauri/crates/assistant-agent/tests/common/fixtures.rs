//! `TurnRunner` / 循环执行器共用的输入源 fixture。
//!
//! 只保留一条 user 消息与一个 scripted provider；`models_json` 用于验证
//! `contextWindow` 相关的压缩与预算路径。

#![allow(dead_code)]

use std::sync::Arc;

use crate::common::rows::{conversation_row, message_row};
use assistant_agent::kernel::ports::test_doubles::FixtureSource;
use assistant_agent::kernel::ProviderCredential;
use assistant_providers::base::BaseProviderAdapter;
use assistant_storage::conversations::{ConversationDetail, ConversationRow};
use assistant_storage::providers::ProviderRow;

pub fn fixture_input(
    conversation_id: &str,
    adapter: Arc<dyn BaseProviderAdapter>,
) -> FixtureSource {
    fixture_input_with_models(conversation_id, adapter, "[]")
}

pub fn fixture_input_with_models(
    conversation_id: &str,
    adapter: Arc<dyn BaseProviderAdapter>,
    models_json: &str,
) -> FixtureSource {
    let conversation = ConversationRow {
        provider_id: Some("fixture-provider".to_string()),
        ..conversation_row(conversation_id, "fixture-model")
    };
    let messages = vec![message_row("user-1", conversation_id, "user", "hi")];
    let provider = ProviderCredential {
        row: ProviderRow {
            id: "fixture-provider".to_string(),
            name: "fixture".to_string(),
            type_: "openai".to_string(),
            base_url: "scripted://local".to_string(),
            api_key_encrypted: String::new(),
            models_json: models_json.to_string(),
            default_model: "fixture-model".to_string(),
            updated_at: 0,
        },
        api_key: "fixture-key".to_string(),
    };
    FixtureSource::new(
        ConversationDetail {
            conversation,
            messages,
        },
        provider,
        "fixture-attachments",
        adapter,
    )
}
