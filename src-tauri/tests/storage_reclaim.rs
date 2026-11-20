// I61 T2：回收计划 / 执行的骨架契约（V6 / V7 / V8 / V9 / 白名单负例）。
//
// 纯函数 `plan_storage_reclaim` / `execute_storage_reclaim` 不碰 `AppHandle`，
// 因此集成测试可以用临时基根 + 真实 owner 方法（`E2` / `E4`）直接驱动。

use std::path::Path;
use std::sync::Arc;

use assistant_app::app_dirs::AppDirs;
use assistant_app::cmd::storage::reclaim::{
    execute_storage_reclaim, plan_storage_reclaim, ReclaimPlanStore, ReclaimPorts, PLAN_TTL_SECONDS,
};
use assistant_app::protocol::dto::{
    StorageConfirmLevelDto, StorageEligibilityDto, StorageReclaimActionDto,
    StorageReclaimEntryRequestDto, StorageReclaimPlanRequestDto, StorageReclaimReasonDto,
    StorageReclaimRequestDto, StorageReclaimStatusDto,
};
use assistant_storage::attachment_store::AttachmentStore;
use assistant_tools::{FileToolResultStorage, ToolResultStorage};

const DERIVED_KEY: &str = "attachments/derived";
const INDEX_KEY: &str = "tool-index";
const TOOL_RESULTS_KEY: &str = "tool-results";

struct Fixture {
    _data: tempfile::TempDir,
    _local: tempfile::TempDir,
    dirs: AppDirs,
    store: AttachmentStore,
    plans: ReclaimPlanStore,
}

impl Fixture {
    fn new() -> Self {
        let data = tempfile::tempdir().unwrap();
        let local = tempfile::tempdir().unwrap();
        let dirs = AppDirs::with_bases(data.path().to_path_buf(), local.path().to_path_buf());
        let store = AttachmentStore::new(dirs.attachments());
        Self {
            _data: data,
            _local: local,
            dirs,
            store,
            plans: ReclaimPlanStore::new(),
        }
    }

    fn ports(&self, now: i64) -> ReclaimPorts<'_> {
        self.ports_with(now, None)
    }

    /// 带 `E3` owner 句柄的装配；`probes` / `db` 由调用方按需给（集成层拿不到这两者的类型）。
    fn ports_with(
        &self,
        now: i64,
        tool_results: Option<Arc<dyn ToolResultStorage>>,
    ) -> ReclaimPorts<'_> {
        ReclaimPorts {
            plans: &self.plans,
            dirs: &self.dirs,
            attachments: &self.store,
            probes: None,
            tool_results,
            db: None,
            now,
        }
    }

    /// 一条孤儿侧车（无对应基附件）+ 两个可重建索引文件。
    async fn seed(&self) -> (u64, u64) {
        self.store
            .write_derived_text("orphan.png", "hello")
            .await
            .unwrap();
        let index_root = self.dirs.indexes();
        std::fs::create_dir_all(&index_root).unwrap();
        std::fs::write(index_root.join("a.idx"), b"0123456789").unwrap();
        std::fs::write(index_root.join("b.idx"), b"0123456789").unwrap();
        (5, 20)
    }
}

fn request(key: &str, action: StorageReclaimActionDto) -> StorageReclaimEntryRequestDto {
    StorageReclaimEntryRequestDto {
        key: key.to_string(),
        action,
    }
}

fn plan_request(keys: &[&str]) -> StorageReclaimPlanRequestDto {
    StorageReclaimPlanRequestDto {
        entry_keys: keys.iter().map(|key| key.to_string()).collect(),
    }
}

fn execute_request(
    plan_id: &str,
    entries: Vec<StorageReclaimEntryRequestDto>,
    confirmed: bool,
) -> StorageReclaimRequestDto {
    StorageReclaimRequestDto {
        plan_id: plan_id.to_string(),
        entries,
        confirmed,
    }
}

