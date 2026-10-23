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
}

fn media_type_to_ext(mt: &str) -> &'static str {
    match mt {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "bin",
    }
}

impl AttachmentStore {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            config: Arc::new(AttachmentStoreConfig { dir }),
        }
    }

    pub async fn save_image(
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
                entries.push(AttachmentEntry {
                    id,
                    bytes: metadata.len(),
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
