//! 采样编排：`E1` 由调用方给，其余条目并行，全部条目共用一个 `sampledAt`（调用方取）。

use std::collections::HashSet;
use std::sync::Arc;

use futures::future::join_all;
use tauri::{AppHandle, Manager};

use assistant_storage::attachment_store::{AttachmentEntry, AttachmentStore};
use assistant_tools::ToolResultStorage;

use crate::adapters::PipelineToolExecution;
use crate::app_dirs::registry::CoreEntry;
use crate::app_dirs::AppDirs;
use crate::cmd::storage::section::{SectionRegistry, StorageSectionHandle};
use crate::protocol::dto::StorageAvailabilityDto;
use crate::setup::DbState;

use super::super::entries::{set_sample, views, EntrySample};
use super::rows::{
    attachment_sample, derived_sample, sample_config_file, sample_crashes, sample_database,
    sample_index, sample_logs, sample_mascot, sample_tool_results, sample_webview,
};
use super::sections::section_sample;
use super::timeout::{timed, timed_blocking, SampleFuture};
use super::{SampleInputs, Sampled};

/// 采样整页：`E1` 由调用方给，其余条目并行；全部条目共用一个 `sampledAt`（调用方取）。
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
    let tool_results: Option<Arc<dyn ToolResultStorage>> = app
        .try_state::<Arc<PipelineToolExecution>>()
        .map(|state| state.storage().clone());
    let db: Option<DbState> = app
        .try_state::<DbState>()
        .map(|state| state.inner().clone());

    collect_with(SampleInputs {
        dirs,
        store,
        entries,
        referenced,
        sections,
        tool_results,
        db,
    })
    .await
}

/// 采样实现本体：只依赖 [`SampleInputs`]，计划命令与 `get_storage_breakdown` 共用。
pub async fn collect_with(inputs: SampleInputs<'_>) -> Sampled {
    let SampleInputs {
        dirs,
        store,
        entries,
        referenced,
        sections,
        tool_results: storage,
        db,
    } = inputs;

    // 清单先定：core 条目在前，可选分区条目紧随其后；样本按同一清单对齐。
    let views = views(&sections);
    let mut samples: Vec<EntrySample> = views.iter().map(|_| EntrySample::missing()).collect();
    set_sample(
        &views,
        &mut samples,
        CoreEntry::Attachments.id(),
        attachment_sample(entries, referenced),
    );

    let mut tasks: Vec<SampleFuture> = Vec::new();

    // E2：派生侧车目录（owner 自持 `derived/`，管理器不拼路径）。
    {
        let store = store.clone();
        tasks.push(Box::pin(timed(
            CoreEntry::AttachmentsDerived.id(),
            async move {
                match store.list_derived().await {
                    Ok(entries) => derived_sample(&entries),
                    Err(error) => {
                        log::debug!("派生侧车统计失败: {error}");
                        EntrySample::degraded(StorageAvailabilityDto::Unreadable)
                    }
                }
            },
        )));
    }

    // E3：工具结果落盘（策略由 owner 自述，含注入时刻）。
    match storage {
        Some(storage) => tasks.push(Box::pin(timed_blocking(
            CoreEntry::ToolResults.id(),
            move || sample_tool_results(&storage),
        ))),
        None => set_sample(
            &views,
            &mut samples,
            CoreEntry::ToolResults.id(),
            EntrySample::degraded(StorageAvailabilityDto::NotImplemented),
        ),
    }

    // E4 / E5 / E13：应用自有目录，各走 owner 的公开入口。
    {
        let index_root = dirs.indexes();
        tasks.push(Box::pin(timed_blocking(
            CoreEntry::ToolIndex.id(),
            move || sample_index(&index_root),
        )));
        let mascot_root = dirs.mascot_assets();
        tasks.push(Box::pin(timed_blocking(
            CoreEntry::MascotAssets.id(),
            move || sample_mascot(&mascot_root),
        )));
        let crash_root = dirs.crashes();
        tasks.push(Box::pin(timed_blocking(
            CoreEntry::Crashes.id(),
            move || sample_crashes(&crash_root),
        )));
    }

    // E6：主库文件，只 `metadata`、不开库。
    match db {
        Some(db) => tasks.push(Box::pin(timed_blocking(
            CoreEntry::Database.id(),
            move || sample_database(&db),
        ))),
        None => set_sample(
            &views,
            &mut samples,
            CoreEntry::Database.id(),
            EntrySample::degraded(StorageAvailabilityDto::Unavailable),
        ),
    }

    // 可选分区条目：按「视图自述所属分区 + 该条 key」分派，core 不认识任何条目字面量（G17(b)）。
    for view in &views {
        let Some(index) = view.section else {
            continue;
        };
        let Some(section) = sections.get(index).cloned() else {
            continue;
        };
        let key = view.key;
        tasks.push(Box::pin(timed_blocking(view.entry, move || {
            section_sample(&section, key)
        })));
    }

    // E12：日志由插件自持轮转（插件无 stats API），路径是应用自有目录，按 `fs_stats` 口径
    // （伴生文件同口径计入）只读遍历。
    {
        let logs_root = dirs.logs();
        tasks.push(Box::pin(timed_blocking(CoreEntry::Logs.id(), move || {
            sample_logs(&logs_root)
        })));
    }

    // E14：WebView2 profile，整棵树由应用接管观测（§13.3）。
    {
        let profile = dirs.webview_profile();
        tasks.push(Box::pin(timed_blocking(
            CoreEntry::WebViewProfile.id(),
            move || sample_webview(&profile),
        )));
    }

    // E15 / E17 / E18：core 侧配置文件，路径由 `AppDirs` 按登记表拼出（不复制段字面量，§13.4.3 第 5 条）。
    // 漏采样会让 `declared = 0`、整个文件被误报成余量（G11），因此必须与登记同批落地。
    for entry in [
        CoreEntry::Config,
        CoreEntry::OrbConfig,
        CoreEntry::MascotConfig,
    ] {
        let path = dirs.managed_path_of(entry);
        tasks.push(Box::pin(timed_blocking(entry.id(), move || {
            sample_config_file(&path)
        })));
    }

    for (entry, sample) in join_all(tasks).await {
        set_sample(&views, &mut samples, entry, sample);
    }
    Sampled { views, samples }
}
