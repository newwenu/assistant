use assistant_config::atomic_write::{atomic_write, recover_temp};
use std::fs;

#[test]
fn atomic_write_creates_file_with_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.json");
    let data = b"hello world";

    atomic_write(&path, data).unwrap();

    let content = fs::read(&path).unwrap();
    assert_eq!(content, data);
}

#[test]
fn atomic_write_creates_parent_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("deep").join("test.json");

    atomic_write(&path, b"data").unwrap();

    assert!(path.exists());
    assert_eq!(fs::read(&path).unwrap(), b"data");
}

#[test]
fn atomic_write_overwrites_existing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.json");

    atomic_write(&path, b"first").unwrap();
    atomic_write(&path, b"second").unwrap();

    assert_eq!(fs::read(&path).unwrap(), b"second");
}

#[test]
fn atomic_write_no_leftover_temp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.json");

    atomic_write(&path, b"data").unwrap();

    let temp = path.with_extension("tmp");
    assert!(!temp.exists());
}

#[test]
fn atomic_write_empty_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.json");

    atomic_write(&path, b"").unwrap();

    assert!(path.exists());
    assert_eq!(fs::read(&path).unwrap(), b"");
}

#[test]
fn recover_temp_none_when_no_temp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.json");

    let result = recover_temp(&path);
    assert!(result.is_none());
}

#[test]
fn recover_temp_reads_and_removes_temp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.json");
    let temp = path.with_extension("tmp");

    fs::write(&temp, b"recovery data").unwrap();

    let result = recover_temp(&path);
    assert_eq!(result, Some(b"recovery data".to_vec()));
    assert!(!temp.exists());
}

#[test]
fn recover_temp_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.json");
    let temp = path.with_extension("tmp");

    fs::write(&temp, b"data").unwrap();

    let first = recover_temp(&path);
    assert!(first.is_some());

    let second = recover_temp(&path);
    assert!(second.is_none());
}

#[test]
fn recover_temp_directory_as_temp_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.json");
    let temp = path.with_extension("tmp");

    fs::create_dir_all(&temp).unwrap();

    let result = recover_temp(&path);
    assert!(result.is_none());
}
