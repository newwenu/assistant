// I61 存储管理器：条目清单装配契约（V1–V5 / V13 / X15）。
//
// 这里只校验纯函数 `assemble_breakdown`：采样与 IO 都在 `sample` / owner 侧，
// 装配层不碰磁盘，因此集成测试可以完全用伪造样本驱动。
//
// 条目清单不再固定条数：core 条目恒在，可选分区条目由已注册分区自述，
// 未挂载 = 行不存在（不出现 `notImplemented` 占位行）。

use assistant_app::app_dirs::registry::{CoreEntry, MANAGED_ROOTS};
use assistant_app::cmd::storage::entries::{
    assemble_breakdown, set_sample, views, BreakdownBase, EntrySample, EntryView,
};
use assistant_app::cmd::storage::section::SectionRegistry;
use assistant_app::protocol::dto::{
    StorageAvailabilityDto, StorageBreakdownDto, StorageEligibilityDto, StorageInventoryDto,
    StorageReclaimActionDto,
};

#[path = "support/fake_section.rs"]
mod fake_section;

// 链接夹具全仓只保留一份实现（`assistant-tools` 测试树），经 `test-support` feature
// 公开为 `assistant_tools::os::test_support::link`：既避免各 crate 各写一份后在 Windows
// 侧漂成两种策略，也避免跨 package 的 `#[path]`（rust-analyzer 会误报 E0583）。
use assistant_tools::os::test_support::link::file_link;

fn base() -> BreakdownBase {
    BreakdownBase {
        total_bytes: 0,
        total_count: 0,
        orphans: Vec::new(),
        conversations: Vec::new(),
        shared_bytes: 0,
        referenced_once_bytes: 0,
    }
}

fn core_views() -> Vec<EntryView> {
    views(&[])
}

fn available(views: &[EntryView]) -> Vec<EntrySample> {
    vec![EntrySample::observed(1024, 1, Some(100), Some(200)); views.len()]
}

fn assemble(views: &[EntryView], samples: &[EntrySample]) -> StorageBreakdownDto {
    assemble_breakdown(base(), views, samples, 100, StorageInventoryDto::default())
}

#[test]
fn entries_follow_view_list_order() {
    let views = core_views();
    let samples = available(&views);
    let dto = assemble(&views, &samples);

    assert_eq!(dto.entries.len(), views.len(), "条目数与视图清单一致");
    assert_eq!(views.len(), MANAGED_ROOTS.len(), "未挂载分区不贡献行");
    for (index, entry) in dto.entries.iter().enumerate() {
        assert_eq!(
            entry.key, views[index].key,
            "第 {index} 条 key 必须等于视图（V1）"
        );
        assert_eq!(
            entry.key, MANAGED_ROOTS[index].key,
            "core 段必须与登记表同序"
        );
        assert_eq!(entry.owner, views[index].owner);
    }
}

