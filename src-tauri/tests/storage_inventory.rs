// I61 对账余量（§3.11）：V18 / V20 前半 / 有界 / `failed` 保留。
//
// 这里只校验 `cmd::storage::inventory` 的纯逻辑与快照状态机：扫描走真实临时目录，
// 但所有断言都不碰 owner 数字，因此不需要 `AppHandle`，也不需要任何回收入口。
// V20 后半（白名单负例，`reclaim_storage` 拒绝 `inventory` 节点名）留给 T2-d。

use std::fs;
use std::path::Path;

use assistant_app::app_dirs::registry::{RootScope, MANAGED_ROOTS};
use assistant_app::app_dirs::AppDirs;
use assistant_app::cmd::storage::entries::{
    assemble_breakdown, BreakdownBase, EntrySample, ENTRY_COUNT,
};
use assistant_app::cmd::storage::inventory::{scan, InventoryCache, RawNode, ScanResult};
use assistant_app::protocol::dto::{
    StorageInventoryClassDto, StorageInventoryDto, StorageInventoryStatusDto, StorageLayerDto,
};

fn base() -> BreakdownBase {
    BreakdownBase {
        total_bytes: 3_072,
        total_count: 4,
        orphans: Vec::new(),
        conversations: Vec::new(),
        shared_bytes: 1_024,
        referenced_once_bytes: 2_048,
    }
}

fn available() -> [EntrySample; ENTRY_COUNT] {
    std::array::from_fn(|_| EntrySample::observed(1_024, 1, Some(100), Some(200)))
}

fn missing() -> [EntrySample; ENTRY_COUNT] {
    std::array::from_fn(|_| EntrySample::missing())
}

fn write_file(path: &Path, len: usize) {
    fs::write(path, vec![0u8; len]).expect("写测试文件");
}

fn node(name: &str, class: StorageInventoryClassDto, scan: Option<u64>) -> RawNode {
    RawNode {
        name: name.to_string(),
        scope: RootScope::Data,
        class,
        entry: None,
        scan_bytes: scan,
        scan_count: scan.map(|_| 1),
        children: Vec::new(),
        truncated: false,
    }
}

fn finish(cache: &InventoryCache, nodes: Vec<RawNode>, at: i64) {
    cache.finish(
        ScanResult {
            nodes,
            partial: false,
            truncated: Vec::new(),
        },
        at,
    );
}

/// 首层六个观测项：declared ×2 / optionalNotMounted / ignored / companion / unregistered。
fn seeded() -> (tempfile::TempDir, AppDirs) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let data = tmp.path().join("data");
    let local = tmp.path().join("local");
    fs::create_dir_all(data.join("attachments")).expect("建 attachments");
    fs::create_dir_all(data.join("security")).expect("建 security");
    fs::create_dir_all(data.join("stray")).expect("建 stray");
    fs::create_dir_all(local.join("logs")).expect("建 logs");
    write_file(&data.join("attachments").join("a.bin"), 10);
    write_file(&data.join("stray").join("a.bin"), 3);
    write_file(&data.join("stray").join("b.bin"), 5);
    write_file(&data.join("Thumbs.db"), 4);
    write_file(&data.join("assistant.db-wal"), 6);
    let dirs = AppDirs::with_bases(data, local);
    (tmp, dirs)
}

