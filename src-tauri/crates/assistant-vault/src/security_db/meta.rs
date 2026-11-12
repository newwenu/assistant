use log::warn;
use rusqlite::{params, Connection, OptionalExtension};

use super::schema::SCHEMA_VERSION;

pub fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    let result = conn
        .query_row(
            "SELECT value FROM vault_meta WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(result)
}

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO vault_meta (key, value) VALUES (?1, ?2)",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn ensure_schema_version(conn: &Connection) -> Result<(), String> {
    let current: Option<String> = get_meta(conn, "schema_version")?;
    match current {
        Some(v) if v.parse::<u32>() == Ok(SCHEMA_VERSION) => Ok(()),
        Some(v) => {
            warn!(
                "Unsupported vault schema version: {} (expected {})",
                v, SCHEMA_VERSION
            );
            Err(format!(
                "Unsupported vault schema version: {} (expected {})",
                v, SCHEMA_VERSION
            ))
        }
        None => {
            set_meta(conn, "schema_version", &SCHEMA_VERSION.to_string())?;
            Ok(())
        }
    }
}

pub const ANCHOR_META_HASH_KEY: &str = "anchor_meta_hash";
pub const ANCHOR_DEK_VERSION_KEY: &str = "anchor_dek_version";
pub const ANCHOR_REVISION_KEY: &str = "anchor_revision";
pub const VAULT_ID_KEY: &str = "vault_id";

const ANCHOR_META_HASH_HEX_LEN: usize = 64;

pub fn set_vault_id(conn: &Connection, vault_id: &str) -> Result<(), String> {
    set_meta(conn, VAULT_ID_KEY, vault_id)
}

pub fn get_vault_id(conn: &Connection) -> Result<Option<String>, String> {
    get_meta(conn, VAULT_ID_KEY)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    pub meta_hash: String,
    pub dek_version: u32,
    pub revision: u64,
}

impl Anchor {
    fn validate(&self) -> Result<(), String> {
        if self.meta_hash.len() != ANCHOR_META_HASH_HEX_LEN
            || !self.meta_hash.bytes().all(|b| b.is_ascii_hexdigit())
            || self.meta_hash.bytes().any(|b| b.is_ascii_uppercase())
        {
            return Err("anchor_meta_hash must be 64 lowercase hex chars".to_string());
        }
        if self.dek_version == 0 {
            return Err("anchor_dek_version must be >= 1".to_string());
        }
        Ok(())
    }
}

pub fn get_anchor(conn: &Connection) -> Result<Option<Anchor>, String> {
    let meta_hash = get_meta(conn, ANCHOR_META_HASH_KEY)?;
    let dek_version = get_meta(conn, ANCHOR_DEK_VERSION_KEY)?;
    let revision = get_meta(conn, ANCHOR_REVISION_KEY)?;

    let (Some(meta_hash), Some(dek_version), Some(revision)) = (meta_hash, dek_version, revision)
    else {
        return Ok(None);
    };

    let dek_version = dek_version
        .parse::<u32>()
        .map_err(|e| format!("anchor_dek_version is not a u32: {}", e))?;
    let revision = revision
        .parse::<u64>()
        .map_err(|e| format!("anchor_revision is not a u64: {}", e))?;

    let anchor = Anchor {
        meta_hash,
        dek_version,
        revision,
    };
    anchor.validate()?;
    Ok(Some(anchor))
}