/// V13 / V23①⑤：`vault` 构建的全量行口径——core 条目固定顺序 + 假分区自述 5 条紧随其后。
///
/// 集成层用假分区（不依赖真实 vault）；真 vault 的逐字段断言在 `assistant-vault`
/// 的分区单测与 `vault_plugin` 的映射测试里，集成层不重复。
#[test]
fn optional_section_entries_join_core_after_registry_rows() {
    let registry = SectionRegistry::new();
    registry
        .register(fake_section::mount(
            "fake-five",
            fake_section::five_entries(),
        ))
        .expect("首段命中 OPTIONAL_SEGMENTS");
    let views = views(&registry.snapshot());

    assert_eq!(views.len(), MANAGED_ROOTS.len() + 5, "core 条 + 分区 5");
    for (index, root) in MANAGED_ROOTS.iter().enumerate() {
        assert_eq!(views[index].core, Some(root.entry), "core 段按登记表顺序");
        assert_eq!(views[index].entry, root.entry.id(), "对外标签即 core id");
        assert_eq!(views[index].key, root.key);
    }
    for (offset, decl) in fake_section::five_entries().iter().enumerate() {
        let view = &views[MANAGED_ROOTS.len() + offset];
        assert_eq!(view.entry, decl.entry, "分区条目按自述顺序紧随 core");
        assert_eq!(view.key, decl.key);
        assert_eq!(view.owner, decl.owner);
    }

    let samples = available(&views);
    let dto = assemble(&views, &samples);
    assert_eq!(dto.entries.len(), views.len(), "装配与视图一一对应（X15）");
    for (offset, decl) in fake_section::five_entries().iter().enumerate() {
        let entry = &dto.entries[MANAGED_ROOTS.len() + offset];
        assert_eq!(entry.key, decl.key);
        assert_eq!(entry.owner, decl.owner);
        assert_eq!(entry.reclaim.action, decl.decl.action, "动作来自分区自述");
        assert_eq!(
            entry.reclaim.eligibility, decl.decl.eligibility,
            "静态资格来自分区自述"
        );
        assert_eq!(
            entry.reclaim.reason,
            Some(decl.decl.reason),
            "reason 用分区自述的那一个词"
        );
        assert!(!entry.includes_derived, "includesDerived 恒 false");

        // V22③ owner 命名域三条：不含 `::`、不等于 feature 名、不是可选 crate 类型名。
        assert!(!entry.owner.contains("::"), "owner 不得含路径分隔");
        assert_ne!(entry.owner, "vault", "owner 不得等于 feature 名");
        assert!(
            !entry.owner.ends_with("State"),
            "owner 不得是可选 crate 类型名"
        );
    }
}

#[test]
fn owner_failure_degrades_to_null_and_unreadable() {
    let views = core_views();
    let mut samples = available(&views);
    set_sample(
        &views,
        &mut samples,
        "E2",
        EntrySample::degraded(StorageAvailabilityDto::Unreadable),
    );

    let dto = assemble(&views, &samples);
    let entry = dto
        .entries
        .iter()
        .find(|entry| entry.key == views[1].key)
        .expect("E2 必须在清单内");

    assert_eq!(
        entry.bytes, None,
        "取不到数必须为 null，不得填 0（V2 / G6）"
    );
    assert_eq!(
        entry.count, None,
        "取不到数必须为 null，不得填 0（V2 / G6）"
    );
    assert_eq!(entry.availability, StorageAvailabilityDto::Unreadable);
    assert!(
        matches!(
            entry.availability,
            StorageAvailabilityDto::Locked
                | StorageAvailabilityDto::Unavailable
                | StorageAvailabilityDto::Unreadable
        ),
        "降级必须是异常集合中的一档（V2 / R-16）"
    );
}

#[test]
fn action_none_entries_are_never_eligible() {
    let views = core_views();
    let samples = available(&views);
    let dto = assemble(&views, &samples);

    for entry in &dto.entries {
        if entry.reclaim.action != StorageReclaimActionDto::None {
            continue;
        }
        assert!(
            matches!(
                entry.reclaim.eligibility,
                StorageEligibilityDto::NotEligible
                    | StorageEligibilityDto::OwnerManaged
                    | StorageEligibilityDto::NotImplemented
                    | StorageEligibilityDto::Locked
            ),
            "{} action = none 却给出可回收资格（V3）",
            entry.key
        );
    }
}

#[test]
fn non_eligible_entries_carry_reason_and_not_implemented_carries_note() {
    let views = core_views();
    let samples = available(&views);
    let dto = assemble(&views, &samples);

    for entry in &dto.entries {
        if entry.reclaim.eligibility == StorageEligibilityDto::Eligible {
            continue;
        }
        assert!(
            entry.reclaim.reason.is_some(),
            "{} 非 eligible 必须给 reason（V4）",
            entry.key
        );
        assert!(
            entry.reclaim.reclaimable_bytes.is_none() && entry.reclaim.reclaimable_count.is_none(),
            "{} 非 eligible 不得给可回收数字（V17）",
            entry.key
        );
        if entry.reclaim.eligibility == StorageEligibilityDto::NotImplemented {
            let note = entry.reclaim.note.as_deref().unwrap_or_default();
            assert!(
                !note.is_empty(),
                "{} notImplemented 必须给结构性缺口说明（V4 / R-39）",
                entry.key
            );
        }
    }
}

