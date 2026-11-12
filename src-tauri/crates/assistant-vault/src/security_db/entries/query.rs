use rusqlite::Connection;

use crate::dto::{VaultEntryDetail, VaultEntrySummary};
use crate::security_db::SecurityDbState;
use crate::types::{VaultCategory, VaultError};

impl SecurityDbState {
    pub fn list_entries(
        &self,
        category: Option<VaultCategory>,
        search: Option<&str>,
        tag: Option<&str>,
    ) -> Result<Vec<VaultEntrySummary>, VaultError> {
        self.with_conn(|conn| list_entries_in(conn, category, search, tag))
    }

    pub fn count_entries(&self) -> Result<u32, VaultError> {
        self.with_conn(count_entries_in)
    }

    pub fn get_entry(&self, id: &str) -> Result<VaultEntryDetail, VaultError> {
        let snapshot = self.snapshot(id)?;
        Ok(VaultEntryDetail::from(&snapshot))
    }

    pub fn list_entry_ids(&self, include_deleted: bool) -> Result<Vec<String>, VaultError> {
        self.with_conn(|conn| list_entry_ids_in(conn, include_deleted))
    }
}

fn list_entries_in(
    conn: &Connection,
    category: Option<VaultCategory>,
    search: Option<&str>,
    tag: Option<&str>,
) -> Result<Vec<VaultEntrySummary>, VaultError> {
    let mut sql = String::from(
        "SELECT id, title, username, url, category, tags, is_favorite, updated_at \
         FROM vault_entries WHERE deleted_at IS NULL",
    );
    let mut param_idx = 1u32;

    if category.is_some() {
        sql.push_str(&format!(" AND category = ?{}", param_idx));
        param_idx += 1;
    }
    if search.is_some() {
        sql.push_str(&format!(
            " AND (title LIKE ?{} OR username LIKE ?{} OR url LIKE ?{})",
            param_idx,
            param_idx + 1,
            param_idx + 2
        ));
        param_idx += 3;
    }
    if tag.is_some() {
        sql.push_str(&format!(" AND tags LIKE ?{}", param_idx));
    }
    sql.push_str(" ORDER BY updated_at DESC");

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| VaultError::Database(e.to_string()))?;

    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(cat) = category {
        param_values.push(Box::new(cat.to_string()));
    }
    if let Some(s) = search {
        let pattern = format!("%{}%", s);
        param_values.push(Box::new(pattern.clone()));
        param_values.push(Box::new(pattern.clone()));
        param_values.push(Box::new(pattern));
    }
    if let Some(t) = tag {
        param_values.push(Box::new(format!("%\"{}\"%", t)));
    }

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let rows = stmt
        .query_map(param_refs.as_slice(), |row| {
            let category_str: String = row.get("category")?;
            let tags_json: String = row.get("tags")?;
            Ok(VaultEntrySummary {
                id: row.get("id")?,
                category: VaultCategory::parse(&category_str).unwrap_or_default(),
                title: row.get("title")?,
                username: row.get("username")?,
                url: row.get("url")?,
                tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                is_favorite: row.get::<_, i64>("is_favorite")? != 0,
                updated_at: row.get("updated_at")?,
            })
        })
        .map_err(|e| VaultError::Database(e.to_string()))?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.map_err(|e| VaultError::Database(e.to_string()))?);
    }
    Ok(entries)
}

fn count_entries_in(conn: &Connection) -> Result<u32, VaultError> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM vault_entries WHERE deleted_at IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(|e| VaultError::Database(e.to_string()))?;

    u32::try_from(count).map_err(|_| VaultError::Validation("entry count exceeds u32".to_string()))
}

fn list_entry_ids_in(conn: &Connection, include_deleted: bool) -> Result<Vec<String>, VaultError> {
    let sql = if include_deleted {
        "SELECT id FROM vault_entries ORDER BY created_at"
    } else {
        "SELECT id FROM vault_entries WHERE deleted_at IS NULL ORDER BY created_at"
    };

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| VaultError::Database(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| VaultError::Database(e.to_string()))?;

    let mut ids = Vec::new();
    for row in rows {
        ids.push(row.map_err(|e| VaultError::Database(e.to_string()))?);
    }
    Ok(ids)
}