#[test]
fn conservation_classifies_every_first_level_entry() {
    let (_tmp, dirs) = seeded();
    let result = scan(&dirs, &[]).expect("扫描临时基根");

    assert_eq!(result.nodes.len(), 6, "六个首层观测项必须逐项分类");
    let declared: Vec<&RawNode> = result
        .nodes
        .iter()
        .filter(|node| node.class == StorageInventoryClassDto::Declared)
        .collect();
    let optional: Vec<&RawNode> = result
        .nodes
        .iter()
        .filter(|node| node.class == StorageInventoryClassDto::OptionalNotMounted)
        .collect();
    let ignored: Vec<&RawNode> = result
        .nodes
        .iter()
        .filter(|node| node.class == StorageInventoryClassDto::Ignored)
        .collect();
    let companion: Vec<&RawNode> = result
        .nodes
        .iter()
        .filter(|node| node.class == StorageInventoryClassDto::Companion)
        .collect();
    let unregistered: Vec<&RawNode> = result
        .nodes
        .iter()
        .filter(|node| node.class == StorageInventoryClassDto::Unregistered)
        .collect();

    assert_eq!(
        declared.len() + optional.len() + ignored.len() + companion.len() + unregistered.len(),
        result.nodes.len(),
        "守恒：五类之和等于首层观测条目数（V18）"
    );
    assert_eq!((declared.len(), optional.len()), (2, 1));
    assert_eq!(
        (ignored.len(), companion.len(), unregistered.len()),
        (1, 1, 1)
    );

    assert_eq!(declared[0].name, "attachments", "登记首段按登记表顺序在前");
    assert_eq!(declared[0].entry, Some("E1"));
    assert_eq!(declared[1].name, "logs");
    assert_eq!(declared[1].entry, Some("E12"));
    assert_eq!(declared[1].scope, RootScope::Local);
    assert_eq!(optional[0].name, "security");
    assert_eq!(optional[0].entry, Some("E7"));
    assert_eq!(ignored[0].name, "Thumbs.db");
    assert_eq!(companion[0].name, "assistant.db-wal");

    let stray = unregistered[0];
    assert_eq!(stray.name, "stray");
    assert_eq!(stray.scan_bytes, Some(8), "字节与条数同一次遍历产出");
    assert_eq!(stray.scan_count, Some(2));
    assert_eq!(stray.children.len(), 2, "未登记目录给单段名明细");
    assert!(
        stray.children.iter().all(|item| !item.name.contains('/')),
        "明细项只允许单段名（§3.11.3）"
    );
}

#[test]
fn declared_node_subtracts_only_declared_samples() {
    let cache = InventoryCache::new();
    let mut samples = missing();
    samples[0] = EntrySample::observed(4, 1, None, None);
    samples[1] = EntrySample::observed(6, 1, None, None);

    finish(
        &cache,
        vec![node(
            "attachments",
            StorageInventoryClassDto::Declared,
            Some(10),
        )],
        100,
    );
    let (dto, trigger) = cache.read(100, &samples);
    let declared = &dto.nodes[0];

    assert_eq!(declared.declared_bytes, Some(10), "E1 + E2 都落在首段内");
    assert_eq!(declared.declared_count, Some(2));
    assert_eq!(declared.residual_bytes, Some(0));
    assert_eq!(declared.residual_count, Some(0));
    assert!(!declared.over_registered);
    assert!(!trigger, "fresh 快照不触发后台扫描");

    let mut samples = missing();
    samples[0] = EntrySample::observed(7, 1, None, None);
    samples[1] = EntrySample::observed(6, 1, None, None);
    finish(
        &cache,
        vec![node(
            "attachments",
            StorageInventoryClassDto::Declared,
            Some(10),
        )],
        100,
    );
    let (dto, _) = cache.read(100, &samples);
    assert!(
        dto.nodes[0].over_registered,
        "登记值大于扫描值必须报警（§3.11.3）"
    );
    assert_eq!(dto.nodes[0].residual_bytes, Some(0), "余量不得为负");
    assert!(dto.partial, "overRegistered 下沉为 partial");
}

#[test]
fn unregistered_node_has_zero_declared_and_residual_equals_scan() {
    let cache = InventoryCache::new();
    finish(
        &cache,
        vec![node(
            "stray",
            StorageInventoryClassDto::Unregistered,
            Some(8),
        )],
        100,
    );
    let (dto, _) = cache.read(100, &missing());
    let stray = &dto.nodes[0];

    assert_eq!(stray.declared_bytes, Some(0), "未登记节点没有减数");
    assert_eq!(stray.residual_bytes, Some(8), "余量就是扫到的那份");
    assert_eq!(dto.unregistered_bytes, Some(8));
    assert_eq!(dto.unregistered_count, Some(1));
    assert_eq!(stray.scope, StorageLayerDto::Data);
}