/// 未挂载分区：清单里既没有该条目，也不出现 `notImplemented` 占位行（§7.1「行不存在」）。
///
/// `E12` 的占用由应用侧只读遍历得到，不再是恒 `notImplemented` 的降级行，
/// 因此本用例对全部 core 条目一视同仁。
#[test]
fn unmounted_sections_leave_no_rows() {
    let views = core_views();
    let samples = available(&views);
    let dto = assemble(&views, &samples);

    assert_eq!(dto.entries.len(), MANAGED_ROOTS.len(), "只剩 core 条目");
    assert!(
        dto.entries
            .iter()
            .all(|entry| entry.availability != StorageAvailabilityDto::NotImplemented),
        "任何 core 条目都不得是 notImplemented 占位行"
    );
}

/// `E12` 有真实占用，但仍不参与 `complete`：日志在采样期间持续写入，
/// 数字必然滞后于真实值，无法与其他条目承诺同一时刻的快照（§6.1）。
#[test]
fn complete_ignores_only_logs_entry() {
    let views = core_views();
    let samples = available(&views);
    let dto = assemble(&views, &samples);
    assert!(dto.complete, "除 E12 外全部 available 时应为 true");

    let mut samples = available(&views);
    set_sample(
        &views,
        &mut samples,
        "E12",
        EntrySample::degraded(StorageAvailabilityDto::Unreadable),
    );
    let dto = assemble(&views, &samples);
    assert!(dto.complete, "E12 的不可用不计入 complete 判定");

    let mut samples = available(&views);
    set_sample(
        &views,
        &mut samples,
        "E5",
        EntrySample::degraded(StorageAvailabilityDto::Unavailable),
    );
    let dto = assemble(&views, &samples);
    assert!(!dto.complete, "E5 超时降级后 complete 必须为 false");
}

/// `E13` 观测验收：诊断目录只认三类产物（`panic_*.log` / `crash_*.dmp` / `crash_*.log`），
/// 无关文件不计入，符号链接不计入且不降级（I61 §3.10 E13）。
#[test]
fn e13_dump_stats_counts_only_crash_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("crashes");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("panic_20260101_000000_000.log"), b"1234").unwrap();
    std::fs::write(root.join("crash_20260101_000000_000.dmp"), b"123456").unwrap();
    std::fs::write(root.join("crash_20260101_000001_000.log"), b"12").unwrap();
    std::fs::write(root.join("notes.txt"), b"ignored").unwrap();
    std::fs::write(root.join("other.log"), b"ignored").unwrap();

    let stats = assistant_crash::dump_stats(&root).expect("扫描应成功");
    assert_eq!(stats.count, 3, "只认三类诊断产物");
    assert_eq!(stats.bytes, 4 + 6 + 2, "无关文件不计入字节");
    assert!(stats.oldest_at.is_some());
    assert!(stats.newest_at.is_some());
}

#[test]
fn e13_dump_stats_skips_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("crashes");
    std::fs::create_dir_all(&root).unwrap();
    let outside = dir.path().join("outside.log");
    std::fs::write(&outside, b"0123456789").unwrap();
    if !file_link(&outside, &root.join("panic_linked.log")) {
        eprintln!("skip e13_dump_stats_skips_symlinks: cannot create file symlink");
        return;
    }

    let stats = assistant_crash::dump_stats(&root).expect("扫描应成功");
    assert_eq!(stats.count, 0, "符号链接不计入");
    assert_eq!(stats.bytes, 0);
}

