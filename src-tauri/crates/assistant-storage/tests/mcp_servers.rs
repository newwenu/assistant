use assistant_storage::mcp_servers::{
    delete_server, get_server, insert_server, list_enabled_servers, list_servers,
    set_server_enabled, update_server, McpLaunchConfig, McpServerRow,
};

mod common;

fn row(id: &str, enabled: bool, updated_at: i64) -> McpServerRow {
    McpServerRow {
        id: id.to_string(),
        name: format!("{id} display"),
        transport: "stdio".to_string(),
        config_encrypted: "ciphertext".to_string(),
        config_hash: "hash".to_string(),
        enabled,
        created_at: 1000,
        updated_at,
    }
}

#[test]
fn insert_get_list_roundtrip() {
    let conn = common::init_db();
    insert_server(&conn, &row("srv-a", false, 1000)).unwrap();
    insert_server(&conn, &row("srv-b", true, 1000)).unwrap();

    let fetched = get_server(&conn, "srv-a").unwrap().expect("row");
    assert_eq!(fetched, row("srv-a", false, 1000));

    let all = list_servers(&conn).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].id, "srv-a", "按 created_at, id 排序");

    let enabled = list_enabled_servers(&conn).unwrap();
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].id, "srv-b");

    assert!(get_server(&conn, "missing").unwrap().is_none());
}

#[test]
fn update_never_changes_id() {
    let conn = common::init_db();
    insert_server(&conn, &row("srv-a", false, 1000)).unwrap();

    let mut changed = row("srv-a", true, 2000);
    changed.name = "renamed".to_string();
    changed.config_hash = "hash-2".to_string();
    update_server(&conn, &changed).unwrap();

    let fetched = get_server(&conn, "srv-a").unwrap().expect("row");
    assert_eq!(fetched.name, "renamed");
    assert_eq!(fetched.config_hash, "hash-2");
    assert!(fetched.enabled);
    assert_eq!(fetched.created_at, 1000, "创建时间不参与更新");
    assert_eq!(fetched.updated_at, 2000);

    // 用另一个 id 更新只会命中 0 行，不会创建新行，也不会改写旧行主键。
    let mut ghost = row("srv-b", false, 3000);
    ghost.name = "ghost".to_string();
    update_server(&conn, &ghost).unwrap();
    assert!(get_server(&conn, "srv-b").unwrap().is_none());
    assert_eq!(get_server(&conn, "srv-a").unwrap().unwrap().name, "renamed");
}

#[test]
fn enabled_persists_across_reopen_and_defaults_off() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mcp.db");

    {
        let conn = common::init_db_at(&path);
        insert_server(&conn, &row("srv-a", true, 1000)).unwrap();
        insert_server(&conn, &row("srv-b", false, 1000)).unwrap();
    }

    let conn = common::init_db_at(&path);
    let enabled = list_enabled_servers(&conn).unwrap();
    assert_eq!(
        enabled.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["srv-a"],
        "enabled 跨重启保留"
    );

    set_server_enabled(&conn, "srv-a", false, 2000).unwrap();
    assert!(list_enabled_servers(&conn).unwrap().is_empty());
    let fetched = get_server(&conn, "srv-a").unwrap().unwrap();
    assert!(!fetched.enabled);
    assert_eq!(fetched.updated_at, 2000);
}

#[test]
fn delete_removes_row() {
    let conn = common::init_db();
    insert_server(&conn, &row("srv-a", false, 1000)).unwrap();
    delete_server(&conn, "srv-a").unwrap();
    assert!(get_server(&conn, "srv-a").unwrap().is_none());
    assert!(list_servers(&conn).unwrap().is_empty());
    delete_server(&conn, "missing").unwrap();
}

#[test]
fn canonical_json_sorts_keys_and_keeps_array_order() {
    let mut first = McpLaunchConfig {
        command: "node".to_string(),
        args: vec!["b.js".to_string(), "a.js".to_string()],
        cwd: Some("C:/work".to_string()),
        ..Default::default()
    };
    first.env.insert("B_TOKEN".to_string(), "x".to_string());
    first.env.insert("A_KEY".to_string(), "y".to_string());

    let canonical = first.canonical_json().unwrap();
    assert_eq!(
        canonical,
        r#"{"args":["b.js","a.js"],"command":"node","cwd":"C:/work","env":{"A_KEY":"y","B_TOKEN":"x"}}"#
    );

    // 同样的内容、不同的插入顺序 → 同一哈希。
    let mut second = McpLaunchConfig::default();
    second.env.insert("B_TOKEN".to_string(), "x".to_string());
    second.command = "node".to_string();
    second.cwd = Some("C:/work".to_string());
    second.args = vec!["b.js".to_string(), "a.js".to_string()];
    second.env.insert("A_KEY".to_string(), "y".to_string());
    assert_eq!(first.config_hash().unwrap(), second.config_hash().unwrap());
}

#[test]
fn config_hash_changes_with_any_effective_field() {
    let base = McpLaunchConfig {
        command: "node".to_string(),
        args: vec!["server.js".to_string()],
        ..Default::default()
    };
    let base_hash = base.config_hash().unwrap();
    assert_eq!(base_hash.len(), 64, "SHA-256 十六进制");

    let mut with_env = base.clone();
    with_env.env.insert("TOKEN".to_string(), "v".to_string());
    assert_ne!(base_hash, with_env.config_hash().unwrap());

    let mut with_cwd = base.clone();
    with_cwd.cwd = Some("C:/tmp".to_string());
    assert_ne!(base_hash, with_cwd.config_hash().unwrap());

    let mut with_args = base.clone();
    with_args.args.push("--flag".to_string());
    assert_ne!(base_hash, with_args.config_hash().unwrap());
}
