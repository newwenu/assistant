use assistant_agent::kernel::request;
use assistant_agent::multimodal::{self, MultimodalConfig, ProjectionPurpose};
use assistant_agent::prompt_builder::PromptFacts;
use assistant_agent::request_projection::{estimate_total_tokens, offload_if_over_budget};
use assistant_agent::token_estimate::estimate_text_tokens;
use assistant_providers::types::{
    ChatMessage, ContentBlock, FileAttachmentRef, ImageAttachmentRef, MessageRole,
};
use assistant_storage::attachment_store::AttachmentStore;
use assistant_storage::conversations::{ConversationDetail, ConversationRow, MessageRow};
use assistant_storage::providers::ProviderRow;

fn store() -> (tempfile::TempDir, AttachmentStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = AttachmentStore::new(dir.path().to_path_buf());
    (dir, store)
}

fn message(role: MessageRole, blocks: Vec<ContentBlock>) -> ChatMessage {
    ChatMessage {
        role,
        content: serde_json::to_string(&blocks).expect("serialize"),
        tool_calls: None,
        tool_call_id: None,
        name: None,
        is_error: false,
        thinking_signature: None,
        content_blocks: Some(blocks),
    }
}

fn text(content: &str) -> ContentBlock {
    ContentBlock::Text {
        text: content.to_string(),
    }
}

fn file_block(id: &str, name: &str, bytes: u64) -> ContentBlock {
    ContentBlock::File {
        attachment: FileAttachmentRef {
            attachment_id: id.to_string(),
            name: name.to_string(),
            bytes,
        },
        cached_data: None,
        file_path: None,
    }
}

fn image_block(id: &str, width: u32, height: u32) -> ContentBlock {
    ContentBlock::Image {
        attachment: ImageAttachmentRef {
            attachment_id: id.to_string(),
            media_type: Some("image/png".to_string()),
            bytes: Some(1),
            width: Some(width),
            height: Some(height),
            name: Some(format!("{}.png", id)),
        },
        cached_data: Some("AAAA".to_string()),
        file_path: None,
    }
}

fn block_text(block: &ContentBlock) -> String {
    match block {
        ContentBlock::Text { text } => text.clone(),
        _ => String::new(),
    }
}

#[tokio::test]
async fn unsupported_pdf_degrades_to_handle_text() {
    let (_dir, store) = store();
    let saved = store
        .save_attachment(b"%PDF-1.4 fake", "application/pdf")
        .await
        .expect("save");
    let mut messages = vec![message(
        MessageRole::User,
        vec![file_block(&saved.id, "doc.pdf", saved.bytes)],
    )];

    multimodal::project_files(
        &mut messages,
        &store,
        false,
        Some(100_000),
        ProjectionPurpose::Send,
    )
    .await;

    let blocks = messages[0].content_blocks.as_ref().expect("blocks");
    assert!(
        matches!(&blocks[0], ContentBlock::Text { text } if text.starts_with("[file: doc.pdf"))
    );
}

#[tokio::test]
async fn native_pdf_requires_budget() {
    let (_dir, store) = store();
    let saved = store
        .save_attachment(b"%PDF-1.4 fake", "application/pdf")
        .await
        .expect("save");

    let mut low = vec![message(
        MessageRole::User,
        vec![file_block(&saved.id, "doc.pdf", saved.bytes)],
    )];
    multimodal::project_files(
        &mut low,
        &store,
        true,
        Some(10_000),
        ProjectionPurpose::Send,
    )
    .await;
    assert!(matches!(
        &low[0].content_blocks.as_ref().unwrap()[0],
        ContentBlock::Text { .. }
    ));

    let mut high = vec![message(
        MessageRole::User,
        vec![file_block(&saved.id, "doc.pdf", saved.bytes)],
    )];
    multimodal::project_files(
        &mut high,
        &store,
        true,
        Some(100_000),
        ProjectionPurpose::Send,
    )
    .await;
    assert!(matches!(
        &high[0].content_blocks.as_ref().unwrap()[0],
        ContentBlock::File {
            cached_data: Some(_),
            ..
        }
    ));
}

