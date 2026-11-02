//! PDF 文本层抽取 + 内容寻址侧车缓存。
//!
//! 抽取在 `spawn_blocking` 中执行：PDF 是不可信输入，解析器 panic 由 JoinError 隔离；
//! 同一 SHA 并发请求只抽取一次（进程内按附件 id 加锁），结果落到 `AttachmentStore`
//! 的 `derived/` 子目录，算法变更时通过侧车文件名版本号失效。

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use assistant_providers::types::FileAttachmentRef;
use assistant_storage::attachment_store::AttachmentStore;
use tokio::sync::Mutex as AsyncMutex;

static EXTRACT_LOCKS: LazyLock<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn lock_for(id: &str) -> Arc<AsyncMutex<()>> {
    let mut map = EXTRACT_LOCKS.lock().expect("pdf extract lock map poisoned");
    map.entry(id.to_string())
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

fn release(id: &str, lock: &Arc<AsyncMutex<()>>) {
    // 只有 map 与当前调用持有 Arc 时才清理，避免把等待者的锁移除。
    if Arc::strong_count(lock) > 2 {
        return;
    }
    let Ok(mut map) = EXTRACT_LOCKS.lock() else {
        return;
    };
    if Arc::strong_count(lock) <= 2 {
        map.remove(id);
    }
}

/// 读取或按需抽取 PDF 文本层。
///
/// 返回 `None` 表示不可用（扫描件、加密 PDF、解析 panic 等），调用方必须回退，
/// 不得阻断发送。
pub async fn extract_or_load(store: &AttachmentStore, att: &FileAttachmentRef) -> Option<String> {
    if let Some(text) = store.read_derived_text(&att.attachment_id).await {
        return Some(text);
    }

    let lock = lock_for(&att.attachment_id);
    let _guard = lock.lock().await;

    // 双重检查：等锁期间可能已被其他请求抽取完成。
    if let Some(text) = store.read_derived_text(&att.attachment_id).await {
        release(&att.attachment_id, &lock);
        return Some(text);
    }

    let path = store.file_path(&att.attachment_id);
    let extracted = tokio::task::spawn_blocking(move || pdf_extract::extract_text(&path)).await;

    let text = match extracted {
        Ok(Ok(text)) if !text.trim().is_empty() => text,
        Ok(Ok(_)) => {
            log::warn!(
                "[pdf-extract] PDF 无文本层（扫描件或空文档），降级占位: {}",
                att.name
            );
            release(&att.attachment_id, &lock);
            return None;
        }
        Ok(Err(e)) => {
            log::warn!("[pdf-extract] PDF 文本抽取失败: {} ({})", att.name, e);
            release(&att.attachment_id, &lock);
            return None;
        }
        Err(e) => {
            log::warn!("[pdf-extract] PDF 解析器异常退出: {} ({})", att.name, e);
            release(&att.attachment_id, &lock);
            return None;
        }
    };

    if let Err(e) = store.write_derived_text(&att.attachment_id, &text).await {
        log::warn!("[pdf-extract] 侧车写入失败: {} ({})", att.name, e);
    }

    release(&att.attachment_id, &lock);
    Some(text)
}