/// R-39：`note` 是唯一承载结构性缺口说明的散文通道——只写能力缺口，不写策略数值、
/// 路径、时间点或内部方法名 / 插件名。形态断言不逐字比对：改措辞不判红，删掉判红。
#[test]
fn notes_keep_structural_shape() {
    let views = core_views();
    let samples = available(&views);
    let dto = assemble(&views, &samples);

    let mut seen = 0usize;
    for entry in &dto.entries {
        let Some(note) = entry.reclaim.note.as_deref() else {
            continue;
        };
        seen += 1;
        assert!(
            note.chars().count() <= 120,
            "{} 的 note 超过 120 字：{note}",
            entry.key
        );
        for bad in [":", "/", "\\"] {
            assert!(
                !note.contains(bad),
                "{} 的 note 不得含盘符 / 路径分隔 `{bad}`：{note}",
                entry.key
            );
        }
        for internal in ["probe", "Probe", "AttachmentStore", "assistant_", "vault"] {
            assert!(
                !note.contains(internal),
                "{} 的 note 不得写内部名 / 插件名 `{internal}`：{note}",
                entry.key
            );
        }
    }

    assert!(seen >= 1, "结构性缺口说明不能整体丢失（E13 必须带 note）");
    assert!(
        dto.entries
            .iter()
            .any(|entry| entry.key == "crashes" && entry.reclaim.note.is_some()),
        "E13 的缺口说明落在 `reclaim.note`"
    );
    assert!(
        dto.entries
            .iter()
            .all(|entry| entry.key != "logs" || entry.reclaim.note.is_none()),
        "E12 有真实统计，不再需要缺口说明（策略数值禁止进 `note`）"
    );
}

/// R-39：wire 上承载字符串的只有「标识符」与「两处散文」（`note` / `message`）。
///
/// 其余字段一律不许用 `String` 承载状态语义——状态只走封闭枚举，
/// 否则前端又得靠匹配自由文本决定展示。
#[test]
fn wire_string_fields_are_identifiers_or_prose() {
    const PROSE: &[&str] = &["note", "message"];
    const IDENTIFIERS: &[&str] = &[
        "attachment_id",
        "media_type",
        "conversation_id",
        "title",
        "file_name",
        "attachments_dir",
        "mascot_assets_dir",
        "key",
        "owner",
        "name",
        "entry",
        "plan_id",
        "entry_keys",
        "truncated_nodes",
    ];

    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("crates/assistant-protocol/src/storage.rs"),
    )
    .expect("读契约源文件");

    let mut offenders: Vec<String> = Vec::new();
    let mut current = String::new();
    for raw in source.lines() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("pub struct ") {
            current = rest
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string();
            continue;
        }
        if line == "}" {
            current.clear();
            continue;
        }
        if current.is_empty() {
            continue;
        }
        let Some(rest) = line.strip_prefix("pub ") else {
            continue;
        };
        if !(rest.contains(": String")
            || rest.contains(": Option<String>")
            || rest.contains(": Vec<String>"))
        {
            continue;
        }
        let field = rest.split(':').next().unwrap_or_default().trim();
        if PROSE.contains(&field) || IDENTIFIERS.contains(&field) {
            continue;
        }
        offenders.push(format!("{current}.{field}"));
    }

    assert!(
        offenders.is_empty(),
        "wire 上新增了承载状态语义的字符串字段（只允许 note / message 承载散文）：{offenders:?}"
    );
}

// ---------------------------------------------------------------------------
// G7 / G8（I67 §4.2 / §4.3 / §7）
// ---------------------------------------------------------------------------

fn manifest_dir() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// 前端名称表 `ENTRY_NAME_KEYS` 的键集合（只读源码，不引入 TS 运行时）。
fn frontend_entry_name_keys() -> Vec<String> {
    let path = manifest_dir().join("../src/settings/pages/storage/lexicon.ts");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} 应可读取: {error}", path.display()));
    let mut inside = false;
    let mut keys = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if !inside {
            inside = line.starts_with("const ENTRY_NAME_KEYS");
            continue;
        }
        if line.starts_with("};") {
            break;
        }
        let key = line.split(':').next().unwrap_or_default().trim();
        let key = key.trim_matches('"');
        if !key.is_empty() && !key.starts_with("//") {
            keys.push(key.to_string());
        }
    }
    keys
}