#[tokio::test]
async fn unknown_window_uses_total_cap_not_per_file_cap() {
    let (_dir, store) = store();
    let mut messages = Vec::new();
    for i in 0..3 {
        let content = format!("%PDF-1.4 fake {}", i);
        let saved = store
            .save_attachment(content.as_bytes(), "application/pdf")
            .await
            .expect("save");
        messages.push(message(
            MessageRole::User,
            vec![file_block(&saved.id, &format!("doc{}.pdf", i), saved.bytes)],
        ));
    }

    multimodal::project_files(
        &mut messages,
        &store,
        true,
        None,
        ProjectionPurpose::Estimate,
    )
    .await;

    for msg in &messages {
        assert!(matches!(
            &msg.content_blocks.as_ref().unwrap()[0],
            ContentBlock::Text { .. }
        ));
    }
}

#[tokio::test]
async fn zero_budget_degrades_every_file_including_native_pdf() {
    let (_dir, store) = store();
    let saved = store
        .save_attachment(b"%PDF-1.4 fake", "application/pdf")
        .await
        .expect("save");
    let mut messages = vec![message(
        MessageRole::User,
        vec![file_block(&saved.id, "doc.pdf", saved.bytes)],
    )];

    multimodal::project_files(
        &mut messages,
        &store,
        true,
        Some(0),
        ProjectionPurpose::Send,
    )
    .await;

    assert!(matches!(
        &messages[0].content_blocks.as_ref().unwrap()[0],
        ContentBlock::Text { .. }
    ));
}

#[tokio::test]
async fn multi_file_total_stays_within_budget() {
    let (_dir, store) = store();
    let mut blocks = Vec::new();
    for i in 0..4 {
        let content = format!("{}{}", i, "x".repeat(512 * 1024));
        let saved = store
            .save_attachment(content.as_bytes(), "text/plain")
            .await
            .expect("save");
        blocks.push(file_block(&saved.id, &format!("f{}.txt", i), saved.bytes));
    }
    let mut messages = vec![message(MessageRole::User, blocks)];

    multimodal::project_files(
        &mut messages,
        &store,
        false,
        Some(64_000),
        ProjectionPurpose::Send,
    )
    .await;

    let blocks = messages[0].content_blocks.as_ref().unwrap();
    assert!(blocks
        .iter()
        .all(|b| matches!(b, ContentBlock::Text { .. })));
    // 允许占位文本与标注的少量开销
    assert!(estimate_total_tokens(&messages) <= 70_000);
}

#[tokio::test]
async fn newest_files_are_allocated_first() {
    let (_dir, store) = store();
    let mut messages = Vec::new();
    for i in 0..3 {
        let content = "a".repeat(16 * 1024);
        let saved = store
            .save_attachment(content.as_bytes(), "text/plain")
            .await
            .expect("save");
        messages.push(message(
            MessageRole::User,
            vec![file_block(&saved.id, &format!("f{}.txt", i), saved.bytes)],
        ));
    }

    multimodal::project_files(
        &mut messages,
        &store,
        false,
        Some(6_000),
        ProjectionPurpose::Send,
    )
    .await;

    let oldest = block_text(&messages[0].content_blocks.as_ref().unwrap()[0]);
    let newest = block_text(&messages[2].content_blocks.as_ref().unwrap()[0]);
    assert!(!oldest.contains("truncated"));
    assert!(oldest.starts_with("[file: f0.txt"));
    assert!(newest.contains("truncated"));
}

#[tokio::test]
async fn pdf_sidecar_is_reused_for_non_native_models() {
    let (_dir, store) = store();
    let saved = store
        .save_attachment(b"%PDF-1.4 fake", "application/pdf")
        .await
        .expect("save");
    store
        .write_derived_text(&saved.id, "sidecar text from pdf")
        .await
        .expect("write sidecar");

    let mut messages = vec![message(
        MessageRole::User,
        vec![file_block(&saved.id, "doc.pdf", saved.bytes)],
    )];
    multimodal::project_files(
        &mut messages,
        &store,
        false,
        Some(100_000),
        ProjectionPurpose::Send,
    )
    .await;

    let projected = block_text(&messages[0].content_blocks.as_ref().unwrap()[0]);
    assert!(projected.contains("sidecar text from pdf"));
}

#[tokio::test]
async fn sidecar_namespace_and_lifecycle() {
    let (_dir, store) = store();
    let saved = store
        .save_attachment(b"%PDF-1.4 fake", "application/pdf")
        .await
        .expect("save");

    assert!(store.read_derived_text(&saved.id).await.is_none());
    store
        .write_derived_text(&saved.id, "hello")
        .await
        .expect("write");
    assert_eq!(
        store.read_derived_text(&saved.id).await.as_deref(),
        Some("hello")
    );

    // derived/ 不属于附件，list_all 只返回根目录文件
    let entries = store.list_all().await.expect("list");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, saved.id);

    store.delete(&saved.id).await.expect("delete");
    assert!(store.read_derived_text(&saved.id).await.is_none());
}

