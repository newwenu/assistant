use rusqlite::Connection;

use assistant_security::types::{VaultCategory, VaultEntrySummary};

pub fn list_entry_summaries(conn: &Connection) -> Result<Vec<VaultEntrySummary>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, category, title, username, url, tags, is_favorite, updated_at
             FROM vault_entries WHERE deleted_at IS NULL ORDER BY updated_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let summaries = stmt
        .query_map([], |row| {
            let category_str: String = row.get("category")?;
            let category = VaultCategory::from_str(&category_str).unwrap_or(VaultCategory::Custom);
            let tags_json: String = row.get("tags")?;
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
            Ok(VaultEntrySummary {
                id: row.get("id")?,
                category,
                title: row.get("title")?,
                username: row.get("username")?,
                url: row.get("url")?,
                tags,
                is_favorite: row.get::<_, i64>("is_favorite")? != 0,
                updated_at: row.get("updated_at")?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(summaries)
}

pub fn count_entries(conn: &Connection) -> Result<u64, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM vault_entries WHERE deleted_at IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(count as u64)
}