/// 目录签名：文件名 + 字节 + mtime 秒，用于断言「计划无副作用」（V6）。
fn signature(root: &Path) -> Vec<(String, u64, i64)> {
    let mut items = Vec::new();
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return items,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if meta.is_dir() {
            items.extend(signature(&path));
            continue;
        }
        let mtime = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX))
            .unwrap_or(-1);
        items.push((path.to_string_lossy().into_owned(), meta.len(), mtime));
    }
    items
}

fn future_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
        .saturating_add(10)
}

/// 写一个「`days_ago` 天前修改过」的工具结果文件：判据只看 `mtime`。
fn write_expired(path: &Path, bytes: &[u8], days_ago: u64) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
    let mtime = std::time::SystemTime::now() - std::time::Duration::from_secs(days_ago * 86_400);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
}

#[tokio::test]
async fn plan_is_read_only_and_reports_reclaimable_numbers() {
    let fixture = Fixture::new();
    let (derived_bytes, index_bytes) = fixture.seed().await;
    let before_derived = signature(&fixture.dirs.attachments());
    let before_index = signature(&fixture.dirs.indexes());

    let plan = plan_storage_reclaim(
        fixture.ports(future_now()),
        plan_request(&[DERIVED_KEY, INDEX_KEY]),
    )
    .await
    .unwrap();

    assert_eq!(plan.entries.len(), 2, "只回请求的两条");
    assert_eq!(plan.entries[0].key, DERIVED_KEY);
    assert_eq!(plan.entries[0].eligibility, StorageEligibilityDto::Eligible);
    assert_eq!(plan.entries[0].expected_bytes, Some(derived_bytes));
    assert_eq!(plan.entries[0].expected_count, Some(1));
    assert_eq!(plan.entries[1].key, INDEX_KEY);
    assert_eq!(plan.entries[1].eligibility, StorageEligibilityDto::Eligible);
    assert_eq!(plan.entries[1].expected_bytes, Some(index_bytes));
    assert_eq!(plan.entries[1].expected_count, Some(2));
    assert_eq!(
        plan.total_reclaimable_bytes,
        Some(derived_bytes + index_bytes)
    );
    assert!(plan.complete, "两条都算得出判据");
    assert!(!plan.plan_id.is_empty(), "planId 不透明但必须非空");

    assert_eq!(
        before_derived,
        signature(&fixture.dirs.attachments()),
        "计划不得改动任何文件（V6）"
    );
    assert_eq!(
        before_index,
        signature(&fixture.dirs.indexes()),
        "计划不得改动任何文件（V6）"
    );
}

#[tokio::test]
async fn plan_rejects_unknown_or_duplicate_keys() {
    let fixture = Fixture::new();
    let error = plan_storage_reclaim(
        fixture.ports(future_now()),
        plan_request(&["unregistered-name"]),
    )
    .await
    .unwrap_err();
    assert_eq!(error, "entryNotAllowed", "未知 key 整体拒绝（V20 负例）");

    let error = plan_storage_reclaim(
        fixture.ports(future_now()),
        plan_request(&[DERIVED_KEY, DERIVED_KEY]),
    )
    .await
    .unwrap_err();
    assert_eq!(error, "duplicateEntry", "重复 key 不静默去重");
}

