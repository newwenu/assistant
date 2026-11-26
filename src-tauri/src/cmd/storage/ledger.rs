//! 附件引用账本（§3.10 第 1 条）：会话库 → `AttachmentRefs` 的适配与顶层字段装配。

use std::collections::{HashMap, HashSet};

use crate::db_state::DbState;
use crate::protocol::dto::{AttachmentSummary, ConversationAttachmentSummary};
use assistant_storage::attachment_store::{self, AttachmentEntry};

use super::entries::BreakdownBase;

/// 引用侧数据：`referenced` 判孤儿，`rows` 供会话分组与「共享 / 仅引用一次」计数。
pub(super) struct AttachmentRefs {
    pub(super) referenced: HashSet<String>,
    pub(super) rows: Vec<ConvAttachmentRow>,
}

pub(super) struct ConvAttachmentRow {
    pub(super) conversation_id: String,
    pub(super) attachment_id: String,
    pub(super) title: String,
    pub(super) updated_at: i64,
}

/// 一次连接、一条 JOIN 拿全：标题与 `updated_at` 随行带回，避免逐会话回查（N+1）。
pub(super) fn load_attachment_refs(db: &DbState) -> Result<AttachmentRefs, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    let referenced = attachment_store::get_all_referenced_attachment_ids(&conn)?
        .into_iter()
        .collect();

    let mut stmt = conn
        .prepare(
            "SELECT ar.conversation_id, ar.attachment_id, \
                    COALESCE(c.title, ar.conversation_id), COALESCE(c.updated_at, 0) \
             FROM attachment_refs ar \
             LEFT JOIN conversations c ON c.id = ar.conversation_id \
             ORDER BY ar.conversation_id",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(ConvAttachmentRow {
                conversation_id: row.get(0)?,
                attachment_id: row.get(1)?,
                title: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(AttachmentRefs { referenced, rows })
}

fn summary(entry: &AttachmentEntry) -> AttachmentSummary {
    AttachmentSummary {
        attachment_id: entry.id.clone(),
        media_type: media_type_from_id(&entry.id),
        bytes: entry.bytes,
        created_at: i64::try_from(entry.created_at).unwrap_or(i64::MAX),
    }
}

/// 由同一份 `list_all` 派生顶层字段与 `E1`（§3.10 第 1 条）：两边不可能对不上。
pub(super) fn build_base(entries: &[AttachmentEntry], refs: AttachmentRefs) -> BreakdownBase {
    let entry_map: HashMap<&str, &AttachmentEntry> = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect();

    let mut orphans: Vec<AttachmentSummary> = entries
        .iter()
        .filter(|entry| !refs.referenced.contains(&entry.id))
        .map(summary)
        .collect();
    orphans.sort_by_key(|orphan| std::cmp::Reverse(orphan.created_at));

    let mut conv_count: HashMap<&str, u64> = HashMap::new();
    let mut conv_attachments: HashMap<&str, (String, i64, Vec<AttachmentSummary>)> = HashMap::new();
    let mut seen: HashSet<(&str, &str)> = HashSet::new();

    for row in &refs.rows {
        let Some(entry) = entry_map.get(row.attachment_id.as_str()) else {
            continue;
        };
        // `attachment_refs` 的主键是（附件，会话，消息）：同一会话里多条消息引用同一附件
        // 会有多行。按（会话，附件）去重——否则同组内出现重复行（前端 key 冲突），
        // 「共享」也会把会话内重复引用误算成跨会话共享（§3.7：按会话数判，同一附件只计一次）。
        if !seen.insert((row.conversation_id.as_str(), entry.id.as_str())) {
            continue;
        }
        *conv_count.entry(entry.id.as_str()).or_default() += 1;
        let bucket = conv_attachments
            .entry(row.conversation_id.as_str())
            .or_insert_with(|| (row.title.clone(), row.updated_at, Vec::new()));
        bucket.2.push(summary(entry));
    }

    let mut conversations: Vec<ConversationAttachmentSummary> = conv_attachments
        .into_iter()
        .map(|(conversation_id, (title, updated_at, attachments))| {
            let total_bytes = attachments.iter().map(|attachment| attachment.bytes).sum();
            ConversationAttachmentSummary {
                conversation_id: conversation_id.to_string(),
                title,
                attachments,
                total_bytes,
                updated_at,
            }
        })
        .collect();
    conversations.sort_by_key(|conversation| std::cmp::Reverse(conversation.total_bytes));

    let shared_bytes: u64 = entries
        .iter()
        .filter(|entry| conv_count.get(entry.id.as_str()).copied().unwrap_or(0) >= 2)
        .map(|entry| entry.bytes)
        .sum();
    let referenced_once_bytes: u64 = entries
        .iter()
        .filter(|entry| conv_count.get(entry.id.as_str()).copied() == Some(1))
        .map(|entry| entry.bytes)
        .sum();

    BreakdownBase {
        total_bytes: entries.iter().map(|entry| entry.bytes).sum(),
        total_count: u32::try_from(entries.len()).unwrap_or(u32::MAX),
        orphans,
        conversations,
        shared_bytes,
        referenced_once_bytes,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, bytes: u64, created_at: u64) -> AttachmentEntry {
        AttachmentEntry {
            id: id.to_string(),
            bytes,
            created_at,
        }
    }

    fn row(conversation_id: &str, attachment_id: &str) -> ConvAttachmentRow {
        ConvAttachmentRow {
            conversation_id: conversation_id.to_string(),
            attachment_id: attachment_id.to_string(),
            title: conversation_id.to_string(),
            updated_at: 0,
        }
    }

    #[test]
    fn ledger_identity_splits_shared_once_and_orphan() {
        let entries = vec![
            entry("once.png", 100, 1),
            entry("shared.png", 200, 2),
            entry("orphan.pdf", 300, 3),
        ];
        let refs = AttachmentRefs {
            referenced: ["once.png", "shared.png"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            rows: vec![
                row("c1", "once.png"),
                row("c1", "shared.png"),
                row("c2", "shared.png"),
            ],
        };
        let base = build_base(&entries, refs);

        assert_eq!(base.total_bytes, 600, "合计 = 全部文件长度之和");
        assert_eq!(base.total_count, 3);
        assert_eq!(base.referenced_once_bytes, 100, "仅被一个会话引用");
        assert_eq!(base.shared_bytes, 200, "被多个会话共享");
        assert_eq!(base.orphans.len(), 1);
        let orphan_bytes: u64 = base.orphans.iter().map(|orphan| orphan.bytes).sum();
        assert_eq!(
            base.total_bytes,
            base.referenced_once_bytes + base.shared_bytes + orphan_bytes,
            "账本恒等式（V5）：每一字节只归属一处"
        );
    }

    #[test]
    fn conversations_group_by_id_with_title_from_join() {
        let entries = vec![entry("a.png", 10, 1), entry("b.png", 20, 2)];
        let refs = AttachmentRefs {
            referenced: ["a.png", "b.png"].into_iter().map(str::to_string).collect(),
            rows: vec![row("c1", "a.png"), row("c1", "b.png")],
        };
        let base = build_base(&entries, refs);

        assert_eq!(base.conversations.len(), 1);
        assert_eq!(base.conversations[0].conversation_id, "c1");
        assert_eq!(base.conversations[0].attachments.len(), 2);
        assert_eq!(base.conversations[0].total_bytes, 30);
    }

    /// `attachment_refs` 一行一条消息引用：同一会话里两条消息引用同一附件会出两行。
    /// 必须按（会话，附件）去重——否则同组内出现两行同 key（前端 key 冲突），
    /// 且「共享」判定把会话内重复引用误算成跨会话共享。
    #[test]
    fn repeated_refs_in_one_conversation_are_deduped() {
        let entries = vec![entry("same.txt", 100, 1), entry("other.txt", 200, 2)];
        let refs = AttachmentRefs {
            referenced: ["same.txt", "other.txt"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            rows: vec![
                row("c1", "same.txt"),
                row("c1", "same.txt"),
                row("c1", "other.txt"),
                row("c2", "same.txt"),
            ],
        };
        let base = build_base(&entries, refs);

        assert_eq!(base.conversations.len(), 2);
        let c1 = base
            .conversations
            .iter()
            .find(|conversation| conversation.conversation_id == "c1")
            .expect("c1 恒在");
        assert_eq!(c1.attachments.len(), 2, "同一会话内同一附件只出现一次");
        assert_eq!(c1.total_bytes, 300);
        assert_eq!(base.shared_bytes, 100, "只有真跨会话（c1 + c2）才算共享");
        assert_eq!(
            base.referenced_once_bytes, 200,
            "c1 内的 other.txt 仅一个会话"
        );
        assert_eq!(
            base.total_bytes,
            base.referenced_once_bytes + base.shared_bytes,
            "账本恒等式不受会话内重复引用影响"
        );
    }
}
