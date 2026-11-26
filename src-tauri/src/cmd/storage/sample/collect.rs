//! 采样编排（I61 §3.10 + I79 §5.1）：按采样档分流，全部条目共用一个 `sampledAt`（调用方取）。
//!
//! `Inline` 恒现场采样（预算 `SAMPLE_TIMEOUT`，超时即声明说谎探针）；`Deferred` 页面档
//! 只读缓存并把需要补算的 key 交给命令层触发，计划档现场采样（预算 `PLAN_SAMPLE_TIMEOUT`）
//! 且不写缓存，保住计划新鲜度（INV3）。

use std::collections::HashSet;
use std::sync::Arc;

use futures::future::join_all;
use tauri::{AppHandle, Manager};

use assistant_storage::attachment_store::{AttachmentEntry, AttachmentStore};
use assistant_tools::{ToolResultStorage, ToolScratchStore};

use crate::app_dirs::registry::{CoreEntry, StatsMode};
use crate::app_dirs::AppDirs;
use crate::cmd::storage::section::{SectionRegistry, StorageSectionHandle};
use crate::db_state::DbState;
use crate::protocol::dto::StorageAvailabilityDto;

use super::super::entries::{views, EntrySample};
use super::rows::{
    attachment_sample, derived_sample, refresh_tool_results_policy, sample_config_file,
    sample_crashes, sample_database, sample_index, sample_logs, sample_mascot, sample_scratch,
    sample_tool_results, sample_webview,
};
use super::sections::{refresh_judgement, section_sample};
use super::timeout::{timed, timed_blocking, Budget, SampleFuture};
use super::{EntryStatsCache, SampleInputs, SampleMode, Sampled};

/// 采样整页：`E1` 由调用方给，`Inline` 现场采样、`Deferred` 读缓存。
///
/// `dirs` 由调用方解析并传入：`AppDirs::resolve()` 失败属装配级错误，
/// 由命令整体返回 `Err`（与 `E1` 同级），不在采样层伪装成「条目不可用」。
pub async fn collect(
    app: &AppHandle,
    dirs: &AppDirs,
    store: &AttachmentStore,
    entries: &[AttachmentEntry],
    referenced: Option<&HashSet<String>>,
) -> Sampled {
    // 分区快照只取一次：采样期间新注册 / 卸载的分区不会让清单与样本错位。
    let sections: Vec<StorageSectionHandle> = app
        .try_state::<Arc<SectionRegistry>>()
        .map(|state| state.inner().snapshot())
        .unwrap_or_default();
    let tool_results: Option<Arc<dyn ToolResultStorage>> = crate::boot::tool_exec(app)
        .ok()
        .map(|exec| exec.storage().clone());
    let db: Option<DbState> = app
        .try_state::<DbState>()
        .map(|state| state.inner().clone());
    let scratch: Option<Arc<dyn ToolScratchStore>> = crate::boot::tool_scratch(app).ok();
    let stats: Option<EntryStatsCache> = app
        .try_state::<EntryStatsCache>()
        .map(|state| state.inner().clone());

    collect_with(
        SampleInputs {
            dirs,
            store,
            entries,
            referenced,
            sections,
            tool_results,
            scratch,
            db,
            stats,
            now: super::super::inventory::now_seconds(),
        },
        SampleMode::Page,
    )
    .await
}