#[tokio::test]
async fn execute_rejects_unconfirmed_empty_and_mismatched_requests() {
    let fixture = Fixture::new();
    fixture.seed().await;
    let now = future_now();
    let plan = plan_storage_reclaim(fixture.ports(now), plan_request(&[DERIVED_KEY]))
        .await
        .unwrap();
    let action = StorageReclaimActionDto::RebuildCache;

    let error = execute_storage_reclaim(
        fixture.ports(now),
        execute_request(&plan.plan_id, vec![request(DERIVED_KEY, action)], false),
    )
    .await
    .unwrap_err();
    assert_eq!(error, "confirmationRequired", "未确认必须拒绝（V8）");

    let error = execute_storage_reclaim(
        fixture.ports(now),
        execute_request(&plan.plan_id, Vec::new(), true),
    )
    .await
    .unwrap_err();
    assert_eq!(error, "entriesRequired", "空条目必须拒绝（V8）");

    let error = execute_storage_reclaim(
        fixture.ports(now),
        execute_request(
            &plan.plan_id,
            vec![request(DERIVED_KEY, StorageReclaimActionDto::DeleteOrphans)],
            true,
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(error, "entryNotAllowed", "action 与计划不符必须拒绝（V8）");

    let error = execute_storage_reclaim(
        fixture.ports(now),
        execute_request(&plan.plan_id, vec![request(INDEX_KEY, action)], true),
    )
    .await
    .unwrap_err();
    assert_eq!(error, "entryNotAllowed", "不在计划内的条目必须拒绝（V8）");
}

#[tokio::test]
async fn execute_rejects_unknown_and_expired_plans() {
    let fixture = Fixture::new();
    fixture.seed().await;
    let now = future_now();
    let plan = plan_storage_reclaim(fixture.ports(now), plan_request(&[DERIVED_KEY]))
        .await
        .unwrap();

    let error = execute_storage_reclaim(
        fixture.ports(now),
        execute_request(
            "no-such-plan",
            vec![request(DERIVED_KEY, StorageReclaimActionDto::RebuildCache)],
            true,
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(error, "planNotFound", "计划不存在零副作用");

    let expired = now.saturating_add(PLAN_TTL_SECONDS + 1);
    let error = execute_storage_reclaim(
        fixture.ports(expired),
        execute_request(
            &plan.plan_id,
            vec![request(DERIVED_KEY, StorageReclaimActionDto::RebuildCache)],
            true,
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(error, "planExpired", "过期计划零副作用（V8）");
    assert_eq!(
        signature(&fixture.dirs.attachments()),
        signature(&fixture.dirs.attachments()),
        "拒绝路径不得动文件"
    );
}

#[tokio::test]
async fn execute_rejects_keys_outside_whitelist() {
    let fixture = Fixture::new();
    fixture.seed().await;
    let now = future_now();
    let plan = plan_storage_reclaim(fixture.ports(now), plan_request(&[]))
        .await
        .unwrap();

    // `E12`（logs）在 T2 仍不可回收：`action = none`，白名单外整体拒绝。
    let error = execute_storage_reclaim(
        fixture.ports(now),
        execute_request(
            "logs",
            vec![request("logs", StorageReclaimActionDto::None)],
            true,
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(error, "planNotFound", "计划里没有该条目");

    let error = execute_storage_reclaim(
        fixture.ports(now),
        execute_request(
            &plan.plan_id,
            vec![request("logs", StorageReclaimActionDto::None)],
            true,
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(error, "entryNotAllowed", "白名单外请求整体拒绝（V17 负例）");
}

/// 白名单只放行 `E1 + A1` / `E2 + A5` / `E3 + A3` / `E4 + A5` / `E8 + A4`：其余条目的
/// 「本切片应执行的动作」（`E10 + A3` / `E11 + A3` / `E13 + A4`）即使计划里能查到也必须被拒。
#[tokio::test]
async fn execute_rejects_whitelisted_out_entry_action_pairs() {
    let fixture = Fixture::new();
    fixture.seed().await;
    let now = future_now();
    let plan = plan_storage_reclaim(
        fixture.ports(now),
        plan_request(&["security/recovery", "security/audit", "crashes"]),
    )
    .await
    .unwrap();
    let before = signature(fixture.dirs.data());

    for (key, action) in [
        ("security/recovery", StorageReclaimActionDto::SweepExpired),
        ("security/audit", StorageReclaimActionDto::SweepExpired),
        ("crashes", StorageReclaimActionDto::PruneToLimit),
    ] {
        let error = execute_storage_reclaim(
            fixture.ports(now),
            execute_request(&plan.plan_id, vec![request(key, action)], true),
        )
        .await
        .unwrap_err();
        assert_eq!(error, "entryNotAllowed", "{} 未接入手动入口，必须拒绝", key);
    }

    assert_eq!(before, signature(fixture.dirs.data()), "拒绝路径零副作用");
}

/// V20 后半：`key` 只能是登记表里的条目 id —— 未登记的子节点名与对账余量
/// （`inventory` 不是条目）在计划与执行两侧都被拒，且不动文件。
#[tokio::test]
async fn inventory_and_unregistered_keys_are_rejected_on_both_sides() {
    let fixture = Fixture::new();
    fixture.seed().await;
    let now = future_now();

    for key in ["inventory", "security/audit/tmp"] {
        let error = plan_storage_reclaim(fixture.ports(now), plan_request(&[key]))
            .await
            .unwrap_err();
        assert_eq!(error, "entryNotAllowed", "计划侧拒绝 {}", key);
    }

    let plan = plan_storage_reclaim(fixture.ports(now), plan_request(&[DERIVED_KEY]))
        .await
        .unwrap();
    let before_derived = signature(&fixture.dirs.attachments());

    for key in ["inventory", "security/audit/tmp"] {
        let error = execute_storage_reclaim(
            fixture.ports(now),
            execute_request(
                &plan.plan_id,
                vec![request(key, StorageReclaimActionDto::RebuildCache)],
                true,
            ),
        )
        .await
        .unwrap_err();
        assert_eq!(error, "entryNotAllowed", "执行侧拒绝 {}", key);
    }

    assert_eq!(
        before_derived,
        signature(&fixture.dirs.attachments()),
        "拒绝路径不得动文件"
    );
}

/// 串行闸门：已有执行在跑时第二次立即回 `busy`，且不产生任何副作用。
#[tokio::test]
async fn concurrent_execution_returns_busy_without_side_effects() {
    let fixture = Fixture::new();
    fixture.seed().await;
    let now = future_now();
    let plan = plan_storage_reclaim(fixture.ports(now), plan_request(&[DERIVED_KEY, INDEX_KEY]))
        .await
        .unwrap();
    let before_derived = signature(&fixture.dirs.attachments());
    let before_index = signature(&fixture.dirs.indexes());

    // 用闸门本身模拟「第一条执行尚未结束」：不必真的并发，语义等价。
    let running = fixture
        .plans
        .begin_execution()
        .expect("首个执行应能抢到闸门");
    let error = execute_storage_reclaim(
        fixture.ports(now),
        execute_request(
            &plan.plan_id,
            vec![
                request(DERIVED_KEY, StorageReclaimActionDto::RebuildCache),
                request(INDEX_KEY, StorageReclaimActionDto::RebuildCache),
            ],
            true,
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(error, "busy", "第二次执行必须回 busy");
    drop(running);

    assert_eq!(
        before_derived,
        signature(&fixture.dirs.attachments()),
        "busy 路径不得动文件"
    );
    assert_eq!(
        before_index,
        signature(&fixture.dirs.indexes()),
        "busy 路径不得动文件"
    );

    // 闸门释放后同一条计划仍可正常执行，说明 busy 不是永久锁死。
    let result = execute_storage_reclaim(
        fixture.ports(now),
        execute_request(
            &plan.plan_id,
            vec![request(INDEX_KEY, StorageReclaimActionDto::RebuildCache)],
            true,
        ),
    )
    .await
    .unwrap();
    assert_eq!(result.entries[0].status, StorageReclaimStatusDto::Done);
}

#[tokio::test]
async fn execute_removes_derived_orphans_and_index_files() {
    let fixture = Fixture::new();
    let (derived_bytes, index_bytes) = fixture.seed().await;
    let now = future_now();
    let plan = plan_storage_reclaim(fixture.ports(now), plan_request(&[DERIVED_KEY, INDEX_KEY]))
        .await
        .unwrap();

    let result = execute_storage_reclaim(
        fixture.ports(now),
        execute_request(
            &plan.plan_id,
            vec![
                request(DERIVED_KEY, StorageReclaimActionDto::RebuildCache),
                request(INDEX_KEY, StorageReclaimActionDto::RebuildCache),
            ],
            true,
        ),
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 2, "与请求顺序一致");
    assert_eq!(result.entries[0].key, DERIVED_KEY);
    assert_eq!(result.entries[0].status, StorageReclaimStatusDto::Done);
    assert_eq!(result.entries[0].removed_count, 1);
    assert_eq!(result.entries[0].removed_bytes, derived_bytes);
    assert_eq!(result.entries[1].key, INDEX_KEY);
    assert_eq!(result.entries[1].status, StorageReclaimStatusDto::Done);
    assert_eq!(result.entries[1].removed_count, 2);
    assert_eq!(result.entries[1].removed_bytes, index_bytes);

    assert!(
        signature(&fixture.dirs.indexes()).is_empty(),
        "索引文件已删净"
    );
    assert_eq!(
        fixture.store.list_derived().await.unwrap(),
        Vec::new(),
        "孤儿侧车已删净"
    );

    // 计划仍然有效：再执行一次时判据已不成立，计入 skipped 而不是失败（V9）。
    let again = execute_storage_reclaim(
        fixture.ports(now),
        execute_request(
            &plan.plan_id,
            vec![request(DERIVED_KEY, StorageReclaimActionDto::RebuildCache)],
            true,
        ),
    )
    .await
    .unwrap();
    assert_eq!(again.entries[0].status, StorageReclaimStatusDto::Skipped);
    assert_eq!(again.entries[0].removed_count, 0);
    assert_eq!(again.entries[0].failed_count, 0);
}

/// 单条失败隔离（V9）：`E4` 整条失败时同批的 `E2` 照常 `done`，不整批回滚。
///
/// 注入方式用「计划后被改成非目录」的真实文件系统状态：把索引根目录替换成同名
/// 文件，执行侧 `read_dir` 拿到 `NotADirectory`，记成读失败而不是删除失败。
/// 不用 mock，也不依赖平台特有的共享模式。
#[tokio::test]
async fn single_entry_failure_does_not_block_other_entries() {
    let fixture = Fixture::new();
    let (derived_bytes, _) = fixture.seed().await;
    let index_root = fixture.dirs.indexes();

    let now = future_now();
    let plan = plan_storage_reclaim(fixture.ports(now), plan_request(&[DERIVED_KEY, INDEX_KEY]))
        .await
        .unwrap();

    std::fs::remove_dir_all(&index_root).unwrap();
    std::fs::write(&index_root, b"not a directory").unwrap();

    let result = execute_storage_reclaim(
        fixture.ports(now),
        execute_request(
            &plan.plan_id,
            vec![
                request(DERIVED_KEY, StorageReclaimActionDto::RebuildCache),
                request(INDEX_KEY, StorageReclaimActionDto::RebuildCache),
            ],
            true,
        ),
    )
    .await
    .unwrap();

    assert_eq!(
        result.entries[0].status,
        StorageReclaimStatusDto::Done,
        "另一条的失败不得影响 `E2`"
    );
    assert_eq!(result.entries[0].removed_bytes, derived_bytes);
    assert_eq!(
        result.entries[1].status,
        StorageReclaimStatusDto::Failed,
        "一个候选都没删掉 → failed，而不是静默 done"
    );
    assert_eq!(result.entries[1].removed_count, 0);
    assert_eq!(result.entries[1].failed_count, 1, "读目录失败计一次");
    assert_eq!(result.entries[1].errors.len(), 1, "失败原因如实上报");
}

/// V7：`expectedBytes` 是**计划时刻的预览**，执行侧重新收集候选——计划后新增的过期文件
/// 会进入候选，因此 `removedBytes > expectedBytes` 是可解释的偏差，不是账目错误。
///
/// 反向也必须成立：计划里预览到、执行前被外部删掉的文件只会让 `removedBytes` 偏小，
/// 不会让执行去删不存在的东西。
#[tokio::test]
async fn execution_resamples_so_removed_bytes_can_deviate_from_expected_bytes() {
    let fixture = Fixture::new();
    let root = fixture.dirs.tool_results();
    write_expired(&root.join("a.txt"), b"0123456789", 40);
    write_expired(&root.join("b.txt"), b"01234", 40);

    let storage: Arc<dyn ToolResultStorage> =
        Arc::new(FileToolResultStorage::new(&root).with_retention(30, future_now()));
    let now = future_now();
    let plan = plan_storage_reclaim(
        fixture.ports_with(now, Some(Arc::clone(&storage))),
        plan_request(&[TOOL_RESULTS_KEY]),
    )
    .await
    .unwrap();
    let planned = &plan.entries[0];
    assert_eq!(planned.key, TOOL_RESULTS_KEY);
    assert_eq!(planned.eligibility, StorageEligibilityDto::Eligible);
    assert_eq!(planned.reason, Some(StorageReclaimReasonDto::Expired));
    assert_eq!(planned.expected_count, Some(2), "预览只算采样时刻的候选");
    assert_eq!(planned.expected_bytes, Some(15));
    assert_eq!(planned.confirmation, StorageConfirmLevelDto::Confirm);
    assert_eq!(plan.total_reclaimable_bytes, Some(15));

    // 计划之后又落了一份过期文件：预览里没有它。
    write_expired(&root.join("c.txt"), b"0123456", 45);
    // 另一份预览到的文件被外部删掉：执行侧不该把它计成失败。
    std::fs::remove_file(root.join("b.txt")).unwrap();

    let result = execute_storage_reclaim(
        fixture.ports_with(now, Some(storage)),
        execute_request(
            &plan.plan_id,
            vec![request(
                TOOL_RESULTS_KEY,
                StorageReclaimActionDto::SweepExpired,
            )],
            true,
        ),
    )
    .await
    .unwrap();
    let entry = &result.entries[0];
    assert_eq!(entry.status, StorageReclaimStatusDto::Done);
    assert_eq!(entry.removed_count, 2, "a.txt + 计划后新增的 c.txt");
    assert_eq!(entry.removed_bytes, 17);
    assert_eq!(
        entry.removed_bytes,
        planned.expected_bytes.unwrap() + 7 - 5,
        "偏差完全可解释：计划后新增的 c.txt（+7）减去计划后消失的 b.txt（-5）"
    );
    assert_eq!(entry.failed_count, 0, "预览到但已不存在的文件不算失败");
    assert_eq!(entry.skipped_count, 0);
    assert!(
        signature(&root).is_empty(),
        "执行后候选全部删净（含预览没见到的那份）"
    );
}

/// V7：不可回收条目没有「可回收量」可言，`expectedCount` / `expectedBytes` 恒 `null`，
/// 确认档位恒 `none`；只有 `eligible` 条目才给数字（§3.6 硬约束 5）。
#[tokio::test]
async fn non_eligible_entries_report_no_expected_numbers() {
    let fixture = Fixture::new();
    let now = future_now();
    let plan = plan_storage_reclaim(
        fixture.ports(now),
        plan_request(&["mascot-assets", "crashes"]),
    )
    .await
    .unwrap();

    let mascot = &plan.entries[0];
    assert_eq!(mascot.eligibility, StorageEligibilityDto::NotEligible);
    assert_eq!(mascot.reason, Some(StorageReclaimReasonDto::UserData));
    assert_eq!(mascot.expected_count, None, "不可回收不给条数");
    assert_eq!(mascot.expected_bytes, None, "不可回收不给字节");
    assert_eq!(mascot.confirmation, StorageConfirmLevelDto::None);

    let crashes = &plan.entries[1];
    assert_eq!(crashes.eligibility, StorageEligibilityDto::NotImplemented);
    assert_eq!(crashes.reason, Some(StorageReclaimReasonDto::NoReclaimApi));
    assert_eq!(crashes.expected_count, None);
    assert_eq!(crashes.expected_bytes, None);
    assert_eq!(crashes.confirmation, StorageConfirmLevelDto::None);

    assert_eq!(
        plan.total_reclaimable_bytes,
        Some(0),
        "没有 eligible 条目时合计是 0，不是 null（null 只表示「有条目算不出」）"
    );
}

/// V12：`E3` 的保留天数读不到时回退 `0`（不是「无限保留」也不是「默认 30 天」）——
/// 计划判 `policyDisabled`、执行 `skipped`，一个文件都不动。
#[tokio::test]
async fn retention_zero_is_policy_disabled_and_deletes_nothing() {
    let fixture = Fixture::new();
    let root = fixture.dirs.tool_results();
    write_expired(&root.join("old.txt"), b"old", 400);

    let storage: Arc<dyn ToolResultStorage> = Arc::new(FileToolResultStorage::new(&root));
    let now = future_now();
    let plan = plan_storage_reclaim(
        fixture.ports_with(now, Some(Arc::clone(&storage))),
        plan_request(&[TOOL_RESULTS_KEY]),
    )
    .await
    .unwrap();
    let planned = &plan.entries[0];
    assert_eq!(planned.eligibility, StorageEligibilityDto::NotEligible);
    assert_eq!(
        planned.reason,
        Some(StorageReclaimReasonDto::PolicyDisabled)
    );
    assert_eq!(planned.expected_count, None);
    assert_eq!(planned.expected_bytes, None);
    assert_eq!(planned.confirmation, StorageConfirmLevelDto::None);

    let before = signature(&root);
    let result = execute_storage_reclaim(
        fixture.ports_with(now, Some(storage)),
        execute_request(
            &plan.plan_id,
            vec![request(
                TOOL_RESULTS_KEY,
                StorageReclaimActionDto::SweepExpired,
            )],
            true,
        ),
    )
    .await
    .unwrap();
    let entry = &result.entries[0];
    assert_eq!(
        entry.status,
        StorageReclaimStatusDto::Skipped,
        "策略禁用不是失败，也不能按旧天数删"
    );
    assert_eq!(entry.removed_count, 0);
    assert_eq!(entry.failed_count, 0);
    assert_eq!(before, signature(&root), "策略回退 0 时零副作用");
}

/// V12 后半：`E11` 没有补录游标（provider 未注入、`auditPolicy` 取不到）时不得判
/// `eligible`——游标不可得即判据不可算，只能停在 `ownerManaged` 且不给可回收数字。
#[tokio::test]
async fn audit_without_cursor_is_never_eligible() {
    let fixture = Fixture::new();
    let now = future_now();
    let plan = plan_storage_reclaim(fixture.ports(now), plan_request(&["security/audit"]))
        .await
        .unwrap();
    let entry = &plan.entries[0];

    assert_eq!(entry.key, "security/audit");
    assert_ne!(
        entry.eligibility,
        StorageEligibilityDto::Eligible,
        "无游标时不得判 eligible"
    );
    assert_eq!(entry.eligibility, StorageEligibilityDto::OwnerManaged);
    assert_eq!(entry.reason, Some(StorageReclaimReasonDto::Expired));
    assert_eq!(
        entry.policy.retention_days, None,
        "取不到游标时不留默认天数"
    );
    assert_eq!(entry.expected_count, None, "判据不可算 → 不给条数");
    assert_eq!(entry.expected_bytes, None, "判据不可算 → 不给字节");
    assert_eq!(entry.confirmation, StorageConfirmLevelDto::None);
}
