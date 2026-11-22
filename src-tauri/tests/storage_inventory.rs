// I61 对账余量（§3.11）：V18 / V20 前半 / 有界 / `failed` 保留。
//
// 这里只校验 `cmd::storage::inventory` 的纯逻辑与快照状态机：扫描走真实临时目录，
// 但所有断言都不碰 owner 数字，因此不需要 `AppHandle`，也不需要任何回收入口。
// V20 后半（白名单负例，`reclaim_storage` 拒绝 `inventory` 节点名）留给 T2-d。

use std::fs;
use std::path::Path;

use assistant_app::app_dirs::registry::{self, RootScope, MANAGED_ROOTS};
use assistant_app::app_dirs::AppDirs;
use assistant_app::cmd::storage::entries::{
    assemble_breakdown, set_sample, views, BreakdownBase, EntrySample, EntryView,
};
use assistant_app::cmd::storage::inventory::{
    scan, InventoryCache, RawNode, ScanResult, ScanTotals,
};
use assistant_app::cmd::storage::section::SectionRegistry;
use assistant_app::protocol::dto::{
    StorageInventoryClassDto, StorageInventoryDto, StorageInventoryStatusDto, StorageLayerDto,
};

#[path = "support/fake_section.rs"]
mod fake_section;

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

fn core_views() -> Vec<EntryView> {
    views(&[])
}

fn available(views: &[EntryView]) -> Vec<EntrySample> {
    vec![EntrySample::observed(1_024, 1, Some(100), Some(200)); views.len()]
}