/// 采样实现本体：只依赖 [`SampleInputs`] + [`SampleMode`]，计划命令与页面命令共用。
pub async fn collect_with(inputs: SampleInputs<'_>, mode: SampleMode<'_>) -> Sampled {
    let SampleInputs {
        dirs,
        store,
        entries,
        referenced,
        sections,
        tool_results: storage,
        scratch,
        db,
        stats,
        now,
    } = inputs;

    // 清单先定：core 条目在前，可选分区条目紧随其后；样本按同一清单对齐。
    let views = views(&sections);
    let mut samples: Vec<EntrySample> = views.iter().map(|_| EntrySample::missing()).collect();
    let mut triggers: Vec<&'static str> = Vec::new();

    // `Deferred` 只认缓存：页面档读缓存并收集补算键；计划档一律现场采样、不读也不写（INV3）。
    let cache = match mode {
        SampleMode::Page => stats.as_ref(),
        SampleMode::Plan { .. } => None,
    };
    if let Some(cache) = cache {
        let (deferred, pending) = cache.read(now, &views);
        samples = deferred;
        triggers = pending;
    }

    // 判据不进缓存（I79 §5.2 / INV9）：`Deferred` 条目的观测取自缓存，资格与策略每次读
    // 按**当前**状态现算（分区锁态 / 保留天数 / 上限即时生效）。因此 core 不需要任何
    // 「功能侧通知失效」的钩子，也不认识任何分区 `key`（G17(b) / X15）。
    if cache.is_some() {
        for (slot, view) in views.iter().enumerate() {
            if view.stats_mode != StatsMode::Deferred {
                continue;
            }
            if let Some(section) = view.section.and_then(|index| sections.get(index)) {
                refresh_judgement(section, view.key, &mut samples[slot]);
            } else if let (Some(CoreEntry::ToolResults), Some(storage)) = (view.core, &storage) {
                // core 侧唯一「判据随运行期状态变化」的 `Deferred` 条目（§3.1）：保留天数来自配置。
                refresh_tool_results_policy(storage, &mut samples[slot]);
            }
        }
    }

    // 句柄缺失的条目（`E3` / `E19` 在特性关闭构建下）不进缓存、也不触发补算（§5.1）：
    // 仍按现状返回 `notImplemented`，否则「无记录」会让每次读都白排一次后台任务。
    for (slot, view) in views.iter().enumerate() {
        if view.stats_mode != StatsMode::Deferred {
            continue;
        }
        let owner_missing = match view.core {
            Some(CoreEntry::ToolResults) => storage.is_none(),
            Some(CoreEntry::ToolScratch) => scratch.is_none(),
            _ => false,
        };
        if owner_missing {
            samples[slot] = EntrySample::degraded(StorageAvailabilityDto::NotImplemented);
            triggers.retain(|key| *key != view.key);
        }
    }

    // `E1` 恒 `Inline`（INV1）：账本与顶层字段同源，永远取调用方给的同一份清单。
    if let Some(slot) = views
        .iter()
        .position(|view| view.core == Some(CoreEntry::Attachments))
    {
        samples[slot] = attachment_sample(entries, referenced);
    }

    let mut tasks: Vec<SampleFuture> = Vec::new();

    for (slot, view) in views.iter().enumerate() {
        // 现场采样判定：`Inline` 恒是；`Deferred` 只在计划档按 `force` 强制（§8.2）。
        let live = match (view.stats_mode, mode) {
            (StatsMode::Inline, _) => true,
            (StatsMode::Deferred, SampleMode::Page) => cache.is_none(),
            (StatsMode::Deferred, SampleMode::Plan { .. }) => mode.forces(view.key),
        };
        if !live {
            continue;
        }
        let budget = match (view.stats_mode, mode) {
            (StatsMode::Inline, _) => Budget::inline(),
            (StatsMode::Deferred, SampleMode::Page) => Budget::deferred_page(),
            (StatsMode::Deferred, SampleMode::Plan { .. }) => Budget::plan(),
        };

        match view.core {
            // `E1` 已由上面直接给定（调用方那份 `list_all`），不进任务。
            Some(CoreEntry::Attachments) => {}
            // `E2`：派生侧车目录（owner 自持 `derived/`，管理器不拼路径）。
            Some(CoreEntry::AttachmentsDerived) => {
                let store = store.clone();
                tasks.push(Box::pin(timed(view.entry, budget, async move {
                    match store.list_derived().await {
                        Ok(entries) => derived_sample(&entries),
                        Err(error) => {
                            log::debug!("派生侧车统计失败: {error}");
                            EntrySample::degraded(StorageAvailabilityDto::Unreadable)
                        }
                    }
                })));
            }
            // `E3`：工具结果落盘（策略由 owner 自述，含注入时刻）。
            Some(CoreEntry::ToolResults) => match &storage {
                Some(storage) => {
                    let storage = storage.clone();
                    tasks.push(Box::pin(timed_blocking(view.entry, budget, move || {
                        sample_tool_results(&storage)
                    })));
                }
                None => {
                    samples[slot] = EntrySample::degraded(StorageAvailabilityDto::NotImplemented)
                }
            },
            // `E4` / `E5` / `E13`：应用自有目录，各走 owner 的公开入口。
            Some(CoreEntry::ToolIndex) => {
                let root = dirs.indexes();
                tasks.push(Box::pin(timed_blocking(view.entry, budget, move || {
                    sample_index(&root)
                })));
            }
            Some(CoreEntry::MascotAssets) => {
                let root = dirs.mascot_assets();
                tasks.push(Box::pin(timed_blocking(view.entry, budget, move || {
                    sample_mascot(&root)
                })));
            }
            Some(CoreEntry::Crashes) => {
                let root = dirs.crashes();
                tasks.push(Box::pin(timed_blocking(view.entry, budget, move || {
                    sample_crashes(&root)
                })));
            }
            // `E6`：主库文件，只 `metadata`、不开库。
            Some(CoreEntry::Database) => match &db {
                Some(db) => {
                    let db = db.clone();
                    tasks.push(Box::pin(timed_blocking(view.entry, budget, move || {
                        sample_database(&db)
                    })));
                }
                None => samples[slot] = EntrySample::degraded(StorageAvailabilityDto::Unavailable),
            },
            // `E12`：日志由插件自持轮转（插件无 stats API），路径是应用自有目录，
            // 按 `fs_stats` 口径（伴生文件同口径计入）只读遍历。
            Some(CoreEntry::Logs) => {
                let root = dirs.logs();
                tasks.push(Box::pin(timed_blocking(view.entry, budget, move || {
                    sample_logs(&root)
                })));
            }
            // `E14`：WebView2 profile，整棵树由应用接管观测（§13.3）。
            Some(CoreEntry::WebViewProfile) => {
                let profile = dirs.webview_profile();
                tasks.push(Box::pin(timed_blocking(view.entry, budget, move || {
                    sample_webview(&profile)
                })));
            }
            // `E15` / `E17` / `E18`：core 侧配置文件，路径由 `AppDirs` 按登记表拼出
            // （不复制段字面量，§13.4.3 第 5 条）。漏采样会让 `declared = 0`、
            // 整个文件被误报成余量（G11），因此必须与登记同批落地。
            Some(core @ (CoreEntry::Config | CoreEntry::OrbConfig | CoreEntry::MascotConfig)) => {
                let path = dirs.managed_path_of(core);
                tasks.push(Box::pin(timed_blocking(view.entry, budget, move || {
                    sample_config_file(&path)
                })));
            }
            // `E19`：shell 临时区（I77 §3.8）。句柄缺失 = 特性未装配，与 `E3` 同款降级。
            // 注意：本文件不是 `match`，编译器不点名，漏采样会让 `declared = 0`、
            // 整棵树被误报成余量（与 `E12` 的同款 G11 缺陷）。
            Some(CoreEntry::ToolScratch) => match &scratch {
                Some(scratch) => {
                    let scratch = scratch.clone();
                    tasks.push(Box::pin(timed_blocking(view.entry, budget, move || {
                        sample_scratch(&scratch)
                    })));
                }
                None => {
                    samples[slot] = EntrySample::degraded(StorageAvailabilityDto::NotImplemented)
                }
            },
            // 可选分区条目：按「视图自述所属分区 + 该条 key」分派，
            // core 不认识任何条目字面量（G17(b)）。
            None => match view.section.and_then(|index| sections.get(index)).cloned() {
                Some(section) => {
                    let key = view.key;
                    tasks.push(Box::pin(timed_blocking(view.entry, budget, move || {
                        section_sample(&section, key)
                    })));
                }
                None => samples[slot] = EntrySample::degraded(StorageAvailabilityDto::Unavailable),
            },
        }
    }

    for (entry, sample) in join_all(tasks).await {
        if let Some(slot) = views.iter().position(|view| view.entry == entry) {
            samples[slot] = sample;
        }
    }
    Sampled {
        views,
        samples,
        triggers,
    }
}
