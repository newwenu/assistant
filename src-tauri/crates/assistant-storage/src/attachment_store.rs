use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;

pub struct AttachmentStoreConfig {
    pub dir: PathBuf,
}

#[derive(Clone)]
pub struct AttachmentStore {
    config: Arc<AttachmentStoreConfig>,
}

#[derive(Debug, thiserror::Error)]
pub enum AttachmentError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid media type: {0}")]
    InvalidMediaType(String),
    #[error("Image too large: {size} bytes (max {max})")]
    TooLarge { size: usize, max: usize },
    #[error("Too many images: {count} (max {max})")]
    TooManyImages { count: usize, max: usize },
    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
}

pub struct ImageAttachmentRef {
    pub id: String,
    pub media_type: String,
    pub bytes: u64,
}

pub struct AttachmentEntry {
    pub id: String,
    pub bytes: u64,
    pub created_at: u64,
}

fn media_type_to_ext(mt: &str) -> &'static str {
    match mt {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "image/tiff" => "tiff",
        "application/pdf" => "pdf",
        "text/plain" => "txt",
        "text/markdown" => "md",
        "text/csv" => "csv",
        "text/html" => "html",
        "application/json" => "json",
        "text/x-python" => "py",
        "text/javascript" => "js",
        "text/typescript" => "ts",
        "text/css" => "css",
        "text/yaml" => "yaml",
        "text/xml" => "xml",
        _ => "bin",
    }
}

impl AttachmentStore {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            config: Arc::new(AttachmentStoreConfig { dir }),
        }
    }

    /// 内容寻址写入任意附件（图片、PDF、文本等），扩展名由 `media_type` 决定。
    pub async fn save_attachment(
        &self,
        data: &[u8],
        media_type: &str,
    ) -> Result<ImageAttachmentRef, AttachmentError> {
        let hash = Sha256::digest(data);
        let sha256_hex = format!("{:x}", hash);
        let ext = media_type_to_ext(media_type);
        let attachment_id = format!("{}.{}", sha256_hex, ext);
        let path = self.config.dir.join(&attachment_id);

        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
        {
            Ok(file) => {
                use tokio::io::AsyncWriteExt;
                let mut writer = tokio::io::BufWriter::new(file);
                writer.write_all(data).await?;
                writer.flush().await?;
                drop(writer);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }

        Ok(ImageAttachmentRef {
            id: attachment_id,
            media_type: media_type.to_string(),
            bytes: data.len() as u64,
        })
    }

    pub async fn read(&self, id: &str) -> Result<Vec<u8>, AttachmentError> {
        let path = self.config.dir.join(id);
        Ok(fs::read(&path).await?)
    }

    pub async fn list_all(&self) -> Result<Vec<AttachmentEntry>, AttachmentError> {
        let mut entries = Vec::new();
        let mut dir = fs::read_dir(&self.config.dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                let id = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                let metadata = entry.metadata().await?;
                let created_at = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                entries.push(AttachmentEntry {
                    id,
                    bytes: metadata.len(),
                    created_at,
                });
            }
        }
        Ok(entries)
    }

    pub async fn delete(&self, id: &str) -> Result<(), AttachmentError> {
        let path = self.config.dir.join(id);
        Ok(fs::remove_file(&path).await?)
    }

    pub async fn clean_tmp(&self) -> Result<(), AttachmentError> {
        let mut dir = fs::read_dir(&self.config.dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".tmp") {
                    let _ = fs::remove_file(&path).await;
                }
            }
        }
        Ok(())
    }
}

pub fn insert_attachment_refs(
    conn: &Connection,
    attachment_ids: &[String],
    conversation_id: &str,
    message_id: &str,
) -> Result<(), String> {
    for aid in attachment_ids {
        conn.execute(
            "INSERT OR IGNORE INTO attachment_refs (attachment_id, conversation_id, message_id) VALUES (?1, ?2, ?3)",
            params![aid, conversation_id, message_id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn delete_attachment_refs_by_conversation(
    conn: &Connection,
    conversation_id: &str,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM attachment_refs WHERE conversation_id = ?1",
        params![conversation_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_attachment_refs_by_message(
    conn: &Connection,
    message_ids: &[String],
) -> Result<(), String> {
    for mid in message_ids {
        conn.execute(
            "DELETE FROM attachment_refs WHERE message_id = ?1",
            params![mid],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn delete_attachment_refs_by_attachment_id(
    conn: &Connection,
    attachment_id: &str,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM attachment_refs WHERE attachment_id = ?1",
        params![attachment_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_all_referenced_attachment_ids(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT attachment_id FROM attachment_refs")
        .map_err(|e| e.to_string())?;
    let ids: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(ids)
}

pub fn get_conversation_attachment_ids(
    conn: &Connection,
    conversation_id: &str,
) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT attachment_id FROM attachment_refs WHERE conversation_id = ?1")
        .map_err(|e| e.to_string())?;
    let ids: Vec<String> = stmt
        .query_map(params![conversation_id], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(ids)
}

pub fn duplicate_attachment_refs(
    conn: &Connection,
    source_conversation_id: &str,
    new_conversation_id: &str,
    message_id_map: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT attachment_id, message_id FROM attachment_refs WHERE conversation_id = ?1")
        .map_err(|e| e.to_string())?;

    let rows: Vec<(String, String)> = stmt
        .query_map(params![source_conversation_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    for (attachment_id, old_message_id) in rows {
        if let Some(new_message_id) = message_id_map.get(&old_message_id) {
            conn.execute(
                "INSERT OR IGNORE INTO attachment_refs (attachment_id, conversation_id, message_id) VALUES (?1, ?2, ?3)",
                params![attachment_id, new_conversation_id, new_message_id],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
