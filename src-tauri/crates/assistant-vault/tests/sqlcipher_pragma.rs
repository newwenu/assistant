use std::path::Path;

use assistant_vault::security_db::{SQLCIPHER_HARDENING_PRAGMAS, SQLCIPHER_PRAGMAS};
use rusqlite::Connection;

const KEY: [u8; 32] = [0x42u8; 32];

fn open_with(path: &Path, key: &[u8; 32], extra_pragmas: &str) -> Connection {
    let key_hex = hex::encode(key);
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(&format!("PRAGMA key = \"x'{key_hex}'\"; {extra_pragmas}"))
        .unwrap();
    conn
}

#[test]
fn sqlcipher_kdf_iter_one_is_noop() {
    let dir = tempfile::tempdir().unwrap();

    let a = open_with(&dir.path().join("a.db"), &KEY, "PRAGMA kdf_iter = 1;");
    a.execute("CREATE TABLE t(v INTEGER)", []).unwrap();
    drop(a);

    let b = open_with(&dir.path().join("b.db"), &KEY, "PRAGMA kdf_iter = 256000;");
    b.execute("CREATE TABLE t(v INTEGER)", []).unwrap();
    drop(b);

    let key_hex = hex::encode(KEY);

    let ca = Connection::open(dir.path().join("a.db")).unwrap();
    ca.execute_batch(&format!("PRAGMA key = \"x'{key_hex}'\";"))
        .unwrap();
    assert!(
        ca.query_row("SELECT count(*) FROM sqlite_master", [], |r| r
            .get::<_, i64>(0))
            .is_ok(),
        "raw key 下 kdf_iter=1 必须与默认写入可互开"
    );

    let cb = Connection::open(dir.path().join("b.db")).unwrap();
    cb.execute_batch(&format!(
        "PRAGMA key = \"x'{key_hex}'\"; PRAGMA kdf_iter = 1;"
    ))
    .unwrap();
    assert!(
        cb.query_row("SELECT count(*) FROM sqlite_master", [], |r| r
            .get::<_, i64>(0))
            .is_ok(),
        "raw key 下 kdf_iter 不参与派生，256000 写入的库用 1 也能打开"
    );
}

#[test]
fn hardening_pragmas_are_supported_and_effective() {
    let dir = tempfile::tempdir().unwrap();
    let key_hex = hex::encode(KEY);
    let conn = Connection::open(dir.path().join("h.db")).unwrap();
    conn.execute_batch(&format!(
        "PRAGMA key = \"x'{key_hex}'\"; {SQLCIPHER_HARDENING_PRAGMAS}"
    ))
    .expect("当前 SQLCipher 必须支持 cipher_memory_security / temp_store");

    let mem_security: String = conn
        .query_row("PRAGMA cipher_memory_security", [], |r| r.get(0))
        .unwrap();
    assert!(
        matches!(mem_security.to_ascii_uppercase().as_str(), "ON" | "1"),
        "cipher_memory_security 必须生效，实际 {}",
        mem_security
    );

    let temp_store: i64 = conn
        .query_row("PRAGMA temp_store", [], |r| r.get(0))
        .unwrap();
    assert_eq!(temp_store, 2, "temp_store 必须为 MEMORY(2)，避免临时表落盘");

    conn.execute("CREATE TABLE t(v INTEGER)", []).unwrap();
    assert!(
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r
            .get::<_, i64>(0))
            .is_ok(),
        "加固 PRAGMA 不得破坏库的可用性"
    );
}

#[test]
fn pragmas_has_no_duplicate_keys() {
    let keys = SQLCIPHER_PRAGMAS
        .split(';')
        .filter_map(|s| s.trim().split('=').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let mut uniq = keys.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(
        keys.len(),
        uniq.len(),
        "SQLCIPHER_PRAGMAS 中存在重复键: {:?}",
        keys
    );
}

#[test]
fn pragmas_pin_kdf_iter_to_one() {
    assert!(
        SQLCIPHER_PRAGMAS.contains("kdf_iter=1"),
        "raw key 模式下必须把 kdf_iter 固定为 1: {SQLCIPHER_PRAGMAS}"
    );
    assert!(
        !SQLCIPHER_PRAGMAS.contains("kdf_iter = 256000"),
        "不得保留 256000 迭代，否则每次打开库都要白跑 256000 次派生"
    );
}
