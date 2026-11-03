use assistant_sync::git_sync::GitSync;
use assistant_sync::types::{ConflictFile, EntryPreview, EntryPreviewField};

use crate::security_db::SecurityDbState;
use crate::vault_file::types::{EntryBlob, ImportedEntry};

const MASKED: &str = "•••";

pub(super) fn mask_entry_preview(
    commit: &str,
    id: String,
    category: String,
    revision: i64,
    blob: EntryBlob,
) -> EntryPreview {
    EntryPreview {
        id,
        commit: commit.to_string(),
        commit_author: None,
        commit_time: None,
        category,
        revision,
        title: blob.title,
        username: blob.username,
        url: blob.url,
        password: blob.password.map(|_| MASKED.to_string()),
        notes: blob.notes,
        totp_secret: blob.totp_secret.map(|_| MASKED.to_string()),
        custom_fields: blob
            .custom_fields
            .into_iter()
            .map(|cf| EntryPreviewField {
                name: cf.name,
                value: if cf.is_secret {
                    MASKED.to_string()
                } else {
                    cf.value
                },
                is_secret: cf.is_secret,
            })
            .collect(),
        tags: blob.tags,
        is_favorite: blob.is_favorite,
        created_at: blob.created_at,
        updated_at: blob.updated_at,
    }
}

pub(super) fn entry_rel_path(entry_id: &str) -> String {
    format!("entries/{}.enc", entry_id)
}

pub(super) fn parse_entry_id(path: &str) -> Option<String> {
    path.trim()
        .strip_prefix("entries/")
        .and_then(|rest| rest.strip_suffix(".enc"))
        .map(|id| id.to_string())
}

pub(super) fn collect_conflicts(git: &GitSync) -> Vec<ConflictFile> {
    match git.detect_conflicts() {
        Ok(conflicts) => conflicts,
        Err(e) => {
            log::warn!("Failed to enumerate conflicted files: {e}");
            Vec::new()
        }
    }
}

pub(super) fn attach_conflict_titles(
    security_db: &SecurityDbState,
    conflicts: &mut [ConflictFile],
) {
    for conflict in conflicts.iter_mut() {
        let Some(entry_id) = parse_entry_id(&conflict.path) else {
            continue;
        };
        match security_db.get_entry(&entry_id) {
            Ok(entry) => conflict.title = Some(entry.title),
            Err(_) => continue,
        }
    }
}

pub(super) fn diff_entry_fields(local: &ImportedEntry, remote: &ImportedEntry) -> Vec<String> {
    let mut fields = Vec::new();

    if local.category != remote.category {
        fields.push("category".to_string());
    }
    if local.revision != remote.revision {
        fields.push("revision".to_string());
    }

    let a = &local.blob;
    let b = &remote.blob;

    if a.title != b.title {
        fields.push("title".to_string());
    }
    if a.username != b.username {
        fields.push("username".to_string());
    }
    if a.url != b.url {
        fields.push("url".to_string());
    }
    if a.password != b.password {
        fields.push("password".to_string());
    }
    if a.notes != b.notes {
        fields.push("notes".to_string());
    }
    if a.totp_secret != b.totp_secret {
        fields.push("totpSecret".to_string());
    }
    if a.tags != b.tags {
        fields.push("tags".to_string());
    }
    if a.is_favorite != b.is_favorite {
        fields.push("isFavorite".to_string());
    }
    if a.created_at != b.created_at {
        fields.push("createdAt".to_string());
    }
    if a.updated_at != b.updated_at {
        fields.push("updatedAt".to_string());
    }

    let names: std::collections::BTreeSet<&str> = a
        .custom_fields
        .iter()
        .map(|field| field.name.as_str())
        .chain(b.custom_fields.iter().map(|field| field.name.as_str()))
        .collect();

    for name in names {
        let left = a.custom_fields.iter().find(|field| field.name == name);
        let right = b.custom_fields.iter().find(|field| field.name == name);
        let same = match (left, right) {
            (Some(left), Some(right)) => {
                left.value == right.value && left.is_secret == right.is_secret
            }
            _ => false,
        };
        if !same {
            fields.push(format!("custom:{}", name));
        }
    }

    fields
}
