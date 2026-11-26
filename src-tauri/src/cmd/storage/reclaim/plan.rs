//! 计划（只读预览，§5.2）：重新采样一次，按条目清单装配，不触碰任何文件。

use crate::app_dirs::registry::CoreEntry;
use crate::protocol::dto::{
    StorageAvailabilityDto, StorageEligibilityDto, StorageReclaimPlanDto,
    StorageReclaimPlanEntryDto, StorageReclaimPlanRequestDto,
};

use super::super::entries::{self, EntrySample, EntryView};
use super::super::ledger;
use super::super::sample::{collect_with, SampleInputs, SampleMode, Sampled};
use super::store::{CachedPlan, CachedPlanEntry};
use super::ReclaimPorts;

/// 计划：**只读**。重新采样一次，按条目清单装配，不触碰任何文件。
///
/// `entryKeys` 为空 = 清单里的全部条目；含未知 key 或重复 key 整体拒绝（不静默丢弃）。
pub async fn plan_storage_reclaim(
    ports: ReclaimPorts<'_>,
    args: StorageReclaimPlanRequestDto,
) -> Result<StorageReclaimPlanDto, String> {
    let (all_entries, e1_failed) = match ports.attachments.list_all().await {
        Ok(entries) => (entries, false),
        Err(error) => {
            log::error!("附件清单读取失败: {error}");
            (Vec::new(), true)
        }
    };
    // `E1` 的孤儿判据要引用集合：取不到就留 `None`，资格按「判据不可算」降级（fail-closed）。
    let referenced = match ports.db.clone() {
        Some(db) => tauri::async_runtime::spawn_blocking(move || ledger::load_attachment_refs(&db))
            .await
            .ok()
            .and_then(Result::ok)
            .map(|refs| refs.referenced),
        None => None,
    };
    // 计划模式只强制采样请求里的 key（`None` = 清单全部）：`force` 用登记表里的静态 key，
    // 未知 key 留给后面的 `requested_views` 统一拒绝（不在这里静默丢弃）。
    let force_keys: Option<Vec<&'static str>> = if args.entry_keys.is_empty() {
        None
    } else {
        let views = entries::views(&ports.sections);
        Some(
            args.entry_keys
                .iter()
                .filter_map(|key| entries::find_by_key(&views, key).map(|view| view.key))
                .collect(),
        )
    };
    let mut sampled: Sampled = collect_with(
        SampleInputs {
            dirs: ports.dirs,
            store: ports.attachments,
            entries: &all_entries,
            referenced: referenced.as_ref(),
            sections: ports.sections.clone(),
            tool_results: ports.tool_results.clone(),
            scratch: ports.scratch.clone(),
            db: ports.db.clone(),
            // 计划不读也不写统计缓存：`Deferred` 全部现场采样，保住计划新鲜度（INV3）。
            stats: None,
            now: ports.now,
        },
        // `force = None`（请求为空 = 清单全部）或请求的 key 集合（§8.2）。
        SampleMode::Plan {
            force: force_keys.as_deref(),
        },
    )
    .await;
    if e1_failed {
        if let Some(slot) = entries::index_of_core(&sampled.views, CoreEntry::Attachments) {
            sampled.samples[slot] = EntrySample::degraded(StorageAvailabilityDto::Unreadable);
        }
    }
    finish(ports, args, &sampled.views, &sampled.samples)
}

/// 装配计划：按请求的 `entryKeys` 从清单里取条目，清单里没有的 key 整体拒绝。
fn finish(
    ports: ReclaimPorts<'_>,
    args: StorageReclaimPlanRequestDto,
    views: &[EntryView],
    samples: &[EntrySample],
) -> Result<StorageReclaimPlanDto, String> {
    let selected = requested_views(&args.entry_keys, views)?;

    let plan_entries: Vec<StorageReclaimPlanEntryDto> = selected
        .iter()
        .map(|view| {
            let sample = entries::sample_for(views, samples, view.key);
            entries::build_plan_entry(view, sample)
        })
        .collect();
    let complete = selected.iter().all(|view| {
        let sample = entries::sample_for(views, samples, view.key);
        entries::judgeable(view, sample)
    });
    let total_reclaimable_bytes = total_expected_bytes(&plan_entries);

    let cached = CachedPlan {
        sampled_at: ports.now,
        created_at: ports.now,
        entries: plan_entries
            .iter()
            .map(|entry| CachedPlanEntry {
                key: entry.key.clone(),
                action: entry.action,
                policy: entry.policy.clone(),
            })
            .collect(),
    };
    let plan_id = ports.plans.store(cached);

    Ok(StorageReclaimPlanDto {
        plan_id,
        sampled_at: ports.now,
        entries: plan_entries,
        total_reclaimable_bytes,
        complete,
    })
}

/// 请求的 key → 视图：空 = 清单全部；未知 key / 重复 key 整体拒绝。
fn requested_views<'v>(
    keys: &[String],
    views: &'v [EntryView],
) -> Result<Vec<&'v EntryView>, String> {
    if keys.is_empty() {
        return Ok(views.iter().collect());
    }
    let mut selected: Vec<&'v EntryView> = Vec::with_capacity(keys.len());
    for key in keys {
        let view = views
            .iter()
            .find(|view| view.key == key)
            .ok_or_else(|| "entryNotAllowed".to_string())?;
        if selected.iter().any(|seen| seen.key == view.key) {
            return Err("duplicateEntry".to_string());
        }
        selected.push(view);
    }
    Ok(selected)
}

/// 任一 `eligible` 条目算不出可回收量则 `null`，不拿部分和冒充总量。
fn total_expected_bytes(plan_entries: &[StorageReclaimPlanEntryDto]) -> Option<u64> {
    let mut total = 0u64;
    for entry in plan_entries
        .iter()
        .filter(|entry| entry.eligibility == StorageEligibilityDto::Eligible)
    {
        total = total.saturating_add(entry.expected_bytes?);
    }
    Some(total)
}
