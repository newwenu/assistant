//! 计划 / 执行骨架（I61 §5.2、§6.1、§6.2）：纯函数，句柄一律由调用方给。
//!
//! 三条纪律：①计划**无副作用**，自己重新采样，不复用整页快照的数字（否则「预览」
//! 与「执行」会各说一套）；②执行必须校验 `planId` 未过期、条目在计划内、`action`
//! 与计划一致、且落在 §3.3 白名单，任一不符**整体拒绝**；③单条失败不中断其余条目。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use assistant_storage::attachment_store::{self, AttachmentStore};
use assistant_tools::{DeleteReport, ExpiredCandidate, ToolResultStorage};

use crate::app_dirs::registry::{self, ManagedRoot};
use crate::app_dirs::AppDirs;
use crate::protocol::dto::{
    ReclaimPolicyDto, StorageAvailabilityDto, StorageEligibilityDto, StorageReclaimActionDto,
    StorageReclaimEntryResultDto, StorageReclaimErrorCodeDto, StorageReclaimErrorDto,
    StorageReclaimPlanDto, StorageReclaimPlanEntryDto, StorageReclaimPlanRequestDto,
    StorageReclaimRequestDto, StorageReclaimResultDto, StorageReclaimStatusDto,
};
use crate::setup::DbState;

use super::entries::{self, EntrySample};
use super::probe::{ProbeError, ProbeRegistry};
use super::sample::{attachment_reason, collect_with, SampleInputs};

/// 计划有效期（§5.2.1）：`planId` 只在 10 分钟内可执行，过期零副作用。
pub const PLAN_TTL_SECONDS: i64 = 600;

/// 单个条目的 `errors` 上限（§5.3 语义硬约束 3）。
const MAX_ERRORS: usize = 5;

/// 执行侧依赖：全部由命令层从 `AppHandle` 取好后传入，本模块不碰 `AppHandle`。
///
/// `dirs` / `attachments` 恒装配（`E1` / `E2` 是 Always 挂载）；`probes` / `tool_results`
/// / `db` 属可选能力，缺装配时对应条目降级为「不可用」，不影响计划出数。
pub struct ReclaimPorts<'a> {
    pub plans: &'a ReclaimPlanStore,
    pub dirs: &'a AppDirs,
    pub attachments: &'a AttachmentStore,
    pub probes: Option<Arc<ProbeRegistry>>,
    pub tool_results: Option<Arc<dyn ToolResultStorage>>,
    pub db: Option<DbState>,
    /// 本次请求的统一时间戳：计划 `sampledAt` 与执行「判据排除」都按它比较。
    pub now: i64,
}

/// 已校验的条目快照：执行时只认「计划里出现过的 `key + action`」。
///
/// `policy` 是计划时刻的策略快照（§7.3 第 4 条）：配置热更新后执行侧要拿它比对，
/// 不一致就 `skipped`，不按旧策略删。`E2` / `E4` 的策略是静态档，比对恒等；
/// 带配置保留期的 `E3` 在 T3 接入时才真正用得上。
#[derive(Debug, Clone, PartialEq)]
pub struct CachedPlanEntry {
    pub key: String,
    pub action: StorageReclaimActionDto,
    pub policy: ReclaimPolicyDto,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CachedPlan {
    pub sampled_at: i64,
    pub created_at: i64,
    pub entries: Vec<CachedPlanEntry>,
}

impl CachedPlan {
    fn allows(&self, key: &str, action: StorageReclaimActionDto) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.key == key && entry.action == action)
    }

    /// 计划时刻的策略快照：执行侧拿它与 owner 当前策略比对（§7.3 第 4 条）。
    fn policy_of(&self, key: &str) -> Option<ReclaimPolicyDto> {
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.policy.clone())
    }
}

/// 计划查不到 / 已过期：两者都零副作用，错误码区分以便前端提示。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanLookupError {
    NotFound,
    Expired,
}

impl PlanLookupError {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "planNotFound",
            Self::Expired => "planExpired",
        }
    }
}

/// 计划缓存 + 单实例串行闸门：两者合一，保证「同一时刻只有一个执行在动文件」。
#[derive(Default)]
pub struct ReclaimPlanStore {
    inner: Mutex<HashMap<String, CachedPlan>>,
    executing: AtomicBool,
}