/// 后端 key 未在前端名称表登记的部分（空 = 门禁通过）。
fn missing_frontend_names(views: &[EntryView]) -> Vec<&'static str> {
    let names = frontend_entry_name_keys();
    views
        .iter()
        .map(|view| view.key)
        .filter(|key| !names.iter().any(|name| name == key))
        .collect()
}

#[test]
fn g7_backend_keys_are_all_named_in_frontend() {
    // 正向门禁只覆盖 core：真实分区条目由特性 crate 自述（本集成测试不链接它们），
    // 分区侧由 `g7_new_section_key_without_frontend_name_is_red` 证明同一检查会判红。
    let all = views(&[]);
    let missing = missing_frontend_names(&all);
    assert!(
        missing.is_empty(),
        "G7：后端 key 必须在前端 ENTRY_NAME_KEYS 登记，缺失：{missing:?}"
    );
}

#[test]
fn g7_new_section_key_without_frontend_name_is_red() {
    let registry = SectionRegistry::new();
    registry
        .register(fake_section::mount(
            "fake-unlisted",
            fake_section::entries(vec![fake_section::entry(
                "X9",
                "unlisted",
                fake_section::decl(
                    StorageReclaimActionDto::None,
                    StorageEligibilityDto::NotEligible,
                    assistant_app::protocol::dto::StorageReclaimReasonDto::BusinessData,
                    assistant_app::cmd::storage::section::RiskTier::Forbidden,
                ),
            )]),
        ))
        .expect("首段命中 OPTIONAL_SEGMENTS");
    let all = views(&registry.snapshot());
    let missing = missing_frontend_names(&all);
    assert_eq!(missing.len(), 1, "V10：新增分区条目但未加前端名称必须判红");
}

fn rust_sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in
        std::fs::read_dir(dir).unwrap_or_else(|error| panic!("{} 应可读取: {error}", dir.display()))
    {
        let path = entry.expect("目录项可读").path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// 生产部分 = 第一个 `#[cfg(test)]` 之前的内容（本仓库测试模块恒在文件尾部）。
fn production_part(text: &str) -> &str {
    match text.find("#[cfg(test)]") {
        Some(index) => &text[..index],
        None => text,
    }
}

/// 提取 `"E<数字>"` 形式的字面量。
fn e_id_literals(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index + 2 < bytes.len() {
        if bytes[index] == b'"' && bytes[index + 1] == b'E' && bytes[index + 2].is_ascii_digit() {
            let mut end = index + 2;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end < bytes.len() && bytes[end] == b'"' {
                out.push(&text[index..=end]);
            }
            index = end;
        } else {
            index += 1;
        }
    }
    out
}

#[test]
fn g8_e_literals_only_live_in_registry() {
    let mut files = Vec::new();
    rust_sources(&manifest_dir().join("src"), &mut files);
    let registry_file = manifest_dir().join("src/app_dirs/registry.rs");

    let mut offenders: Vec<String> = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file)
            .unwrap_or_else(|error| panic!("{} 应可读取: {error}", file.display()));
        let literals = e_id_literals(production_part(&text));
        if literals.is_empty() {
            continue;
        }
        if file == &registry_file {
            continue;
        }
        offenders.push(format!("{}: {literals:?}", file.display()));
    }
    assert!(
        offenders.is_empty(),
        "G8：`E` 字面量只允许出现在登记表（registry.rs）与测试：{offenders:?}"
    );

    let registry_text = std::fs::read_to_string(&registry_file).expect("registry.rs 可读");
    let literals = e_id_literals(production_part(&registry_text));
    let expected: Vec<&str> = CoreEntry::ALL.iter().map(|entry| entry.id()).collect();
    let mut quoted: Vec<String> = expected.iter().map(|id| format!("\"{id}\"")).collect();
    quoted.sort();
    let mut found: Vec<String> = literals.iter().map(|literal| literal.to_string()).collect();
    found.sort();
    assert_eq!(
        found, quoted,
        "G8：登记表里的 `E` 字面量必须与 CoreEntry::ALL 一一对应（单点定义）"
    );
}
