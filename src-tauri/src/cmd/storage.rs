use tauri::{command, AppHandle, Manager};

use crate::protocol::dto::{
    AttachmentDeleteResultDto, AttachmentSummary, ConversationAttachmentSummary,
    StorageBreakdownDto,
};
use assistant_storage::attachment_store::{self, AttachmentStore};

#[command]
pub async fn get_storage_breakdown(app: AppHandle) -> Result<StorageBreakdownDto, String> {
    let attachment_store = app.state::<AttachmentStore>();
    let all_entries = attachment_store
        .list_all()
        .await
        .map_err(|e| e.to_string())?;

    let referenced_ids: std::collections::HashSet<String>;
    let conv_rows: Vec<(String, String)>;
    {
        let state = app.state::<crate::setup::DbState>();
        let conn = super::get_db_conn(&state)?;

        referenced_ids = attachment_store::get_all_referenced_attachment_ids(&conn)?
            .into_iter()
            .collect();

        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT ar.conversation_id, ar.attachment_id \
                 FROM attachment_refs ar \
                 ORDER BY ar.conversation_id",
            )
            .map_err(|e| e.to_string())?;

        conv_rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
    }

    let disk_ids: std::collections::HashSet<String> =
        all_entries.iter().map(|e| e.id.clone()).collect();

    let orphan_ids: std::collections::HashSet<String> =
        disk_ids.difference(&referenced_ids).cloned().collect();

    let entry_map: std::collections::HashMap<String, &attachment_store::AttachmentEntry> =
        all_entries.iter().map(|e| (e.id.clone(), e)).collect();

    let orphans: Vec<AttachmentSummary> = orphan_ids
        .iter()
        .filter_map(|id| {
            entry_map.get(id).map(|entry| AttachmentSummary {
                attachment_id: entry.id.clone(),
                media_type: media_type_from_id(&entry.id),
                bytes: entry.bytes,
                created_at: entry.created_at as i64,
            })
        })
        .collect();

    let mut conv_attachments: std::collections::HashMap<String, Vec<AttachmentSummary>> =
        std::collections::HashMap::new();

    for (conv_id, att_id) in conv_rows {
        if let Some(entry) = entry_map.get(&att_id) {
            conv_attachments
                .entry(conv_id)
                .or_default()
                .push(AttachmentSummary {
                    attachment_id: entry.id.clone(),
                    media_type: media_type_from_id(&entry.id),
                    bytes: entry.bytes,
                    created_at: entry.created_at as i64,
                });
        }
    }

    let mut conversations: Vec<ConversationAttachmentSummary> = Vec::new();

    {
        let state = app.state::<crate::setup::DbState>();
        let conn = super::get_db_conn(&state)?;

        for (conv_id, attachments) in conv_attachments {
            let title: String = conn
                .query_row(
                    "SELECT title FROM conversations WHERE id = ?1",
                    rusqlite::params![conv_id],
                    |row| row.get(0),
                )
                .unwrap_or_else(|_| conv_id.clone());

            let updated_at: i64 = conn
                .query_row(
                    "SELECT updated_at FROM conversations WHERE id = ?1",
                    rusqlite::params![conv_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            let total_bytes: u64 = attachments.iter().map(|a| a.bytes).sum();

            conversations.push(ConversationAttachmentSummary {
                conversation_id: conv_id,
                title,
                attachments,
                total_bytes,
                updated_at,
            });
        }
    }

    conversations.sort_by(|a, b| b.total_bytes.cmp(&a.total_bytes));

    let total_bytes: u64 = all_entries.iter().map(|e| e.bytes).sum();
    let total_count =
        u32::try_from(all_entries.len()).map_err(|_| "attachment count exceeds u32".to_string())?;

    Ok(StorageBreakdownDto {
        total_bytes,
        total_count,
        orphans,
        conversations,
    })
}

#[command]
pub async fn delete_attachments(
    app: AppHandle,
    attachment_ids: Vec<String>,
) -> Result<AttachmentDeleteResultDto, String> {
    let attachment_store = app.state::<AttachmentStore>();

    let all_entries = attachment_store
        .list_all()
        .await
        .map_err(|e| e.to_string())?;
    let size_map: std::collections::HashMap<String, u64> = all_entries
        .iter()
        .map(|e| (e.id.clone(), e.bytes))
        .collect();

    {
        let state = app.state::<crate::setup::DbState>();
        let conn = super::get_db_conn(&state)?;

        for id in &attachment_ids {
            attachment_store::delete_attachment_refs_by_attachment_id(&conn, id)?;
        }
    }

    let mut removed_count: u64 = 0;
    let mut removed_bytes: u64 = 0;

    for id in &attachment_ids {
        let bytes = size_map.get(id).copied().unwrap_or(0);

        match attachment_store.delete(id).await {
            Ok(()) => {
                removed_count += 1;
                removed_bytes += bytes;
            }
            Err(e) => {
                log::warn!("Failed to delete attachment file {}: {}", id, e);
            }
        }
    }

    let removed_count = u32::try_from(removed_count)
        .map_err(|_| "removed attachment count exceeds u32".to_string())?;

    Ok(AttachmentDeleteResultDto {
        removed_count,
        removed_bytes,
    })
}

#[command]
pub async fn open_attachment(app: AppHandle, attachment_id: String) -> Result<bool, String> {
    if !is_plain_file_name(&attachment_id) {
        return Ok(false);
    }

    let path = app.state::<AttachmentStore>().file_path(&attachment_id);

    tauri::async_runtime::spawn_blocking(move || {
        let metadata = match std::fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e.to_string()),
        };
        if !metadata.is_file() {
            return Ok(false);
        }
        tauri_plugin_opener::open_path(path.as_path(), None::<&str>).map_err(|e| e.to_string())?;
        Ok(true)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn media_type_from_id(id: &str) -> String {
    if let Some(ext) = id.rsplit('.').next() {
        match ext {
            "png" => "image/png".to_string(),
            "jpg" | "jpeg" => "image/jpeg".to_string(),
            "gif" => "image/gif".to_string(),
            "webp" => "image/webp".to_string(),
            "pdf" => "application/pdf".to_string(),
            _ => format!("application/{}", ext),
        }
    } else {
        "application/octet-stream".to_string()
    }
}

fn is_plain_file_name(id: &str) -> bool {
    if id.is_empty() || id.starts_with('.') || id.ends_with(".tmp") {
        return false;
    }
    let mut components = std::path::Path::new(id).components();
    matches!(
        (components.next(), components.next()),
        (Some(std::path::Component::Normal(_)), None)
    )
}

#[cfg(test)]
mod tests {
    use super::is_plain_file_name;

    #[test]
    fn accepts_plain_file_name() {
        assert!(is_plain_file_name("abc123.png"));
        assert!(is_plain_file_name("abc123.pdf"));
    }

    #[test]
    fn rejects_empty_hidden_and_tmp() {
        assert!(!is_plain_file_name(""));
        assert!(!is_plain_file_name(".hidden.png"));
        assert!(!is_plain_file_name("abc123.png.tmp"));
    }

    #[test]
    fn rejects_traversal_and_separators() {
        assert!(!is_plain_file_name(".."));
        assert!(!is_plain_file_name("../abc123.png"));
        assert!(!is_plain_file_name("a/b.png"));
        assert!(!is_plain_file_name("/abc123.png"));
    }

    #[cfg(windows)]
    #[test]
    fn rejects_windows_prefix_and_backslash() {
        assert!(!is_plain_file_name(r"C:\abc123.png"));
        assert!(!is_plain_file_name(r"a\b.png"));
    }
}