#[test]
fn running_and_failed_states_leave_entries_untouched() {
    let samples = available();
    let cache = InventoryCache::new();

    assert!(cache.begin(), "首次触发应真正开跑");
    let (running, trigger) = cache.read(100, &samples);
    assert_eq!(running.status, StorageInventoryStatusDto::Running);
    assert!(!trigger, "running 期间不得再触发（single-flight）");
    assert!(running.nodes.is_empty(), "running 下没有数字可回");
    assert!(!cache.begin(), "running 期间重复触发直接复用");

    let never_run = StorageInventoryDto::default();
    let running_dto = assemble_breakdown(base(), &samples, 100, running.clone());
    let baseline = assemble_breakdown(base(), &samples, 100, never_run);

    finish(
        &cache,
        vec![node(
            "stray",
            StorageInventoryClassDto::Unregistered,
            Some(8),
        )],
        100,
    );
    cache.begin();
    cache.fail(200);
    let (failed, _) = cache.read(200, &samples);
    assert_eq!(failed.status, StorageInventoryStatusDto::Failed);
    assert_eq!(failed.failed_at, Some(200));
    let failed_dto = assemble_breakdown(base(), &samples, 100, failed.clone());

    assert_eq!(
        running_dto.entries, baseline.entries,
        "V20：running 不改 entries"
    );
    assert_eq!(
        failed_dto.entries, baseline.entries,
        "V20：failed 不改 entries"
    );
    assert_eq!(running_dto.total_bytes, baseline.total_bytes);
    assert_eq!(failed_dto.total_bytes, baseline.total_bytes);
    assert_eq!(
        running_dto.total_bytes,
        running_dto.shared_bytes + running_dto.referenced_once_bytes,
        "账本恒等式与扫描状态无关（V5）"
    );
}

#[test]
fn bounded_scan_truncates_and_keeps_other_nodes() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let data = tmp.path().join("data");
    let local = tmp.path().join("local");
    fs::create_dir_all(data.join("attachments")).expect("建 attachments");
    write_file(&data.join("attachments").join("a.bin"), 10);

    let mut deep = data.join("deep");
    fs::create_dir_all(&deep).expect("建 deep");
    for level in 0..12 {
        deep = deep.join(format!("d{level}"));
        fs::create_dir_all(&deep).expect("建深目录");
    }
    write_file(&deep.join("bottom.bin"), 4);

    let dirs = AppDirs::with_bases(data, local);
    let result = scan(&dirs, &[]).expect("扫描临时基根");

    let deep_node = result
        .nodes
        .iter()
        .find(|node| node.name == "deep")
        .expect("deep 必须出现");
    assert!(deep_node.truncated, "超过深度上限必须触界");
    assert_eq!(deep_node.scan_bytes, None, "触界后不给残缺数字");
    assert_eq!(deep_node.scan_count, None);

    let attachments = result
        .nodes
        .iter()
        .find(|node| node.name == "attachments")
        .expect("attachments 必须出现");
    assert_eq!(
        (attachments.scan_bytes, attachments.scan_count),
        (Some(10), Some(1)),
        "有界只降级触界节点，其余节点仍出数"
    );
    assert_eq!(attachments.entry, Some("E1"));

    assert!(result.partial, "触界必须置 partial");
    assert!(
        result.truncated.contains(&"deep".to_string()),
        "触界节点进 truncatedNodes"
    );
}

