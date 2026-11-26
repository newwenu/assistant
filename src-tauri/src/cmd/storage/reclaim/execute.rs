//! 执行（§5.3）：先整批校验（不过就零副作用），再逐条跑，单条失败不影响其余。

use crate::app_dirs::registry::CoreEntry;
use crate::cmd::storage::entries::{views, EntryView};
use crate::protocol::dto::{
    StorageReclaimActionDto, StorageReclaimRequestDto, StorageReclaimResultDto,
    StorageReclaimStatusDto,
};

use super::executors::{
    execute_derived, execute_index, execute_orphans, execute_scratch_clear, execute_section,
    execute_tool_results, execute_webview,
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
        // core 与分区两条路由互斥且穷尽：core 条目**不可能**落进分区分支，
        // 没有执行器的 core 条目也不会被误路由（§4.3 第 2 条）。
        let result = match view.core {
            Some(CoreEntry::Attachments) => {
                execute_orphans(
                    ports.attachments.clone(),
                    ports.db.clone(),
                    plan.sampled_at,
                    key.to_string(),
                    action,
                )
                .await
            }
            Some(CoreEntry::AttachmentsDerived) => {
                execute_derived(ports.attachments, plan.sampled_at, key.to_string(), action).await
            }
            Some(CoreEntry::ToolResults) => {
                let policy = plan.policy_of(key);
                execute_tool_results(ports.tool_results.clone(), policy, key.to_string(), action)
                    .await
            }
            Some(CoreEntry::ToolIndex) => execute_index(ports.dirs, key.to_string(), action).await,
            // `E14` 运行期只能登记待执行：WebView2 持有 profile 句柄，唯一安全执行点是
            // 下次启动、任何 webview 创建之前（§13.3.5）。
            Some(CoreEntry::WebViewProfile) => {
                execute_webview(ports.dirs, key.to_string(), action).await
            }
            // `E19` 临时区：用户显式清空整根，判据不来自磁盘差集（I77 §4.1）。
            Some(CoreEntry::ToolScratch) => {
                execute_scratch_clear(ports.scratch.clone(), key.to_string(), action).await
            }
            // 其余 core 条目没有执行器（`executor_action` 已在校验期拒绝），此处保持零副作用。
            Some(
                CoreEntry::MascotAssets
                | CoreEntry::Database
                | CoreEntry::Logs
                | CoreEntry::Crashes
                | CoreEntry::Config
                | CoreEntry::OrbConfig
                | CoreEntry::MascotConfig,
            ) => entry_result(
                key.to_string(),
                action,
                Totals::default(),
                Vec::new(),
                StorageReclaimStatusDto::NotEligible,
            ),
            None => match view.section.and_then(|index| ports.sections.get(index)) {
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

/// 白名单（§3.3）：core 只放行有执行器的 5 条（`E1 + A1` / `E2|E4|E14 + A5` / `E3 + A3`），
/// 可选条目只放行「分区自述可执行的动作」——分区没声明的动作一律拒绝。
///
/// 按 core 枚举判，不按下标、不按 `E*` 字符串：清单顺序调整不会悄悄改变可执行集合，
/// 新增 core 条目时编译器会点名这里（不会静默退化成「不可执行」）。
fn executor_action(view: &EntryView) -> Option<StorageReclaimActionDto> {
    match view.core? {
        CoreEntry::Attachments => Some(StorageReclaimActionDto::DeleteOrphans),
        CoreEntry::AttachmentsDerived | CoreEntry::ToolIndex | CoreEntry::WebViewProfile => {
            Some(StorageReclaimActionDto::RebuildCache)
        }
        CoreEntry::ToolResults => Some(StorageReclaimActionDto::SweepExpired),
        // `E19` 临时区：整根清空，与 `decl.rs` 的 `action` 必须同档（双重校验，漏一处即被整体拒绝）。
        CoreEntry::ToolScratch => Some(StorageReclaimActionDto::ClearAll),
        // `E15` / `E17` / `E18` 是用户业务数据（`Forbidden`），没有执行器：白名单直接拒绝。
        CoreEntry::MascotAssets
        | CoreEntry::Database
        | CoreEntry::Logs
        | CoreEntry::Crashes
        | CoreEntry::Config
        | CoreEntry::OrbConfig
        | CoreEntry::MascotConfig => None,
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
    match view.core {
        Some(_) => executor_action(view) == Some(action),
        None => section_action(view, ports) == Some(action),
    }
}