fn missing(views: &[EntryView]) -> Vec<EntrySample> {
    vec![EntrySample::missing(); views.len()]
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
            totals: ScanTotals::default(),
            totals_partial: false,
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
    fs::create_dir_all(data.join(registry::OPTIONAL_SEGMENTS[0])).expect("建可选首段");
    fs::create_dir_all(data.join("stray")).expect("建 stray");
    fs::create_dir_all(local.join("logs")).expect("建 logs");
    write_file(&local.join("logs").join("app.log"), 7);
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
    let views = core_views();
    let result = scan(&dirs, &views).expect("扫描临时基根");

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
    assert_eq!(
        (declared[1].scan_bytes, declared[1].scan_count),
        (Some(7), Some(1)),
        "inventory = false 的条目仍要观测：计入分母、只关减法（I67 §3.5）"
    );
    assert!(!declared[1].truncated, "未触界不得伪装成触界");
    assert_eq!(optional[0].name, registry::OPTIONAL_SEGMENTS[0]);
    assert_eq!(
        optional[0].entry, None,
        "未挂载分区的条目在 core 侧不存在，不得挂条目编号"
    );
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
fn totals_carry_denominator_and_unreconciled_bucket() {
    let (_tmp, dirs) = seeded();
    let views = core_views();
    let result = scan(&dirs, &views).expect("扫描临时基根");

    assert_eq!(
        result.totals.data.scan_bytes,
        Some(28),
        "分母含 E12 与未登记项"
    );
    assert_eq!(result.totals.data.scan_count, Some(5));
    assert_eq!(
        result.totals.local.scan_bytes,
        Some(7),
        "E12 日志目录计入分母"
    );
    assert_eq!(
        result.totals.data.unnameable_bytes,
        Some(0),
        "空桶是 0，不是 null"
    );
    assert!(!result.totals_partial);

    let cache = InventoryCache::new();
    let samples = available(&views);
    let nodes = result.nodes.clone();
    cache.finish(result, 1);
    let (dto, _) = cache.read(1, &views, &samples);

    assert_eq!(dto.totals.scan_bytes, Some(35), "两个 scope 之和（§3.7）");
    assert_eq!(dto.totals.data.unregistered_bytes, Some(8));
    assert_eq!(dto.totals.data.unregistered_count, Some(2));
    assert_eq!(dto.totals.local.unreconciled_bytes, Some(7));
    assert_eq!(dto.totals.local.unreconciled_count, Some(1));
    assert!(!dto.totals.partial, "全部节点可观测时不是下界");
    assert_eq!(dto.nodes.len(), nodes.len());
}

#[test]
fn never_run_totals_are_all_null() {
    let views = core_views();
    let samples = missing(&views);
    let cache = InventoryCache::new();
    let (dto, trigger) = cache.read(0, &views, &samples);
    assert_eq!(dto.status, StorageInventoryStatusDto::NeverRun);
    assert!(trigger);
    assert_eq!(dto.totals, Default::default(), "从未扫描：内部数字全 null");
}

#[test]
fn declared_node_subtracts_only_declared_samples() {
    let cache = InventoryCache::new();
    let views = core_views();
    let mut samples = missing(&views);
    set_sample(
        &views,
        &mut samples,
        "E1",
        EntrySample::observed(4, 1, None, None),
    );
    set_sample(
        &views,
        &mut samples,
        "E2",
        EntrySample::observed(6, 1, None, None),
    );

    finish(
        &cache,
        vec![node(
            "attachments",
            StorageInventoryClassDto::Declared,
            Some(10),
        )],
        100,
    );
    let (dto, trigger) = cache.read(100, &views, &samples);
    let declared = &dto.nodes[0];

    assert_eq!(declared.declared_bytes, Some(10), "E1 + E2 都落在首段内");
    assert_eq!(declared.declared_count, Some(2));
    assert_eq!(declared.residual_bytes, Some(0));
    assert_eq!(declared.residual_count, Some(0));
    assert!(!declared.over_registered);
    assert!(!trigger, "fresh 快照不触发后台扫描");

    let mut samples = missing(&views);
    set_sample(
        &views,
        &mut samples,
        "E1",
        EntrySample::observed(7, 1, None, None),
    );
    set_sample(
        &views,
        &mut samples,
        "E2",
        EntrySample::observed(6, 1, None, None),
    );
    finish(
        &cache,
        vec![node(
            "attachments",
            StorageInventoryClassDto::Declared,
            Some(10),
        )],
        100,
    );
    let (dto, _) = cache.read(100, &views, &samples);
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
    let views = core_views();
    finish(
        &cache,
        vec![node(
            "stray",
            StorageInventoryClassDto::Unregistered,
            Some(8),
        )],
        100,
    );
    let (dto, _) = cache.read(100, &views, &missing(&views));
    let stray = &dto.nodes[0];

    assert_eq!(stray.declared_bytes, Some(0), "未登记节点没有减数");
    assert_eq!(stray.residual_bytes, Some(8), "余量就是扫到的那份");
    assert_eq!(dto.unregistered_bytes, Some(8));
    assert_eq!(dto.unregistered_count, Some(1));
    assert_eq!(stray.scope, StorageLayerDto::Data);
}

#[test]
fn running_and_failed_states_leave_entries_untouched() {
    let views = core_views();
    let samples = available(&views);
    let cache = InventoryCache::new();

    assert!(cache.begin(), "首次触发应真正开跑");
    let (running, trigger) = cache.read(100, &views, &samples);
    assert_eq!(running.status, StorageInventoryStatusDto::Running);
    assert!(!trigger, "running 期间不得再触发（single-flight）");
    assert!(running.nodes.is_empty(), "running 下没有数字可回");
    assert!(!cache.begin(), "running 期间重复触发直接复用");

    let never_run = StorageInventoryDto::default();
    let running_dto = assemble_breakdown(base(), &views, &samples, 100, running.clone());
    let baseline = assemble_breakdown(base(), &views, &samples, 100, never_run);

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
    let (failed, _) = cache.read(200, &views, &samples);
    assert_eq!(failed.status, StorageInventoryStatusDto::Failed);
    assert_eq!(failed.failed_at, Some(200));
    let failed_dto = assemble_breakdown(base(), &views, &samples, 100, failed.clone());

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
    let views = core_views();
    let result = scan(&dirs, &views).expect("扫描临时基根");

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
    let views = core_views();
    finish(
        &cache,
        vec![node(
            "stray",
            StorageInventoryClassDto::Unregistered,
            Some(100),
        )],
        100,
    );

    let (fresh, _) = cache.read(100, &views, &missing(&views));
    assert_eq!(fresh.status, StorageInventoryStatusDto::Fresh);
    assert_eq!(fresh.scanned_at, Some(100));
    assert_eq!(fresh.nodes[0].scan_bytes, Some(100));

    assert!(cache.begin());
    cache.fail(200);

    let (failed, _) = cache.read(200, &views, &missing(&views));
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
    let views = core_views();
    finish(
        &cache,
        vec![node(
            "stray",
            StorageInventoryClassDto::Unregistered,
            Some(100),
        )],
        100,
    );

    let (fresh, trigger) = cache.read(100, &views, &missing(&views));
    assert!(!trigger);
    assert_eq!(fresh.nodes[0].scan_bytes, Some(100));

    let (stale, trigger) = cache.read(100 + 300, &views, &missing(&views));
    assert_eq!(stale.status, StorageInventoryStatusDto::Stale);
    assert_eq!(stale.nodes[0].scan_bytes, Some(100), "stale 返回旧值");
    assert!(trigger, "stale 必须触发一次后台扫描");

    assert!(cache.begin());
    let (running, trigger) = cache.read(100 + 300, &views, &missing(&views));
    assert_eq!(running.status, StorageInventoryStatusDto::Running);
    assert!(!trigger, "已开跑则不再触发");
}

#[test]
fn registry_keeps_all_core_entries() {
    let views = core_views();
    assert_eq!(views.len(), MANAGED_ROOTS.len(), "未挂载分区不贡献行");
    for (index, root) in MANAGED_ROOTS.iter().enumerate() {
        assert_eq!(views[index].core, Some(root.entry));
        assert_eq!(views[index].key, root.key);
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
    let views = core_views();
    let result = scan(&dirs, &views).expect("扫描临时基根");

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

/// V19 / V23：可选首段四态——槽缺失 / 零分区 / 单分区 / 双分区。
///
/// 判定只看「有没有分区自述该首段」，不看能力名 / feature 名（V22）。
#[test]
fn optional_segment_states_follow_registered_sections() {
    let (_tmp, dirs) = seeded();
    let segment = registry::OPTIONAL_SEGMENTS[0];
    let core_len = core_views().len();

    // 槽缺失 / 零分区：清单只有 core 条目，首段整体归 optionalNotMounted。
    for sections in [Vec::new(), SectionRegistry::new().snapshot()] {
        let entry_views = views(&sections);
        assert_eq!(entry_views.len(), core_len, "零分区不贡献行");
        let result = scan(&dirs, &entry_views).expect("扫描临时基根");
        let optional: Vec<&RawNode> = result
            .nodes
            .iter()
            .filter(|node| node.class == StorageInventoryClassDto::OptionalNotMounted)
            .collect();
        assert_eq!(optional.len(), 1, "可选首段整体归未挂载");
        assert_eq!(optional[0].name, segment);
        assert_eq!(optional[0].entry, None, "未挂载不挂条目编号");
    }

    // 单分区：自述 5 条进入清单，首段转为 declared。
    let registry = SectionRegistry::new();
    registry
        .register(fake_section::mount(
            "fake-five",
            fake_section::five_entries(),
        ))
        .expect("首段命中 OPTIONAL_SEGMENTS");
    let entry_views = views(&registry.snapshot());
    assert_eq!(entry_views.len(), core_len + 5, "core 条 + 分区 5");
    let result = scan(&dirs, &entry_views).expect("扫描临时基根");
    let declared = result
        .nodes
        .iter()
        .find(|node| node.name == segment)
        .expect("首段必须出现");
    assert_eq!(declared.class, StorageInventoryClassDto::Declared);
    assert_eq!(
        declared.entry,
        Some(fake_section::five_entries()[0].entry),
        "首段挂分区自述的首条条目编号"
    );

    // 双分区：同一首段下两个分区，条目累加。
    let registry = SectionRegistry::new();
    registry
        .register(fake_section::mount(
            "fake-pair",
            fake_section::pair_entries(),
        ))
        .expect("首个分区");
    registry
        .register(fake_section::mount(
            "fake-triple",
            fake_section::triple_entries(),
        ))
        .expect("第二个分区");
    let entry_views = views(&registry.snapshot());
    assert_eq!(entry_views.len(), core_len + 5, "2 + 3 条分区条目");
    let result = scan(&dirs, &entry_views).expect("扫描临时基根");
    let declared = result
        .nodes
        .iter()
        .find(|node| node.name == segment)
        .expect("首段必须出现");
    assert_eq!(declared.class, StorageInventoryClassDto::Declared);
    assert_eq!(
        declared.entry,
        Some(fake_section::pair_entries()[0].entry),
        "双分区首段挂第一个分区自述的首条条目编号"
    );
}

/// 首段不在 `OPTIONAL_SEGMENTS` 的假分区：注册被拒，且不进入清单。
#[test]
fn foreign_section_is_rejected_and_absent_from_views() {
    let registry = SectionRegistry::new();
    let error = registry
        .register(fake_section::foreign_handle())
        .expect_err("首段不在 OPTIONAL_SEGMENTS 必须被拒");
    assert!(error.contains("OPTIONAL_SEGMENTS"), "{error}");
    let entry_views = views(&registry.snapshot());
    assert_eq!(entry_views.len(), core_views().len(), "被拒分区不贡献行");
    assert!(!entry_views.iter().any(|view| view.key == "elsewhere"));
}

/// V13（T6）：core 侧三个配置文件登记后从 `unregistered` 移入 `declared`，
/// 且 `declaredBytes == scanBytes`、`residualBytes == 0`。
///
/// 首段名一律从登记表取（`registry::find_by_entry`），测试里不写文件名/条目字面量。
/// `E16` 不在此列：它是可选特性（vault）分区自注册的条目，核心侧不持有其知识。
#[test]
fn config_files_are_declared_and_reconciled() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let data = tmp.path().join("data");
    let local = tmp.path().join("local");
    fs::create_dir_all(&data).expect("建 data");
    fs::create_dir_all(&local).expect("建 local");

    let mut written = Vec::new();
    for (offset, id) in ["E15", "E17", "E18"].into_iter().enumerate() {
        let root = registry::find_by_entry(id).expect("配置条目必须已登记");
        assert_eq!(root.scope, RootScope::Data, "{id} 落在数据基根");
        let len = 11 + offset * 2;
        write_file(&data.join(root.key), len);
        written.push((root.key, u64::try_from(len).expect("len fits u64")));
    }

    let dirs = AppDirs::with_bases(data, local);
    let views = core_views();
    let result = scan(&dirs, &views).expect("扫描临时基根");

    let declared: Vec<&RawNode> = result
        .nodes
        .iter()
        .filter(|node| node.class == StorageInventoryClassDto::Declared)
        .collect();
    assert_eq!(declared.len(), 3, "三个配置文件全部归 declared");
    for (key, _) in &written {
        let node = declared
            .iter()
            .find(|node| node.name == *key)
            .expect("每个配置文件名都必须有观测项");
        assert!(node.entry.is_some(), "declared 节点必须挂条目编号");
    }
    assert!(
        !result
            .nodes
            .iter()
            .any(|node| node.class == StorageInventoryClassDto::Unregistered),
        "配置文件的字节不得再计入 unregistered（V13）"
    );
    let total: u64 = written.iter().map(|(_, bytes)| bytes).sum();
    assert_eq!(
        result.totals.data.scan_bytes,
        Some(total),
        "扫描分母只含这三个已登记文件"
    );

    let mut samples = missing(&views);
    for (id, (_, bytes)) in ["E15", "E17", "E18"].into_iter().zip(&written) {
        set_sample(
            &views,
            &mut samples,
            id,
            EntrySample::observed(*bytes, 1, None, None),
        );
    }
    let cache = InventoryCache::new();
    cache.finish(result, 1);
    let (dto, _) = cache.read(1, &views, &samples);

    assert_eq!(dto.nodes.len(), 3, "三个配置节点");
    for node in &dto.nodes {
        assert_eq!(
            node.declared_bytes, node.scan_bytes,
            "{} 的登记值必须等于扫描值（R1）",
            node.name
        );
        assert_eq!(node.residual_bytes, Some(0), "{} 余量为 0", node.name);
        assert!(
            !node.over_registered,
            "{} 不得出现登记值大于扫描值",
            node.name
        );
    }
}
