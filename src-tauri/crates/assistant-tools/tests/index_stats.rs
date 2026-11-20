// `E4` 的 owner 观测入口：索引目录里跨 revision 的残留必须全部计入，
// 伴生文件与符号链接不计入（I61 §3.9 / §3.10）。

use assistant_tools::os::index_stats;
use assistant_tools::os::purge_index_files;

#[test]
fn missing_index_root_is_empty_state() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("does-not-exist");

    let stats = index_stats(&root).expect("缺失目录应为空态");
    assert_eq!(stats.count, 0);
    assert_eq!(stats.bytes, 0);
    assert_eq!(stats.oldest_at, None, "无文件时两端为 None，不是 0");
    assert_eq!(stats.newest_at, None);
}

#[test]
fn cross_revision_residuals_are_all_counted() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("indexes");
    std::fs::create_dir_all(&root).unwrap();

    std::fs::write(root.join("aaaa_10.1700000000.idx"), b"1234").unwrap();
    std::fs::write(root.join("aaaa_11.1700000099.idx"), b"123456").unwrap();

    let stats = index_stats(&root).expect("扫描应成功");
    assert_eq!(stats.count, 2, "旧 revision 的残留同样占用磁盘，必须计入");
    assert_eq!(stats.bytes, 10);
    assert!(stats.oldest_at.is_some());
    assert!(stats.newest_at.is_some());
}

#[test]
fn companion_files_are_not_counted() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("indexes");
    std::fs::create_dir_all(&root).unwrap();

    std::fs::write(root.join("aaaa_10.1700000000.idx"), b"1234").unwrap();
    std::fs::write(root.join("aaaa_11.1700000099.idx.tmp"), b"partial").unwrap();

    let stats = index_stats(&root).expect("扫描应成功");
    assert_eq!(stats.count, 1, "伴生 .tmp 不计入条数");
    assert_eq!(stats.bytes, 4, "伴生 .tmp 不计入字节");
}

#[test]
fn purge_index_files_is_idempotent_and_only_removes_idx() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("indexes");
    std::fs::create_dir_all(&root).unwrap();

    std::fs::write(root.join("aaaa_10.1700000000.idx"), b"1234").unwrap();
    std::fs::write(root.join("aaaa_11.1700000099.idx"), b"123456").unwrap();
    std::fs::write(root.join("keep.txt"), b"keep").unwrap();

    let first = purge_index_files(&root);
    assert_eq!(first.removed, 2, "只删 *.idx");
    assert_eq!(first.removed_bytes, 10);
    assert_eq!(first.failed, 0);
    assert_eq!(first.skipped, 0);
    assert!(root.join("keep.txt").exists(), "非 .idx 文件不得动");
    assert!(!root.join("aaaa_10.1700000000.idx").exists());
    assert!(!root.join("aaaa_11.1700000099.idx").exists());

    let second = purge_index_files(&root);
    assert_eq!(second.removed, 0, "重复执行不得再删到东西");
    assert_eq!(second.removed_bytes, 0);
    assert_eq!(second.failed, 0);
    assert!(root.join("keep.txt").exists(), "重复执行也不动非 .idx 文件");
}

#[test]
fn purge_index_files_on_missing_root_is_empty_state() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("does-not-exist");

    let report = purge_index_files(&root);
    assert_eq!(report.removed, 0);
    assert_eq!(report.failed, 0);
    assert_eq!(report.read_failures, 0, "目录不存在是正常空态");
}

#[cfg(unix)]
#[test]
fn symlinks_are_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("indexes");
    let outside = dir.path().join("outside.idx");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&outside, b"0123456789").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("link.idx")).unwrap();

    let stats = index_stats(&root).expect("扫描应成功");
    assert_eq!(stats.count, 0, "符号链接不计入，也不使整条降级");
    assert_eq!(stats.bytes, 0);
}
