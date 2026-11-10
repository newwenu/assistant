use assistant_storage::attachment_store::{is_plain_file_name, AttachmentError, AttachmentStore};

fn store() -> (tempfile::TempDir, AttachmentStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = AttachmentStore::new(dir.path().to_path_buf());
    (dir, store)
}

fn is_invalid_id(err: &AttachmentError) -> bool {
    matches!(err, AttachmentError::InvalidId(_))
}

#[test]
fn t3_plain_file_name_vectors() {
    assert!(is_plain_file_name("abc.png"));
    assert!(is_plain_file_name("0123456789abcdef0123456789abcdef.pdf"));

    for bad in [
        "",
        ".",
        "..",
        "a/b",
        "a\\b",
        ".hidden",
        "x.tmp",
        "C:x",
        "a:b",
        "abc\u{0}.png",
        "abc\u{1}.png",
        "../abc.png",
    ] {
        assert!(!is_plain_file_name(bad), "应拒绝 id: {:?}", bad);
    }
}

#[tokio::test]
async fn t5_invalid_id_rejected_at_every_entry() {
    let (_guard, store) = store();
    let bad = "../escape.png";

    assert!(is_invalid_id(&store.read(bad).await.unwrap_err()));
    assert!(is_invalid_id(&store.file_path(bad).unwrap_err()));
    assert!(is_invalid_id(
        &store.write_derived_text(bad, "x").await.unwrap_err()
    ));
    assert!(is_invalid_id(
        &store.delete_derived_text(bad).await.unwrap_err()
    ));
    assert!(is_invalid_id(&store.delete(bad).await.unwrap_err()));
}

#[tokio::test]
async fn t5_read_derived_text_returns_none_without_io() {
    let (guard, store) = store();
    assert_eq!(store.read_derived_text("../escape.pdf").await, None);
    let derived = guard.path().join("derived");
    assert!(!derived.exists(), "非法 id 不应触发任何 IO");
}

#[tokio::test]
async fn valid_id_roundtrip_through_file_path() {
    let (guard, store) = store();
    let saved = store
        .save_attachment(b"pdf-bytes", "application/pdf")
        .await
        .unwrap();
    store.write_derived_text(&saved.id, "hello").await.unwrap();
    assert_eq!(
        store.read_derived_text(&saved.id).await.as_deref(),
        Some("hello")
    );
    assert_eq!(
        store.file_path(&saved.id).unwrap(),
        guard.path().join(&saved.id)
    );
    store.delete(&saved.id).await.unwrap();
    assert_eq!(store.read_derived_text(&saved.id).await, None);
    assert!(!guard.path().join(&saved.id).exists());
}

#[tokio::test]
async fn t12_long_path_survives_save_read_delete() {
    let base = tempfile::tempdir().unwrap();
    let mut root = base.path().to_path_buf();
    for _ in 0..6 {
        root.push("a".repeat(40));
    }
    tokio::fs::create_dir_all(&root).await.unwrap();

    let store = AttachmentStore::new(root.clone());
    let data = b"long path payload";
    let saved = store.save_attachment(data, "text/plain").await.unwrap();
    let path = store.file_path(&saved.id).unwrap();

    assert!(
        path.as_os_str().len() > 260,
        "构造的路径应超过 260，实际 {}",
        path.as_os_str().len()
    );
    assert_eq!(path, root.join(&saved.id), "路径不得被截断或改写");
    assert_eq!(store.read(&saved.id).await.unwrap(), data);

    store.delete(&saved.id).await.unwrap();
    assert!(!path.exists());
}