#[tokio::test]
async fn offload_removes_oldest_images_first() {
    let mut messages = vec![
        message(MessageRole::User, vec![image_block("old", 768, 768)]),
        message(MessageRole::User, vec![image_block("new", 768, 768)]),
    ];
    offload_if_over_budget(&mut messages, Some(1_000), ProjectionPurpose::Send);

    assert!(!messages[0]
        .content_blocks
        .as_ref()
        .unwrap()
        .iter()
        .any(|b| matches!(b, ContentBlock::Image { .. })));
    assert!(messages[1]
        .content_blocks
        .as_ref()
        .unwrap()
        .iter()
        .any(|b| matches!(b, ContentBlock::Image { .. })));
}

#[tokio::test]
async fn offload_unknown_budget_leaves_messages_unchanged() {
    let mut messages = vec![message(
        MessageRole::User,
        vec![image_block("img", 768, 768)],
    )];
    offload_if_over_budget(&mut messages, None, ProjectionPurpose::Estimate);
    assert!(matches!(
        &messages[0].content_blocks.as_ref().unwrap()[0],
        ContentBlock::Image { .. }
    ));
}

#[tokio::test]
async fn offload_zero_budget_degrades_images() {
    let mut messages = vec![message(
        MessageRole::User,
        vec![image_block("img", 768, 768)],
    )];
    offload_if_over_budget(&mut messages, Some(0), ProjectionPurpose::Send);
    assert!(matches!(
        &messages[0].content_blocks.as_ref().unwrap()[0],
        ContentBlock::Text { .. }
    ));
}

#[tokio::test]
async fn offload_never_degrades_last_user_instruction() {
    let last_instruction = "a".repeat(4_000);
    let mut messages = vec![
        message(MessageRole::User, vec![text(&"b".repeat(4_000))]),
        message(MessageRole::User, vec![text(&last_instruction)]),
    ];
    offload_if_over_budget(&mut messages, Some(300), ProjectionPurpose::Send);

    let last = block_text(&messages[1].content_blocks.as_ref().unwrap()[0]);
    assert_eq!(last, last_instruction);
    let oldest = block_text(&messages[0].content_blocks.as_ref().unwrap()[0]);
    assert!(estimate_text_tokens(&oldest) <= 250);
}

#[tokio::test]
async fn offload_truncates_tool_content_and_keeps_call_id() {
    let mut messages = vec![
        message(MessageRole::User, vec![text("hi")]),
        ChatMessage {
            role: MessageRole::Tool,
            content: "z".repeat(8_000),
            tool_calls: None,
            tool_call_id: Some("call_1".to_string()),
            name: None,
            is_error: false,
            thinking_signature: None,
            content_blocks: None,
        },
        message(MessageRole::User, vec![text("continue")]),
    ];
    let before = messages[1].content.len();
    offload_if_over_budget(&mut messages, Some(500), ProjectionPurpose::Send);

    assert!(messages[1].content.len() < before);
    assert_eq!(messages[1].tool_call_id.as_deref(), Some("call_1"));
}

#[tokio::test]
async fn offload_does_not_touch_projected_file_body() {
    let file_body = format!("[file: a.txt]\n{}", "x".repeat(4_000));
    let mut messages = vec![
        message(MessageRole::User, vec![text(&file_body)]),
        message(MessageRole::User, vec![text("continue")]),
    ];
    offload_if_over_budget(&mut messages, Some(300), ProjectionPurpose::Send);

    let oldest = block_text(&messages[0].content_blocks.as_ref().unwrap()[0]);
    assert_eq!(oldest, file_body);
}

