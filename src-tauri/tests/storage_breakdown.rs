// I61 存储管理器：13 条条目的装配契约（V1–V5 / V13）。
//
// 这里只校验纯函数 `assemble_breakdown`：采样与 IO 都在 `sample` / owner 侧，
// 装配层不碰磁盘，因此集成测试可以完全用伪造样本驱动。

use assistant_app::app_dirs::registry::MANAGED_ROOTS;
use assistant_app::cmd::storage::entries::{
    assemble_breakdown, unmounted_probe_sample, BreakdownBase, EntrySample, ENTRY_COUNT,
};
use assistant_app::protocol::dto::{
    StorageAvailabilityDto, StorageEligibilityDto, StorageInventoryDto, StorageReclaimActionDto,
    StorageReclaimReasonDto,
};

// 链接夹具全仓只保留一份实现（`assistant-tools` 测试树），各 crate 用 `#[path]` 引入，
// 避免各写一份后在 Windows 侧漂成两种策略。
#[path = "../crates/assistant-tools/tests/support/link.rs"]
mod link;

use link::file_link;

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

fn available() -> [EntrySample; ENTRY_COUNT] {
    std::array::from_fn(|_| EntrySample::observed(1024, 1, Some(100), Some(200)))
}

#[test]
fn entries_follow_registry_order() {
    let dto = assemble_breakdown(base(), &available(), 100, StorageInventoryDto::default());

    assert_eq!(dto.entries.len(), ENTRY_COUNT, "13 条恒存在");
    for (index, entry) in dto.entries.iter().enumerate() {
        let root = &MANAGED_ROOTS[index];
        assert_eq!(
            entry.key, root.key,
            "第 {index} 条 key 必须等于登记表（V1）"
        );
        assert_eq!(
            entry.owner, root.owner,
            "第 {index} 条 owner 必须等于登记表"
        );
    }
}

#[test]
fn owner_failure_degrades_to_null_and_unreadable() {
    let mut samples = available();
    samples[1] = EntrySample::degraded(StorageAvailabilityDto::Unreadable, "io");

    let dto = assemble_breakdown(base(), &samples, 100, StorageInventoryDto::default());
    let entry = &dto.entries[1];

    assert_eq!(
        entry.bytes, None,
        "取不到数必须为 null，不得填 0（V2 / G6）"
    );
    assert_eq!(
        entry.count, None,
        "取不到数必须为 null，不得填 0（V2 / G6）"
    );
    assert_eq!(entry.availability, StorageAvailabilityDto::Unreadable);
    assert!(entry.availability_reason.is_some(), "降级必须给原因");
}

#[test]
fn action_none_entries_are_never_eligible() {
    let dto = assemble_breakdown(base(), &available(), 100, StorageInventoryDto::default());

    for (index, entry) in dto.entries.iter().enumerate() {
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
            "E{} action = none 却给出可回收资格（V3）",
            index + 1
        );
    }
}

#[test]
fn non_eligible_entries_carry_reason_and_not_implemented_carries_note() {
    let dto = assemble_breakdown(base(), &available(), 100, StorageInventoryDto::default());

    for (index, entry) in dto.entries.iter().enumerate() {
        if entry.reclaim.eligibility == StorageEligibilityDto::Eligible {
            continue;
        }
        assert!(
            entry.reclaim.reason.is_some(),
            "E{} 非 eligible 必须给 reason（V4）",
            index + 1
        );
        assert!(
            entry.reclaim.reclaimable_bytes.is_none() && entry.reclaim.reclaimable_count.is_none(),
            "E{} 非 eligible 不得给可回收数字（V17）",
            index + 1
        );
        if entry.reclaim.eligibility == StorageEligibilityDto::NotImplemented {
            let note = entry.reclaim.note.as_deref().unwrap_or_default();
            assert!(
                !note.is_empty(),
                "E{} notImplemented 必须点名缺的方法（V4）",
                index + 1
            );
        }
    }
}

#[test]
fn unmounted_probe_keeps_e7_to_e11_visible_and_not_reclaimable() {
    let samples: [EntrySample; ENTRY_COUNT] = std::array::from_fn(|index| {
        if (6..=10).contains(&index) {
            unmounted_probe_sample()
        } else {
            EntrySample::observed(1024, 1, Some(100), Some(200))
        }
    });

    let dto = assemble_breakdown(base(), &samples, 100, StorageInventoryDto::default());

    assert_eq!(dto.entries.len(), ENTRY_COUNT, "两种构建 entries 恒 13 条");
    for (index, entry) in dto.entries.iter().enumerate().take(11).skip(6) {
        assert_eq!(
            entry.availability,
            StorageAvailabilityDto::NotImplemented,
            "E{} 可选能力未装配必须 notImplemented（V13）",
            index + 1
        );
        assert_eq!(entry.bytes, None);
        assert_eq!(entry.count, None);
    }

    let expected = [
        StorageEligibilityDto::NotEligible,
        StorageEligibilityDto::NotImplemented,
        StorageEligibilityDto::NotEligible,
        StorageEligibilityDto::OwnerManaged,
        StorageEligibilityDto::OwnerManaged,
    ];
    for (offset, want) in expected.iter().enumerate() {
        assert_eq!(
            dto.entries[6 + offset].reclaim.eligibility,
            *want,
            "E{} 未装配时的资格必须按 §3.10 第 5 条（V13）",
            7 + offset
        );
    }

    assert_eq!(
        dto.entries[6].reclaim.reason,
        Some(StorageReclaimReasonDto::BusinessData)
    );
    assert_eq!(
        dto.entries[8].reclaim.reason,
        Some(StorageReclaimReasonDto::UserData)
    );
    assert_eq!(
        dto.entries[7].reclaim.reason,
        Some(StorageReclaimReasonDto::NoReclaimApi)
    );
}

#[test]
fn complete_ignores_only_logs_entry() {
    let dto = assemble_breakdown(base(), &available(), 100, StorageInventoryDto::default());
    assert!(dto.complete, "除 E12 外全部 available 时应为 true");

    let mut samples = available();
    samples[11] = EntrySample::degraded(StorageAvailabilityDto::NotImplemented, "插件自持");
    let dto = assemble_breakdown(base(), &samples, 100, StorageInventoryDto::default());
    assert!(dto.complete, "E12 不计入 complete 判定");

    let mut samples = available();
    samples[4] = EntrySample::degraded(StorageAvailabilityDto::Unavailable, "timeout");
    let dto = assemble_breakdown(base(), &samples, 100, StorageInventoryDto::default());
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
