use argon2::{Algorithm, Argon2, Params, Version};
use hkdf::Hkdf;
use rusqlite::Connection;
use sha2::Sha256;
use std::time::Duration;

#[link(name = "user32")]
#[link(name = "crypt32")]
extern "C" {}

fn main() {
    let password = "TestPassword123!";
    let salt = [0u8; 32];

    let argon2_params = Params::new(65536, 3, 4, Some(32)).unwrap();
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2_params);

    println!("=== Argon2id (64MB, 3 passes, 4 lanes) ===");
    let mut key = [0u8; 32];

    let t1 = measure(|| {
        argon2
            .hash_password_into(password.as_bytes(), &salt, &mut key)
            .unwrap();
    });
    println!("  #1 (bootstrap_salt):  {:?}", t1);

    let t2 = measure(|| {
        argon2
            .hash_password_into(password.as_bytes(), &salt, &mut key)
            .unwrap();
    });
    println!("  #2 (master_key_salt): {:?}", t2);

    let t3 = measure(|| {
        argon2
            .hash_password_into(password.as_bytes(), &salt, &mut key)
            .unwrap();
    });
    println!("  #3 (hash_salt verify): {:?}", t3);
    let argon2_total = t1 + t2 + t3;
    println!("  Argon2id total:       {:?}", argon2_total);

    println!("\n=== HKDF (bootstrap_key -> sqlcipher_key) ===");
    let t_hkdf = measure(|| {
        let hkdf = Hkdf::<Sha256>::new(None, &key);
        let mut sqlcipher_key = [0u8; 32];
        hkdf.expand(b"sqlcipher", &mut sqlcipher_key).unwrap();
    });
    println!("  HKDF: {:?}", t_hkdf);

    println!("\n=== SQLCipher open + PRAGMA (kdf_iter=256000, raw hex key) ===");
    let db_path = std::env::temp_dir().join("bench_unlock_test.db");
    let _ = std::fs::remove_file(&db_path);

    let sqlcipher_key_hex = {
        let hkdf = Hkdf::<Sha256>::new(None, &key);
        let mut sk = [0u8; 32];
        hkdf.expand(b"sqlcipher", &mut sk).unwrap();
        hex::encode(sk)
    };

    let t_create = measure(|| {
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", sqlcipher_key_hex))
            .unwrap();
        conn.execute_batch("PRAGMA cipher_page_size = 4096; PRAGMA kdf_iter = 256000; PRAGMA cipher_hmac_algorithm = HMAC_SHA512; PRAGMA cipher_kdf_algorithm = PBKDF2_HMAC_SHA512;").unwrap();
        conn.execute_batch("CREATE TABLE IF NOT EXISTS vault_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL); INSERT OR IGNORE INTO vault_meta VALUES ('schema_version','1');").unwrap();
    });
    println!("  Create + PRAGMA:     {:?}", t_create);

    let t_open = measure(|| {
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", sqlcipher_key_hex))
            .unwrap();
        conn.execute_batch("PRAGMA cipher_page_size = 4096; PRAGMA kdf_iter = 256000; PRAGMA cipher_hmac_algorithm = HMAC_SHA512; PRAGMA cipher_kdf_algorithm = PBKDF2_HMAC_SHA512;").unwrap();
        let _: String = conn
            .query_row(
                "SELECT value FROM vault_meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    });
    println!("  Open + PRAGMA + read: {:?}", t_open);

    println!("\n=== SQLCipher open + PRAGMA (kdf_iter=1, raw hex key) ===");
    let db_path2 = std::env::temp_dir().join("bench_unlock_test_kdf1.db");
    let _ = std::fs::remove_file(&db_path2);

    let t_create2 = measure(|| {
        let conn = Connection::open(&db_path2).unwrap();
        conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", sqlcipher_key_hex))
            .unwrap();
        conn.execute_batch("PRAGMA cipher_page_size = 4096; PRAGMA kdf_iter = 1; PRAGMA cipher_hmac_algorithm = HMAC_SHA512; PRAGMA cipher_kdf_algorithm = PBKDF2_HMAC_SHA512;").unwrap();
        conn.execute_batch("CREATE TABLE IF NOT EXISTS vault_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL); INSERT OR IGNORE INTO vault_meta VALUES ('schema_version','1');").unwrap();
    });
    println!("  Create + PRAGMA (kdf=1):   {:?}", t_create2);

    let t_open2 = measure(|| {
        let conn = Connection::open(&db_path2).unwrap();
        conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", sqlcipher_key_hex))
            .unwrap();
        conn.execute_batch("PRAGMA cipher_page_size = 4096; PRAGMA kdf_iter = 1; PRAGMA cipher_hmac_algorithm = HMAC_SHA512; PRAGMA cipher_kdf_algorithm = PBKDF2_HMAC_SHA512;").unwrap();
        let _: String = conn
            .query_row(
                "SELECT value FROM vault_meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    });
    println!("  Open + PRAGMA + read (kdf=1): {:?}", t_open2);

    println!("\n=== SQLCipher with PASSPHRASE key (kdf_iter=256000) ===");
    let db_path3 = std::env::temp_dir().join("bench_unlock_test_passphrase.db");
    let _ = std::fs::remove_file(&db_path3);

    let t_passphrase_create = measure(|| {
        let conn = Connection::open(&db_path3).unwrap();
        conn.execute_batch("PRAGMA key = 'test_passphrase_for_kdf_benchmark';")
            .unwrap();
        conn.execute_batch("PRAGMA cipher_page_size = 4096; PRAGMA kdf_iter = 256000; PRAGMA cipher_hmac_algorithm = HMAC_SHA512; PRAGMA cipher_kdf_algorithm = PBKDF2_HMAC_SHA512;").unwrap();
        conn.execute_batch("CREATE TABLE IF NOT EXISTS vault_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL); INSERT OR IGNORE INTO vault_meta VALUES ('schema_version','1');").unwrap();
    });
    println!(
        "  Create with passphrase (kdf=256000): {:?}",
        t_passphrase_create
    );

    let t_passphrase_open = measure(|| {
        let conn = Connection::open(&db_path3).unwrap();
        conn.execute_batch("PRAGMA key = 'test_passphrase_for_kdf_benchmark';")
            .unwrap();
        conn.execute_batch("PRAGMA cipher_page_size = 4096; PRAGMA kdf_iter = 256000; PRAGMA cipher_hmac_algorithm = HMAC_SHA512; PRAGMA cipher_kdf_algorithm = PBKDF2_HMAC_SHA512;").unwrap();
        let _: String = conn
            .query_row(
                "SELECT value FROM vault_meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    });
    println!(
        "  Open with passphrase (kdf=256000):  {:?}",
        t_passphrase_open
    );

    println!("\n=== Summary ===");
    println!(
        "  Argon2id x3 total:                          {:?}",
        argon2_total
    );
    println!("  SQLCipher open (raw hex key, kdf=256000):   {:?}", t_open);
    println!(
        "  SQLCipher open (raw hex key, kdf=1):        {:?}",
        t_open2
    );
    println!(
        "  SQLCipher open (passphrase, kdf=256000):    {:?}",
        t_passphrase_open
    );
    println!(
        "  Crypto total (current):                     {:?}",
        argon2_total + t_open
    );
    let diff = if t_open > t_open2 {
        t_open - t_open2
    } else {
        Duration::ZERO
    };
    println!("  kdf_iter difference (raw key):              {:?}", diff);
    let pass_diff = if t_passphrase_open > t_open2 {
        t_passphrase_open - t_open2
    } else {
        Duration::ZERO
    };
    println!(
        "  kdf_iter difference (passphrase vs raw):    {:?}",
        pass_diff
    );

    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(&db_path2);
    let _ = std::fs::remove_file(&db_path3);
}

fn measure(f: impl FnOnce()) -> std::time::Duration {
    let start = std::time::Instant::now();
    f();
    start.elapsed()
}
