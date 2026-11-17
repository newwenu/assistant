use std::cmp::Reverse;
use std::collections::HashMap;

use assistant_providers::types::{ChatMessage, ContentBlock, FileAttachmentRef, MessageRole};
use assistant_storage::attachment_store::AttachmentStore;
use base64::engine::general_purpose;
use base64::Engine;

use crate::token_estimate::{is_pdf_name, PDF_NATIVE_TOKENS};

use super::blocks::{apply_blocks, blocks_of};
use super::text::{
    fallback_block, file_not_found_block, is_text_file, placeholder_block, placeholder_tokens,
    project_text_body, project_text_file,
};
use super::{ProjectionPurpose, MAX_UNKNOWN_WINDOW_FILE_TOKENS};

pub fn project_images_for_text_model(messages: &mut [ChatMessage]) {
    for msg in messages.iter_mut() {
        if msg.role != MessageRole::User {
            continue;
        }
        let Some(blocks) = blocks_of(msg) else {
            continue;
        };

        let projected: Vec<ContentBlock> = blocks
            .into_iter()
            .map(|block| match block {
                ContentBlock::Image { .. } => ContentBlock::Text {
                    text: text_only_image_text(),
                },
                other => other,
            })
            .collect();

        apply_blocks(msg, projected);
    }
}

/// 文件投影：在预算内把文件正文转成文本 / 保留原生 PDF / 降级占位。
///
/// `file_budget_tokens = None` 表示窗口未知（不可判定），按所有文件合计上限投影；
/// `Some(0)` 表示没有文件预算，全部降级。预算由调用方按「输入预算 - 非文件估算」算出。
pub async fn project_files(
    messages: &mut [ChatMessage],
    store: &AttachmentStore,
    support_pdf: bool,
    file_budget_tokens: Option<usize>,
    purpose: ProjectionPurpose,
) {
    let mut occurrences: Vec<(usize, usize, FileAttachmentRef)> = Vec::new();
    for (mi, msg) in messages.iter().enumerate() {
        if msg.role != MessageRole::User {
            continue;
        }
        let Some(blocks) = blocks_of(msg) else {
            continue;
        };
        for (bi, block) in blocks.iter().enumerate() {
            if let ContentBlock::File { attachment, .. } = block {
                occurrences.push((mi, bi, attachment.clone()));
            }
        }
    }
    if occurrences.is_empty() {
        return;
    }

    let mut remaining = match file_budget_tokens {
        Some(v) => v,
        None => {
            if purpose.warns() {
                log::warn!(
                    "[attachment-budget] 窗口未知，按文件总额上限 {} tokens 投影",
                    MAX_UNKNOWN_WINDOW_FILE_TOKENS
                );
            }
            MAX_UNKNOWN_WINDOW_FILE_TOKENS
        }
    };
    // 单文件上限：只有一个文件时可用全部预算；两个及以上时每个最多一半，
    // 避免最新一个文件吞掉全部配额后其余文件直接降级。
    let per_file_cap = remaining / occurrences.len().clamp(1, 2);

    // 新消息优先：消息索引从大到小；同一消息内保持原顺序。
    let mut ordered = occurrences.clone();
    ordered.sort_by_key(|(mi, bi, _)| (Reverse(*mi), *bi));

    let mut projected: HashMap<(usize, usize), ContentBlock> = HashMap::new();
    for (mi, bi, att) in ordered {
        let quota = remaining.min(per_file_cap);
        let (block, used) = project_one_file(&att, store, support_pdf, quota, purpose).await;
        remaining = remaining.saturating_sub(used);
        projected.insert((mi, bi), block);
    }

    for (mi, bi, _) in occurrences {
        let Some(mut blocks) = blocks_of(&messages[mi]) else {
            continue;
        };
        if let Some(block) = projected.remove(&(mi, bi)) {
            if bi < blocks.len() {
                blocks[bi] = block;
            }
        }
        apply_blocks(&mut messages[mi], blocks);
    }
}

async fn project_one_file(
    att: &FileAttachmentRef,
    store: &AttachmentStore,
    support_pdf: bool,
    quota: usize,
    purpose: ProjectionPurpose,
) -> (ContentBlock, usize) {
    if quota == 0 {
        return (placeholder_block(att), placeholder_tokens(att));
    }

    if is_pdf_name(&att.name) {
        // 原生优先：预算足够时保留 PDF 原样发送；不足时回退侧车文本。
        if support_pdf && quota >= PDF_NATIVE_TOKENS {
            match store.read(&att.attachment_id).await {
                Ok(data) => {
                    return (
                        ContentBlock::File {
                            attachment: att.clone(),
                            cached_data: Some(general_purpose::STANDARD.encode(&data)),
                            file_path: None,
                        },
                        PDF_NATIVE_TOKENS,
                    );
                }
                Err(e) => {
                    log::warn!("Failed to rehydrate file {}: {}", att.attachment_id, e);
                    return (file_not_found_block(), 0);
                }
            }
        }

        if let Some(text) = crate::pdf_text::extract_or_load(store, att).await {
            return project_text_body(att, &text, quota);
        }

        if purpose.warns() {
            log::warn!(
                "[attachment-budget] PDF 无可用文本层，降级为占位文本: {}",
                att.name
            );
        }
        return (placeholder_block(att), placeholder_tokens(att));
    }

    if is_text_file(&att.name) {
        return match store.read(&att.attachment_id).await {
            Ok(data) => match project_text_file(att, &data, quota) {
                Ok((block, used)) => (block, used),
                Err(reason) => (fallback_block(att, &reason), placeholder_tokens(att)),
            },
            Err(e) => {
                log::warn!("Failed to read file {}: {}", att.attachment_id, e);
                (file_not_found_block(), 0)
            }
        };
    }

    (placeholder_block(att), placeholder_tokens(att))
}

pub fn text_only_image_text() -> String {
    "[Image omitted — model does not support images]".to_string()
}

pub fn offloaded_image_text(attachment_id: &str) -> String {
    format!("[Image offloaded (ID: {})]", attachment_id)
}