#[test]
fn failed_keeps_previous_numbers() {
    let cache = InventoryCache::new();
    finish(
        &cache,
        vec![node(
            "stray",
            StorageInventoryClassDto::Unregistered,
            Some(100),
        )],
        100,
    );

    let (fresh, _) = cache.read(100, &missing());
    assert_eq!(fresh.status, StorageInventoryStatusDto::Fresh);
    assert_eq!(fresh.scanned_at, Some(100));
    assert_eq!(fresh.nodes[0].scan_bytes, Some(100));

    assert!(cache.begin());
    cache.fail(200);

    let (failed, _) = cache.read(200, &missing());
    assert_eq!(failed.status, StorageInventoryStatusDto::Failed);
    assert_eq!(failed.failed_at, Some(200));
    assert_eq!(failed.scanned_at, Some(100), "失败保留上次成功时刻");
    assert_eq!(
        failed.nodes[0].scan_bytes,
        Some(100),
        "失败保留上次成功的数字"
    );
    assert_eq!(failed.unregistered_bytes, Some(100));

    let status = cache.status(200);
    assert_eq!(status.status, StorageInventoryStatusDto::Failed);
    assert_eq!(status.scanned_at, Some(100));
}

#[test]
fn stale_snapshot_returns_old_numbers_and_triggers_once() {
    let cache = InventoryCache::new();
    finish(
        &cache,
        vec![node(
            "stray",
            StorageInventoryClassDto::Unregistered,
            Some(100),
        )],
        100,
    );

    let (fresh, trigger) = cache.read(100, &missing());
    assert!(!trigger);
    assert_eq!(fresh.nodes[0].scan_bytes, Some(100));

    let (stale, trigger) = cache.read(100 + 300, &missing());
    assert_eq!(stale.status, StorageInventoryStatusDto::Stale);
    assert_eq!(stale.nodes[0].scan_bytes, Some(100), "stale 返回旧值");
    assert!(trigger, "stale 必须触发一次后台扫描");

    assert!(cache.begin());
    let (running, trigger) = cache.read(100 + 300, &missing());
    assert_eq!(running.status, StorageInventoryStatusDto::Running);
    assert!(!trigger, "已开跑则不再触发");
}

#[test]
fn registry_keeps_thirteen_entries() {
    assert_eq!(MANAGED_ROOTS.len(), ENTRY_COUNT);
    for (index, root) in MANAGED_ROOTS.iter().enumerate() {
        assert_eq!(root.entry, format!("E{}", index + 1));
    }
}

/// V21（管理器侧）：伴生后缀大小写不敏感。
///
/// owner 侧判定一律先 `to_ascii_lowercase()`，`FOO.WAL` 在 owner 侧被跳过；
/// 对账扫描若按原样比对，同名文件会落进 `unregistered`（§3.11.3 主信号），
/// 变成「未登记」假告警。这里用真实临时目录锁死「两侧同一结论」。
#[test]
fn uppercase_companion_suffix_is_not_unregistered() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let data = tmp.path().join("data");
    let local = tmp.path().join("local");
    fs::create_dir_all(&data).expect("建 data");
    fs::create_dir_all(&local).expect("建 local");
    write_file(&data.join("assistant.db-WAL"), 7);
    write_file(&data.join("assistant.db-SHM"), 3);
    let dirs = AppDirs::with_bases(data, local);

    let result = scan(&dirs, &[]).expect("扫描临时基根");

    assert_eq!(result.nodes.len(), 2, "两个大写伴生文件都必须出观测项");
    for scanned in &result.nodes {
        assert_eq!(
            scanned.class,
            StorageInventoryClassDto::Companion,
            "{} 必须与 owner 侧同判 companion",
            scanned.name
        );
        assert!(
            scanned.entry.is_none(),
            "伴生项不得挂任何条目编号（§3.11.3）"
        );
    }
    assert!(
        result
            .nodes
            .iter()
            .all(|scanned| scanned.class != StorageInventoryClassDto::Unregistered),
        "大写伴生文件不得产生「未登记」假告警"
    );
}