pub fn set_anchor(conn: &Connection, anchor: &Anchor) -> Result<(), String> {
    anchor.validate()?;

    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("Failed to open anchor transaction: {}", e))?;
    set_meta(&tx, ANCHOR_META_HASH_KEY, &anchor.meta_hash)?;
    set_meta(&tx, ANCHOR_DEK_VERSION_KEY, &anchor.dek_version.to_string())?;
    set_meta(&tx, ANCHOR_REVISION_KEY, &anchor.revision.to_string())?;
    tx.commit()
        .map_err(|e| format!("Failed to commit anchor transaction: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE vault_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
            .unwrap();
        conn
    }

    fn anchor() -> Anchor {
        Anchor {
            meta_hash: "a".repeat(64),
            dek_version: 3,
            revision: 7,
        }
    }

    #[test]
    fn anchor_roundtrip_uses_three_kv_rows() {
        let conn = conn();
        let anchor = anchor();
        set_anchor(&conn, &anchor).unwrap();

        assert_eq!(
            get_meta(&conn, ANCHOR_META_HASH_KEY).unwrap().as_deref(),
            Some(anchor.meta_hash.as_str())
        );
        assert_eq!(
            get_meta(&conn, ANCHOR_DEK_VERSION_KEY).unwrap().as_deref(),
            Some("3")
        );
        assert_eq!(
            get_meta(&conn, ANCHOR_REVISION_KEY).unwrap().as_deref(),
            Some("7")
        );
        assert_eq!(get_anchor(&conn).unwrap(), Some(anchor));
    }

    #[test]
    fn anchor_missing_any_key_returns_none() {
        for missing in [
            ANCHOR_META_HASH_KEY,
            ANCHOR_DEK_VERSION_KEY,
            ANCHOR_REVISION_KEY,
        ] {
            let conn = conn();
            set_anchor(&conn, &anchor()).unwrap();
            conn.execute("DELETE FROM vault_meta WHERE key = ?1", params![missing])
                .unwrap();
            assert_eq!(get_anchor(&conn).unwrap(), None, "missing {missing}");
        }
    }

    #[test]
    fn set_anchor_rejects_uppercase_hash() {
        let conn = conn();
        let mut anchor = anchor();
        anchor.meta_hash = "A".repeat(64);
        assert!(set_anchor(&conn, &anchor).is_err());
        assert_eq!(get_anchor(&conn).unwrap(), None);
    }

    #[test]
    fn set_anchor_rejects_short_hash() {
        let conn = conn();
        let mut anchor = anchor();
        anchor.meta_hash = "a".repeat(63);
        assert!(set_anchor(&conn, &anchor).is_err());
    }

    #[test]
    fn set_anchor_rejects_zero_dek_version() {
        let conn = conn();
        let mut anchor = anchor();
        anchor.dek_version = 0;
        assert!(set_anchor(&conn, &anchor).is_err());
    }

    #[test]
    fn get_anchor_rejects_malformed_revision() {
        let conn = conn();
        set_anchor(&conn, &anchor()).unwrap();
        set_meta(&conn, ANCHOR_REVISION_KEY, "not-a-number").unwrap();
        assert!(get_anchor(&conn).is_err());
    }

    #[test]
    fn set_anchor_is_all_or_nothing() {
        let conn = conn();
        let mut anchor = anchor();
        anchor.dek_version = 0;
        assert!(set_anchor(&conn, &anchor).is_err());
        assert!(get_meta(&conn, ANCHOR_META_HASH_KEY).unwrap().is_none());
    }

    #[test]
    fn revision_is_preserved_across_set_anchor() {
        let conn = conn();
        let mut anchor = anchor();
        set_anchor(&conn, &anchor).unwrap();
        anchor.revision += 1;
        set_anchor(&conn, &anchor).unwrap();
        assert_eq!(get_anchor(&conn).unwrap().unwrap().revision, 8);
    }

    #[test]
    fn t_c10_schema_version_mismatch_is_rejected() {
        let conn = conn();
        let stale = (SCHEMA_VERSION + 1).to_string();
        set_meta(&conn, "schema_version", &stale).unwrap();

        let err = ensure_schema_version(&conn).unwrap_err();
        assert!(
            err.contains(&SCHEMA_VERSION.to_string()),
            "错误必须说明期望版本，实际：{err}"
        );
        assert_eq!(
            get_meta(&conn, "schema_version").unwrap().as_deref(),
            Some(stale.as_str()),
            "不得静默改写版本号"
        );
    }

    #[test]
    fn t_c10_schema_version_unparsable_is_rejected() {
        let conn = conn();
        set_meta(&conn, "schema_version", "not-a-number").unwrap();

        assert!(ensure_schema_version(&conn).is_err());
    }

    #[test]
    fn t_c10_fresh_schema_version_is_stamped() {
        let conn = conn();
        assert!(get_meta(&conn, "schema_version").unwrap().is_none());

        ensure_schema_version(&conn).unwrap();
        assert_eq!(
            get_meta(&conn, "schema_version").unwrap().as_deref(),
            Some(SCHEMA_VERSION.to_string().as_str())
        );
    }
}