#[tokio::test]
async fn admission_rejects_untrusted_channel_attachments() {
    let (_dir, store) = store();
    let content = r#"[{"type":"image","attachment":{"attachmentId":"","name":"x.png"},"cached_data":"AAAA"}]"#;
    let result = multimodal::admit_attachments(
        content,
        &store,
        &MultimodalConfig::untrusted_channel_input(),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn admission_rejects_file_path_channel() {
    let (dir, store) = store();
    let path = dir.path().join("x.png");
    std::fs::write(&path, b"not an image").expect("write");
    let config = MultimodalConfig::trusted_channel_input();

    let traversal = r#"[{"type":"image","attachment":{"attachmentId":"","name":"x.png"},"file_path":"../x.png"}]"#;
    let error = multimodal::admit_attachments(traversal, &store, &config)
        .await
        .expect_err("file_path 通道必须整体拒绝");
    assert!(error.to_string().contains("file_path"));

    let absolute = format!(
        r#"[{{"type":"image","attachment":{{"attachmentId":"","name":"x.png"}},"file_path":{}}}]"#,
        serde_json::to_string(&path.to_string_lossy()).unwrap()
    );
    assert!(multimodal::admit_attachments(&absolute, &store, &config)
        .await
        .is_err());

    let file = format!(
        r#"[{{"type":"file","attachment":{{"attachmentId":"","name":"a.txt"}},"file_path":{}}}]"#,
        serde_json::to_string(&path.to_string_lossy()).unwrap()
    );
    assert!(multimodal::admit_attachments(&file, &store, &config)
        .await
        .is_err());
    assert!(
        !std::fs::read_dir(dir.path())
            .expect("read dir")
            .any(|entry| entry
                .map(|e| e.file_name().to_string_lossy().starts_with("derived"))
                .unwrap_or(false)),
        "拒绝路径不得产生任何落盘产物"
    );
}

#[tokio::test]
async fn admission_rejects_forged_mime() {
    let (_dir, store) = store();
    let content = r#"[{"type":"image","attachment":{"attachmentId":"","name":"fake.png"},"cached_data":"AAAA"}]"#;
    let result =
        multimodal::admit_attachments(content, &store, &MultimodalConfig::trusted_channel_input())
            .await;
    assert!(result.is_err());
}

fn provider_row(model: &str, models_json: &str) -> ProviderRow {
    ProviderRow {
        id: "p1".to_string(),
        name: "Test".to_string(),
        type_: "openai".to_string(),
        base_url: "https://example.com/v1".to_string(),
        api_key_encrypted: String::new(),
        models_json: models_json.to_string(),
        default_model: model.to_string(),
        updated_at: 0,
    }
}

fn detail_with_file(model: &str, file_content: &str) -> ConversationDetail {
    let file = ContentBlock::File {
        attachment: FileAttachmentRef {
            attachment_id: "x.txt".to_string(),
            name: "x.txt".to_string(),
            bytes: file_content.len() as u64,
        },
        cached_data: None,
        file_path: None,
    };
    ConversationDetail {
        conversation: ConversationRow {
            id: "c1".to_string(),
            title: "t".to_string(),
            title_source: "user".to_string(),
            provider_id: Some("p1".to_string()),
            model_id: model.to_string(),
            system_prompt: Some("sys".to_string()),
            is_pinned: false,
            sort_order: 0,
            generation: 0,
            created_at: 0,
            updated_at: 0,
            is_full: false,
            active_branch_id: None,
            mode_id: None,
            tool_config_json: None,
            workspace_root: String::new(),
        },
        messages: vec![MessageRow {
            id: "m1".to_string(),
            conversation_id: "c1".to_string(),
            role: "user".to_string(),
            content: serde_json::to_string(&vec![file, text("summarize")]).unwrap(),
            tokens: None,
            timestamp: 0,
            parent_id: None,
            is_pinned: false,
            reasoning: None,
            tool_calls_json: None,
            sibling_index: 0,
            provider_id: Some("p1".to_string()),
            model_id: Some(model.to_string()),
            tool_call_id: None,
            is_error: false,
            status: "completed".to_string(),
            mode_id: None,
        }],
    }
}

#[tokio::test]
async fn display_and_send_use_the_same_budget() {
    let (_dir, store) = store();
    let content = "a".repeat(40_000);
    let saved = store
        .save_attachment(content.as_bytes(), "text/plain")
        .await
        .expect("save");
    let models_json = r#"[{"id":"m1","contextWindow":8192}]"#;
    let row = provider_row("m1", models_json);
    let mut detail = detail_with_file("m1", "a");
    // 指向真实落盘附件
    detail.messages[0].content = serde_json::to_string(&vec![
        file_block(&saved.id, "x.txt", saved.bytes),
        text("summarize"),
    ])
    .unwrap();

    let send_budget = 8192usize - assistant_agent::chat_options::DEFAULT_MAX_TOKENS as usize;
    let sent = request::project_chat_messages(
        &detail,
        "sys",
        &row,
        "m1",
        &store,
        Some(send_budget),
        ProjectionPurpose::Send,
        None,
    )
    .await;
    let shown =
        request::estimate_branch_tokens(&detail, "sys", &PromptFacts::default(), &row, &store)
            .await;
    assert_eq!(estimate_total_tokens(&sent), shown);
}