impl ReclaimPlanStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记计划并回收过期项（顺手裁剪，不做定时任务）。
    pub fn store(&self, plan: CachedPlan) -> String {
        let plan_id = uuid::Uuid::new_v4().to_string();
        let mut plans = self.lock();
        let now = plan.created_at;
        plans.retain(|_, cached| plan_age(now, cached.created_at) <= PLAN_TTL_SECONDS);
        plans.insert(plan_id.clone(), plan);
        plan_id
    }

    pub fn lookup(&self, plan_id: &str, now: i64) -> Result<CachedPlan, PlanLookupError> {
        let cached = self
            .lock()
            .get(plan_id)
            .cloned()
            .ok_or(PlanLookupError::NotFound)?;
        if plan_age(now, cached.created_at) > PLAN_TTL_SECONDS {
            self.lock().remove(plan_id);
            return Err(PlanLookupError::Expired);
        }
        Ok(cached)
    }

    /// 抢串行闸门；已有执行在跑时返回 `None`（命令层据此回 `busy`）。
    pub fn begin_execution(&self) -> Option<ExecutionGuard<'_>> {
        self.executing
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| ExecutionGuard {
                flag: &self.executing,
            })
    }

    /// 毒锁不该让整个存储页失效：只取回内部数据，继续用。
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, CachedPlan>> {
        self.inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }
}

fn plan_age(now: i64, created_at: i64) -> i64 {
    now.saturating_sub(created_at)
}

/// 执行期间持有；`Drop` 一定放闸（含提前 `?` 返回的路径）。
pub struct ExecutionGuard<'a> {
    flag: &'a AtomicBool,
}

impl Drop for ExecutionGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}

