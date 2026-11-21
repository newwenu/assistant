//! 执行（§5.3）：先整批校验（不过就零副作用），再逐条跑，单条失败不影响其余。

use crate::cmd::storage::entries::{views, EntryView};
use crate::protocol::dto::{
    StorageReclaimActionDto, StorageReclaimRequestDto, StorageReclaimResultDto,
    StorageReclaimStatusDto,
};

use super::executors::{
    execute_derived, execute_index, execute_orphans, execute_section, execute_tool_results,
};
use super::report::{entry_result, Totals};
use super::store::CachedPlan;
use super::ReclaimPorts;

/// 执行：先整批校验（不过就零副作用），再逐条跑，单条失败不影响其余（§5.3）。
pub async fn execute_storage_reclaim(
    ports: ReclaimPorts<'_>,
    args: StorageReclaimRequestDto,
) -> Result<StorageReclaimResultDto, String> {
    if !args.confirmed {
        return Err("confirmationRequired".to_string());
    }
    if args.entries.is_empty() {
        return Err("entriesRequired".to_string());
    }
    let plan = ports
        .plans
        .lookup(&args.plan_id, ports.now)
        .map_err(|error| error.as_str().to_string())?;
    // 清单只用于校验与路由：不重新采样，执行判据一律以 owner 重算为准（§5.2 第 4 条）。
    let views = views(&ports.sections);
    let targets = validated_targets(&ports, &args, &plan, &views)?;

    let _serial = ports
        .plans
        .begin_execution()
        .ok_or_else(|| "busy".to_string())?;

    let mut results = Vec::with_capacity(targets.len());
    for (view, key, action) in targets {
        let result = match view.entry {
            "E1" => {
                execute_orphans(
                    ports.attachments.clone(),
                    ports.db.clone(),
                    plan.sampled_at,
                    key.to_string(),
                    action,
                )
                .await
            }
            "E2" => {
                execute_derived(ports.attachments, plan.sampled_at, key.to_string(), action).await
            }
            "E3" => {
                let policy = plan.policy_of(key);
                execute_tool_results(ports.tool_results.clone(), policy, key.to_string(), action)
                    .await
            }
            "E4" => execute_index(ports.dirs, key.to_string(), action).await,
            _ => match view.section.and_then(|index| ports.sections.get(index)) {
                Some(section) => {
                    let policy = plan.policy_of(key);
                    execute_section(section.clone(), policy, key, action).await
                }
                None => entry_result(
                    key.to_string(),
                    action,
                    Totals::default(),
                    Vec::new(),
                    StorageReclaimStatusDto::NotEligible,
                ),
            },
        };
        log::info!(
            "storage reclaim: plan={} entry={} action={:?} removed={} bytes={} failed={} skipped={}",
            args.plan_id,
            result.key,
            result.action,
            result.removed_count,
            result.removed_bytes,
            result.failed_count,
            result.skipped_count
        );
        results.push(result);
    }

    Ok(StorageReclaimResultDto {
        plan_id: args.plan_id,
        executed_at: ports.now,
        entries: results,
    })
}

/// 白名单（§3.3）：core 只放行有执行器的 4 条（`E1 + A1` / `E2|E4 + A5` / `E3 + A3`），
/// 可选条目只放行「分区自述可执行的动作」——分区没声明的动作一律拒绝。
///
/// 按条目 id / 分区声明判，不按下标：清单顺序调整不会悄悄改变可执行集合。
fn executor_action(view: &EntryView) -> Option<StorageReclaimActionDto> {
    match view.entry {
        "E1" => Some(StorageReclaimActionDto::DeleteOrphans),
        "E2" | "E4" => Some(StorageReclaimActionDto::RebuildCache),
        "E3" => Some(StorageReclaimActionDto::SweepExpired),
        _ => None,
    }
}

fn section_action(view: &EntryView, ports: &ReclaimPorts<'_>) -> Option<StorageReclaimActionDto> {
    let section = ports.sections.get(view.section?)?;
    section
        .reclaim_actions()
        .iter()
        .copied()
        .find(|action| *action == view.action)
}

fn validated_targets<'v>(
    ports: &ReclaimPorts<'_>,
    args: &StorageReclaimRequestDto,
    plan: &CachedPlan,
    views: &'v [EntryView],
) -> Result<Vec<(&'v EntryView, &'static str, StorageReclaimActionDto)>, String> {
    let mut targets: Vec<(&'v EntryView, &'static str, StorageReclaimActionDto)> =
        Vec::with_capacity(args.entries.len());
    for item in &args.entries {
        let view = views
            .iter()
            .find(|view| view.key == item.key)
            .ok_or_else(|| "entryNotAllowed".to_string())?;
        if !action_allowed(view, ports, item.action) {
            return Err("entryNotAllowed".to_string());
        }
        if !plan.allows(&item.key, item.action) {
            return Err("entryNotAllowed".to_string());
        }
        if targets.iter().any(|(seen, _, _)| seen.key == view.key) {
            return Err("duplicateEntry".to_string());
        }
        targets.push((view, view.key, item.action));
    }
    Ok(targets)
}

/// 逐条执行前的动作校验：core 与可选分区共用同一入口，规则各自自述。
fn action_allowed(
    view: &EntryView,
    ports: &ReclaimPorts<'_>,
    action: StorageReclaimActionDto,
) -> bool {
    match view.section {
        Some(_) => section_action(view, ports) == Some(action),
        None => executor_action(view) == Some(action),
    }
}
