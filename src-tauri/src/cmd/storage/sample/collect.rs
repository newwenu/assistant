//! 采样编排：`E1` 由调用方给，其余条目并行，全部条目共用一个 `sampledAt`（调用方取）。

use std::collections::HashSet;
use std::sync::Arc;

use futures::future::join_all;
use tauri::{AppHandle, Manager};

use assistant_storage::attachment_store::{AttachmentEntry, AttachmentStore};
use assistant_tools::ToolResultStorage;

use crate::adapters::PipelineToolExecution;
use crate::app_dirs::AppDirs;
use crate::cmd::storage::section::{SectionRegistry, StorageSectionHandle};
use crate::protocol::dto::StorageAvailabilityDto;
use crate::setup::DbState;

use super::super::entries::{set_sample, views, EntrySample};
use super::rows::{
    attachment_sample, derived_sample, sample_crashes, sample_database, sample_index,
    sample_mascot, sample_tool_results,
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

    // 清单先定：core 8 条在前，可选分区条目紧随其后；样本按同一清单对齐。
    let views = views(&sections);
    let mut samples: Vec<EntrySample> = views.iter().map(|_| EntrySample::missing()).collect();
    set_sample(
        &views,
        &mut samples,
        "E1",
        attachment_sample(entries, referenced),
    );

    let mut tasks: Vec<SampleFuture> = Vec::new();

    // E2：派生侧车目录（owner 自持 `derived/`，管理器不拼路径）。
    {
        let store = store.clone();
        tasks.push(Box::pin(timed("E2", async move {
            match store.list_derived().await {
                Ok(entries) => derived_sample(&entries),
                Err(error) => {
                    log::debug!("派生侧车统计失败: {error}");
                    EntrySample::degraded(StorageAvailabilityDto::Unreadable)
                }
            }
        })));
    }

    // E3：工具结果落盘（策略由 owner 自述，含注入时刻）。
    match storage {
        Some(storage) => tasks.push(Box::pin(timed_blocking("E3", move || {
            sample_tool_results(&storage)
        }))),
        None => set_sample(
            &views,
            &mut samples,
            "E3",
            EntrySample::degraded(StorageAvailabilityDto::NotImplemented),
        ),
    }

    // E4 / E5 / E13：应用自有目录，各走 owner 的公开入口。
    {
        let index_root = dirs.indexes();
        tasks.push(Box::pin(timed_blocking("E4", move || {
            sample_index(&index_root)
        })));
        let mascot_root = dirs.mascot_assets();
        tasks.push(Box::pin(timed_blocking("E5", move || {
            sample_mascot(&mascot_root)
        })));
        let crash_root = dirs.crashes();
        tasks.push(Box::pin(timed_blocking("E13", move || {
            sample_crashes(&crash_root)
        })));
    }

    // E6：主库文件，只 `metadata`、不开库。
    match db {
        Some(db) => tasks.push(Box::pin(timed_blocking("E6", move || sample_database(&db)))),
        None => set_sample(
            &views,
            &mut samples,
            "E6",
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

    // E12：日志组件不提供占用统计，恒为常量降级；缺口说明由静态 `note` 承载（R-16）。
    set_sample(
        &views,
        &mut samples,
        "E12",
        EntrySample::degraded(StorageAvailabilityDto::NotImplemented),
    );

    for (entry, sample) in join_all(tasks).await {
        set_sample(&views, &mut samples, entry, sample);
    }
    Sampled { views, samples }
}