/// 计划：**只读**。重新采样一次，按 §3.1 矩阵装配条目，不触碰任何文件。
///
/// `entryKeys` 为空 = 全部 13 条；含未知 key 或重复 key 整体拒绝（不静默丢弃）。
pub async fn plan_storage_reclaim(
    ports: ReclaimPorts<'_>,
    args: StorageReclaimPlanRequestDto,
) -> Result<StorageReclaimPlanDto, String> {
    let roots = requested_roots(&args.entry_keys)?;

    let (all_entries, e1_reason) = match ports.attachments.list_all().await {
        Ok(entries) => (entries, None),
        Err(error) => (Vec::new(), Some(attachment_reason(&error))),
    };
    // `E1` 的孤儿判据要引用集合：取不到就留 `None`，资格按「判据不可算」降级（fail-closed）。
    let referenced = match ports.db.clone() {
        Some(db) => tauri::async_runtime::spawn_blocking(move || super::load_attachment_refs(&db))
            .await
            .ok()
            .and_then(Result::ok)
            .map(|refs| refs.referenced),
        None => None,
    };
    let mut samples = collect_with(SampleInputs {
        dirs: ports.dirs,
        store: ports.attachments,
        entries: &all_entries,
        referenced: referenced.as_ref(),
        probes: ports.probes,
        tool_results: ports.tool_results,
        db: ports.db,
    })
    .await;
    if let Some(reason) = e1_reason {
        entries::set_sample(
            &mut samples,
            "E1",
            EntrySample::degraded(StorageAvailabilityDto::Unreadable, reason),
        );
    }

    let plan_entries: Vec<StorageReclaimPlanEntryDto> = roots
        .iter()
        .map(|root| entries::build_plan_entry(root, entries::sample_for(&samples, root)))
        .collect();
    let complete = roots
        .iter()
        .all(|root| entries::judgeable(root, entries::sample_for(&samples, root)));
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
    let targets = validated_targets(&args, &plan)?;

    let _serial = ports
        .plans
        .begin_execution()
        .ok_or_else(|| "busy".to_string())?;

    let mut results = Vec::with_capacity(targets.len());
    for (root, key, action) in targets {
        let result = match root.entry {
            "E1" => {
                execute_orphans(
                    ports.attachments.clone(),
                    ports.db.clone(),
                    plan.sampled_at,
                    key,
                    action,
                )
                .await
            }
            "E2" => execute_derived(ports.attachments, plan.sampled_at, key, action).await,
            "E4" => execute_index(ports.dirs, key, action).await,
            "E3" => {
                let policy = plan.policy_of(&key);
                execute_tool_results(ports.tool_results.clone(), policy, key, action).await
            }
            "E8" => {
                let policy = plan.policy_of(&key);
                execute_backups(ports.probes.clone(), policy, key, action).await
            }
            _ => entry_result(
                key,
                action,
                Totals::default(),
                Vec::new(),
                StorageReclaimStatusDto::NotEligible,
            ),
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

fn requested_roots(keys: &[String]) -> Result<Vec<&'static ManagedRoot>, String> {
    if keys.is_empty() {
        return Ok(registry::MANAGED_ROOTS.iter().collect());
    }
    let mut roots = Vec::with_capacity(keys.len());
    for key in keys {
        let root = registry::find(key).ok_or_else(|| "entryNotAllowed".to_string())?;
        if roots.iter().any(|seen: &&ManagedRoot| seen.key == root.key) {
            return Err("duplicateEntry".to_string());
        }
        roots.push(root);
    }
    Ok(roots)
}

/// 白名单（§3.3）：T2 放行 `E2 + A5` / `E4 + A5`，T3 追加 `E1 + A1` / `E3 + A3` / `E8 + A4`，
/// 其余一律 `None`。
///
/// 按条目 id 判，不按下标：登记表顺序调整不会悄悄改变可执行集合。
fn executor_action(root: &ManagedRoot) -> Option<StorageReclaimActionDto> {
    match root.entry {
        "E1" => Some(StorageReclaimActionDto::DeleteOrphans),
        "E2" | "E4" => Some(StorageReclaimActionDto::RebuildCache),
        "E3" => Some(StorageReclaimActionDto::SweepExpired),
        "E8" => Some(StorageReclaimActionDto::PruneToLimit),
        _ => None,
    }
}

fn validated_targets(
    args: &StorageReclaimRequestDto,
    plan: &CachedPlan,
) -> Result<Vec<(&'static ManagedRoot, String, StorageReclaimActionDto)>, String> {
    let mut targets: Vec<(&'static ManagedRoot, String, StorageReclaimActionDto)> =
        Vec::with_capacity(args.entries.len());
    for item in &args.entries {
        let root = registry::find(&item.key).ok_or_else(|| "entryNotAllowed".to_string())?;
        if executor_action(root) != Some(item.action) {
            return Err("entryNotAllowed".to_string());
        }
        if !plan.allows(&item.key, item.action) {
            return Err("entryNotAllowed".to_string());
        }
        if targets.iter().any(|(seen, _, _)| seen.key == root.key) {
            return Err("duplicateEntry".to_string());
        }
        targets.push((root, item.key.clone(), item.action));
    }
    Ok(targets)
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

#[derive(Debug, Clone, Copy, Default)]
struct Totals {
    removed: u64,
    removed_bytes: u64,
    failed: u64,
    skipped: u64,
}

/// `E1`：执行时按 owner 重算孤儿集合（不认计划里的数量），逐个在 ID 锁内查引用再删。
///
/// 三条纪律：①引用集合在 `spawn_blocking` 内、持 DB 连接时重算（§5.2 第 4 条），
/// 计划里的孤儿数只用于预览；②`mtime` 不早于 `sampledAt` 的文件由 owner 跳过
/// （挡住「采样后新导入、引用尚未落库」的窗口，§5.2 第 5 条）；③锁顺序恒为
/// 「DB 连接锁 → 附件 ID 锁」，与 `delete_attachments` / 引用写入一致，不会 ABBA。
async fn execute_orphans(
    store: AttachmentStore,
    db: Option<DbState>,
    sampled_at: i64,
    key: String,
    action: StorageReclaimActionDto,
) -> StorageReclaimEntryResultDto {
    let Some(db) = db else {
        return entry_result(
            key,
            action,
            Totals::default(),
            vec![error_dto(
                StorageReclaimErrorCodeDto::OwnerUnavailable,
                "数据库未装配",
            )],
            StorageReclaimStatusDto::Failed,
        );
    };
    let entries = match store.list_all().await {
        Ok(entries) => entries,
        Err(error) => {
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::OwnerUnavailable,
                    attachment_reason(&error),
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
    };
    let work = move || -> Result<assistant_storage::attachment_store::OrphanPurgeReport, String> {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let referenced: std::collections::HashSet<String> =
            attachment_store::get_all_referenced_attachment_ids(&conn)?
                .into_iter()
                .collect();
        let orphans: Vec<String> = entries
            .into_iter()
            .filter(|entry| !referenced.contains(&entry.id))
            .map(|entry| entry.id)
            .collect();
        Ok(store.delete_orphans_by_ids(&conn, &orphans, sampled_at))
    };
    let report = match tauri::async_runtime::spawn_blocking(work).await {
        Ok(Ok(report)) => report,
        Ok(Err(_)) => {
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::Io,
                    "附件引用集合读取失败",
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
        Err(_) => {
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::Unknown,
                    "执行线程异常".to_string(),
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
    };
    let totals = Totals {
        removed: report.removed,
        removed_bytes: report.removed_bytes,
        failed: report.failed,
        skipped: report.skipped,
    };
    let errors = failure_errors(totals.failed, 0, "部分孤儿附件删除失败", "附件目录读取失败");
    entry_result(key, action, totals, errors, status_of(totals))
}

/// `E2`：执行时按 owner 重算孤儿（不认计划里的名字），只删「采样前就已存在」的。
async fn execute_derived(
    store: &AttachmentStore,
    sampled_at: i64,
    key: String,
    action: StorageReclaimActionDto,
) -> StorageReclaimEntryResultDto {
    let orphans: Vec<String> = match store.list_derived().await {
        Ok(entries) => entries
            .into_iter()
            .filter(|entry| entry.orphan)
            .map(|entry| entry.name)
            .collect(),
        Err(error) => {
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::OwnerUnavailable,
                    attachment_reason(&error),
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
    };
    let report = store.purge_derived_orphans(&orphans, sampled_at).await;
    let totals = Totals {
        removed: report.removed,
        removed_bytes: report.removed_bytes,
        failed: report.failed,
        skipped: report.skipped,
    };
    let errors = failure_errors(totals.failed, 0, "部分派生文件删除失败", "派生目录读取失败");
    entry_result(key, action, totals, errors, status_of(totals))
}

/// `E4`：全量重建缓存，删掉索引根下全部 `*.idx`（与 `index_stats` 同一遍历）。
async fn execute_index(
    dirs: &AppDirs,
    key: String,
    action: StorageReclaimActionDto,
) -> StorageReclaimEntryResultDto {
    let root = dirs.indexes();
    let report = match tauri::async_runtime::spawn_blocking(move || {
        assistant_tools::os::purge_index_files(&root)
    })
    .await
    {
        Ok(report) => report,
        Err(_) => {
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::Unknown,
                    "执行线程异常".to_string(),
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
    };
    let totals = Totals {
        removed: report.removed,
        removed_bytes: report.removed_bytes,
        failed: report.failed.saturating_add(report.read_failures),
        skipped: report.skipped,
    };
    let errors = failure_errors(
        totals.failed,
        report.read_failures,
        "部分索引文件删除失败",
        "索引目录读取失败",
    );
    entry_result(key, action, totals, errors, status_of(totals))
}

/// `E3`：按计划时刻的保留天数收集过期候选（带字节）并逐个删除。
///
/// 三条纪律：①计划生成后策略被改（§7.3 第 4 条）→ `skipped`，不按旧策略删；
/// ②候选的字节在收集时取，删除时不再重算（删完取不到，§3.10 接口形状 1）；
/// ③「文件已不存在」算 `skipped` 不算失败，单条失败不影响其余（§5.1 第 2、3 条）。
async fn execute_tool_results(
    storage: Option<Arc<dyn ToolResultStorage>>,
    planned_policy: Option<ReclaimPolicyDto>,
    key: String,
    action: StorageReclaimActionDto,
) -> StorageReclaimEntryResultDto {
    let Some(storage) = storage else {
        return entry_result(
            key,
            action,
            Totals::default(),
            vec![error_dto(
                StorageReclaimErrorCodeDto::OwnerUnavailable,
                "工具执行管线未装配",
            )],
            StorageReclaimStatusDto::Failed,
        );
    };
    let planned_days = planned_policy
        .and_then(|policy| policy.retention_days)
        .unwrap_or(0);
    let current_days = storage
        .result_retention()
        .map(|(days, _)| days)
        .unwrap_or(0);
    // 策略禁用（`0`）或与计划快照不一致：零副作用跳过，绝不按旧天数删。
    if planned_days == 0 || planned_days != current_days {
        return entry_result(
            key,
            action,
            Totals::default(),
            Vec::new(),
            StorageReclaimStatusDto::Skipped,
        );
    }

    let work = move || -> Option<(Vec<ExpiredCandidate>, DeleteReport)> {
        let candidates = storage.collect_expired()?;
        let report = storage.delete_candidates(&candidates)?;
        Some((candidates, report))
    };
    let report = match tauri::async_runtime::spawn_blocking(work).await {
        Ok(Some((_, report))) => report,
        Ok(None) => {
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::OwnerUnavailable,
                    "存储未提供回收入口",
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
        Err(_) => {
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::Unknown,
                    "执行线程异常".to_string(),
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
    };
    let totals = Totals {
        removed: report.removed,
        removed_bytes: report.removed_bytes,
        failed: report.failed,
        skipped: report.skipped,
    };
    let errors = failure_errors(
        totals.failed,
        0,
        "部分工具结果删除失败",
        "工具结果目录读取失败",
    );
    entry_result(key, action, totals, errors, status_of(totals))
}

/// `E8`：按计划时刻的 `keep` 让 owner 重新计数并裁剪。
///
/// 三条纪律：①份数只从策略快照取，管理器不硬编码 `5`（§3.1 `E8` 行）；
/// ②重新计数与「保留最新一份」都在 owner 的 `prune_backups_to` 内完成（§5.2 第 4 条），
/// 计划后被新建的备份只会把更旧的挤出去，不会误删新备份；
/// ③`keep` 缺失（`0`）视为策略不可用 → `skipped`，不猜份数。
async fn execute_backups(
    probes: Option<Arc<ProbeRegistry>>,
    planned_policy: Option<ReclaimPolicyDto>,
    key: String,
    action: StorageReclaimActionDto,
) -> StorageReclaimEntryResultDto {
    let keep = planned_policy
        .and_then(|policy| policy.keep_count)
        .unwrap_or(0);
    if keep == 0 {
        return entry_result(
            key,
            action,
            Totals::default(),
            Vec::new(),
            StorageReclaimStatusDto::Skipped,
        );
    }
    let Some(capability) = registry::find_by_entry("E8").and_then(|root| match root.mount {
        registry::RootMount::Feature(capability) => Some(capability),
        registry::RootMount::Always => None,
    }) else {
        return entry_result(
            key,
            action,
            Totals::default(),
            vec![error_dto(
                StorageReclaimErrorCodeDto::OwnerUnavailable,
                "备份目录未登记为可选能力",
            )],
            StorageReclaimStatusDto::Failed,
        );
    };
    let Some(probe) = probes.as_ref().and_then(|probes| probes.find(capability)) else {
        return entry_result(
            key,
            action,
            Totals::default(),
            vec![error_dto(
                StorageReclaimErrorCodeDto::OwnerUnavailable,
                "备份目录所属能力未装配",
            )],
            StorageReclaimStatusDto::Failed,
        );
    };
    let work = move || probe.prune_backups(keep);
    let report = match tauri::async_runtime::spawn_blocking(work).await {
        Ok(Some(Ok(report))) => report,
        Ok(Some(Err(error))) => {
            let code = match error {
                ProbeError::Locked | ProbeError::Unavailable => {
                    StorageReclaimErrorCodeDto::OwnerUnavailable
                }
                ProbeError::Unreadable => StorageReclaimErrorCodeDto::Io,
            };
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(code, "备份目录读取失败")],
                StorageReclaimStatusDto::Failed,
            );
        }
        Ok(None) => {
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::OwnerUnavailable,
                    "存储未提供回收入口",
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
        Err(_) => {
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::Unknown,
                    "执行线程异常".to_string(),
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
    };
    let totals = Totals {
        removed: report.removed,
        removed_bytes: report.removed_bytes,
        failed: report.failed,
        skipped: 0,
    };
    let errors = failure_errors(totals.failed, 0, "部分备份删除失败", "备份目录读取失败");
    entry_result(key, action, totals, errors, status_of(totals))
}

fn status_of(totals: Totals) -> StorageReclaimStatusDto {
    if totals.failed > 0 {
        if totals.removed > 0 {
            StorageReclaimStatusDto::Partial
        } else {
            StorageReclaimStatusDto::Failed
        }
    } else if totals.removed > 0 {
        StorageReclaimStatusDto::Done
    } else {
        StorageReclaimStatusDto::Skipped
    }
}

/// 失败原因只报类别 + 通用文案，不带路径与文件名（§5.3 硬约束 3）。
fn failure_errors(
    failed: u64,
    read_failures: u64,
    delete_message: &str,
    read_message: &str,
) -> Vec<StorageReclaimErrorDto> {
    let mut errors = Vec::new();
    if failed.saturating_sub(read_failures) > 0 {
        errors.push(error_dto(StorageReclaimErrorCodeDto::Io, delete_message));
    }
    if read_failures > 0 {
        errors.push(error_dto(StorageReclaimErrorCodeDto::Io, read_message));
    }
    errors
}

fn error_dto(
    code: StorageReclaimErrorCodeDto,
    message: impl Into<String>,
) -> StorageReclaimErrorDto {
    StorageReclaimErrorDto {
        code,
        message: message.into(),
    }
}

fn entry_result(
    key: String,
    action: StorageReclaimActionDto,
    totals: Totals,
    errors: Vec<StorageReclaimErrorDto>,
    status: StorageReclaimStatusDto,
) -> StorageReclaimEntryResultDto {
    StorageReclaimEntryResultDto {
        key,
        action,
        status,
        removed_count: totals.removed,
        removed_bytes: totals.removed_bytes,
        failed_count: totals.failed,
        skipped_count: totals.skipped,
        errors: errors.into_iter().take(MAX_ERRORS).collect(),
    }
}
