//! 可选功能分区契约（I61 §3.10「可选功能分区自注册」）：core 与功能 crate 之间唯一的 feature 缝。
//!
//! 本文件**无条件编译**：trait、类型与注册表在两种构建里都存在，关闭特性时靠「没有任何分区
//! 注册」走零分区路径，而不是靠编译期裁剪。因此这里不得出现任何功能 crate 类型、feature 名、
//! 能力名或条目 `key` 字面量（G1 / G16 / G19）。
//!
//! 三条形状纪律：①方法一律**同步阻塞**（`async fn` 在 trait 里不对象安全，且这些 owner 入口
//! 本身就是同步 `metadata` / 目录枚举），由管理器在 `spawn_blocking` 内调用；②参数与返回值
//! 只用普通数据（`protocol::dto::storage` 的 DTO 或纯 `u64/i64`）；③条目清单、策略、资格判据、
//! 回收入口全部由分区自述，core 只做通用接收与按 `key` 路由，不写任何功能条目知识。

use std::sync::{Arc, Mutex, MutexGuard};

use crate::app_dirs::registry::{self, RootCoverage, RootScope};
use crate::protocol::dto::{
    ReclaimPolicyDto, StorageAvailabilityDto, StorageEligibilityDto, StoragePolicySourceDto,
    StorageReclaimActionDto, StorageReclaimReasonDto,
};

/// 分区条目的观测四元组：与 core 条目、各 owner 的 `DirStats` 同形状（§3.9）。
///
/// `oldest_at` / `newest_at` 在无文件时为 `None`，不是 `0`；`bytes` / `count` 是实测值，
/// 「取不到」由 [`SectionError`] 表达，不用 `0` 冒充（G6）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SectionStats {
    pub bytes: u64,
    pub count: u64,
    pub oldest_at: Option<i64>,
    pub newest_at: Option<i64>,
}

/// 取数失败的封闭类别（§7.1）。
///
/// 类别由 core 映射成 `availability`（`locked` / `unavailable` / `unreadable`）；
/// 错误细节由分区写自己的日志，不进契约（R-16）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionError {
    /// 存在但受保护读不到：下一步是解锁。
    Locked,
    /// 文件正被替换 / 迁移：下一步是重试。
    Unavailable,
    /// 权限 / IO 失败。
    Unreadable,
}

impl SectionError {
    /// 类别 → `availability`（固定映射，分区不得自定义）。
    pub const fn availability(self) -> StorageAvailabilityDto {
        match self {
            Self::Locked => StorageAvailabilityDto::Locked,
            Self::Unavailable => StorageAvailabilityDto::Unavailable,
            Self::Unreadable => StorageAvailabilityDto::Unreadable,
        }
    }
}

/// 分区回收入口的一次回报：只用普通数据，分区之外不出现任何 owner 类型（G1）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SectionReclaimReport {
    pub removed: u64,
    pub removed_bytes: u64,
    pub failed: u64,
}

/// 条目风险档（§3.6 硬约束 5–6）：core 据此派生 `confirmation`，**不进 wire**。
///
/// 与 `action` 无关：风险档是「删这个东西有多危险」的登记事实，不是「这个动作需要多强确认」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskTier {
    Low,
    Medium,
    High,
    /// 禁止回收（业务数据 / 插件自持）。
    Forbidden,
}

/// 分区条目的静态声明：语义与 §3.2 的 core 条目字段一一对应。
#[derive(Debug, Clone, PartialEq)]
pub struct EntryDecl {
    /// 该条目的唯一动作（`none` 表示不提供手动入口）。
    pub action: StorageReclaimActionDto,
    /// 不随观测态变化的那一档资格；随观测态变化的部分由 [`StorageSection::eligibility`] 给。
    pub eligibility: StorageEligibilityDto,
    pub reason: StorageReclaimReasonDto,
    /// 静态策略档；运行期数值由 [`StorageSection::policy`] 覆盖。
    pub policy: ReclaimPolicyDto,
    pub auto_active: bool,
    pub counts_self_logs: bool,
    /// 风险档：core 的 `confirmation` 由「资格 + 风险档」两步派生。
    pub risk: RiskTier,
    /// 结构性能力缺口的说明（前端原文展示），不写策略数值 / 路径 / 时间点。
    pub note: Option<&'static str>,
}

/// 一条分区条目的完整声明：`decl` 之外的部分就是 core 登记表一行的同名字段。
#[derive(Debug, Clone, PartialEq)]
pub struct SectionEntry {
    /// 条目 id（对外契约，不与 core 条目或其它分区重名）。
    pub entry: &'static str,
    /// 稳定身份，恒等于 `segments` 用 `/` 连接。
    pub key: &'static str,
    /// 段序列；首段即对账单位，必须命中 `registry::OPTIONAL_SEGMENTS`。
    pub segments: &'static [&'static str],
    pub scope: RootScope,
    pub coverage: RootCoverage,
    /// 数据所有者（展示用，不承载路由，G17(a)）。
    pub owner: &'static str,
    /// 该 scope 是否参与余量对账。
    pub inventory: bool,
    pub decl: EntryDecl,
}

/// 分区条目的观测态：`observed` 给四元组或封闭失败类别，`policy` 给策略自述快照。
///
/// `policy` 为 `None` = 本分区此刻给不出策略（判据不可算时由分区据此判 `unknown`）。
#[derive(Debug, Clone, PartialEq)]
pub struct SectionSample {
    pub observed: Result<SectionStats, SectionError>,
    pub policy: Option<ReclaimPolicyDto>,
}

impl SectionSample {
    pub fn from_stats(stats: SectionStats) -> Self {
        Self {
            observed: Ok(stats),
            policy: None,
        }
    }

    pub fn from_error(error: SectionError) -> Self {
        Self {
            observed: Err(error),
            policy: None,
        }
    }

    pub fn with_policy(mut self, policy: ReclaimPolicyDto) -> Self {
        self.policy = Some(policy);
        self
    }
}

/// 可选功能分区；实现方是功能装配缝的适配器（功能 crate 不依赖本 crate）。
pub trait StorageSection: Send + Sync {
    /// 分区标识：只用于诊断日志与去重，不是 feature 名 / 类型名 / 能力名。
    fn id(&self) -> &'static str;

    /// 条目清单（**由分区自述**，core 不另建映射，X15）。
    fn entries(&self) -> &'static [SectionEntry];

    /// 按 `key` 取观测快照；`None` = 该 key 不归本分区（未登记或属于别的分区）。
    fn stats(&self, key: &'static str) -> Option<Result<SectionStats, SectionError>>;

    /// 随观测态变化的资格判据；`None` = 用 `decl.eligibility` 那一档。
    fn eligibility(
        &self,
        _key: &'static str,
        _sample: &SectionSample,
    ) -> Option<(StorageEligibilityDto, StorageReclaimReasonDto)> {
        None
    }

    /// 运行期策略自述（如 owner 的保留天数 / 份数）；`None` = 用 `decl.policy`。
    fn policy(&self, _key: &'static str) -> Option<ReclaimPolicyDto> {
        None
    }

    /// 本分区可执行的动作集合；`None` = 不支持该动作。
    fn reclaim_actions(&self) -> &'static [StorageReclaimActionDto] {
        &[]
    }

    /// 回收入口；`None` = 本分区不提供该 `key + action` 的回收能力。
    fn reclaim(
        &self,
        _key: &'static str,
        _action: StorageReclaimActionDto,
        _policy: Option<ReclaimPolicyDto>,
    ) -> Option<Result<SectionReclaimReport, SectionError>> {
        None
    }
}

/// 注入与取用必须用同一别名：`dyn` 不满足 `Sized`，`Arc` 同时满足 `Send + Sync + 'static`。
pub type StorageSectionHandle = Arc<dyn StorageSection>;

/// 分区的唯一通用接收槽（G18）：注册表**永远存在**，与特性开关无关。
///
/// 关闭特性时没有任何分区注册，查询一律落空 → `entries` 只剩 core 条目、
/// 对账把可选首段归 `optionalNotMounted`，不需要 `#[cfg]` 分支。
#[derive(Default)]
pub struct SectionRegistry {
    sections: Mutex<Vec<StorageSectionHandle>>,
}

impl SectionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个分区：只做**结构校验**，失败返回 `Err` 且不覆盖先注册者。
    ///
    /// 校验项（V22 / V23）：分区标识非空且唯一、条目清单非空、每条 `key == segments.join("/")`、
    /// 首段命中 core 的 `OPTIONAL_SEGMENTS` 磁盘事实、条目 id / `key` 不与 core 条目或其它
    /// 已注册分区重名。**不**校验策略数值或资格判据——那些是分区自己的事。
    pub fn register(&self, section: StorageSectionHandle) -> Result<(), String> {
        let id = section.id();
        if id.is_empty() {
            return Err("分区标识不得为空".to_string());
        }
        let entries = section.entries();
        if entries.is_empty() {
            return Err(format!("分区 `{id}` 的条目清单不得为空"));
        }

        let mut keys: Vec<&'static str> = Vec::with_capacity(entries.len());
        for entry in entries {
            if entry.entry.is_empty() {
                return Err(format!("分区 `{id}` 有条目 id 为空"));
            }
            if entry.segments.is_empty() {
                return Err(format!("分区 `{id}` 的条目 `{}` 段序列为空", entry.entry));
            }
            let joined = entry.segments.join("/");
            if joined != entry.key {
                return Err(format!(
                    "分区 `{id}` 的条目 `{}` key `{}` != segments `{}`",
                    entry.entry, entry.key, joined
                ));
            }
            if !registry::OPTIONAL_SEGMENTS.contains(&entry.segments[0]) {
                return Err(format!(
                    "分区 `{id}` 的条目 `{}` 首段 `{}` 不在 OPTIONAL_SEGMENTS 内",
                    entry.entry, entry.segments[0]
                ));
            }
            if keys.contains(&entry.key) {
                return Err(format!("分区 `{id}` 内 key `{}` 重复", entry.key));
            }
            if let Some(root) = registry::MANAGED_ROOTS
                .iter()
                .find(|root| root.key == entry.key || root.entry.id() == entry.entry)
            {
                return Err(format!(
                    "分区 `{id}` 的条目 `{}` 与 core 条目 `{}` 重名",
                    entry.entry,
                    root.entry.id()
                ));
            }
            keys.push(entry.key);
        }

        let mut sections = self.lock();
        if sections.iter().any(|existing| existing.id() == id) {
            return Err(format!("分区标识 `{id}` 已注册"));
        }
        for existing in sections.iter() {
            for known in existing.entries() {
                if keys.contains(&known.key) {
                    return Err(format!(
                        "分区 `{id}` 的 key `{}` 与分区 `{}` 重名",
                        known.key,
                        existing.id()
                    ));
                }
                if entries.iter().any(|entry| entry.entry == known.entry) {
                    return Err(format!(
                        "分区 `{id}` 的条目 `{}` 与分区 `{}` 重名",
                        known.entry,
                        existing.id()
                    ));
                }
            }
        }
        sections.push(section);
        Ok(())
    }

    /// 取一份当前已注册分区的快照（克隆 `Vec<Arc<…>>` 后立即释放锁）。
    ///
    /// 注册只发生在 `setup` 阶段，扫描 / 采样期间不再变化；快照保证调用方不会在
    /// 遍历条目清单时持锁。
    pub fn snapshot(&self) -> Vec<StorageSectionHandle> {
        self.lock().clone()
    }

    /// 是否没有任何分区注册（关闭特性构建的常态）。
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    fn lock(&self) -> MutexGuard<'_, Vec<StorageSectionHandle>> {
        self.sections
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}

/// 无策略：`source = none`，三个数值全 `None`，`failClosed` 保持 `true`。
pub const fn no_policy() -> ReclaimPolicyDto {
    ReclaimPolicyDto {
        source: StoragePolicySourceDto::None,
        retention_days: None,
        keep_count: None,
        max_bytes: None,
        effective_since: None,
        mutable: false,
        fail_closed: true,
    }
}

/// 常量策略：数值由 owner 常量固定，用户不可改。
pub const fn constant_policy(keep_count: Option<u32>) -> ReclaimPolicyDto {
    ReclaimPolicyDto {
        source: StoragePolicySourceDto::Constant,
        retention_days: None,
        keep_count,
        max_bytes: None,
        effective_since: None,
        mutable: false,
        fail_closed: true,
    }
}

/// owner 自述策略：数值是 owner 的私有常量，管理器不自填默认值（G3）。
pub const fn owner_policy() -> ReclaimPolicyDto {
    ReclaimPolicyDto {
        source: StoragePolicySourceDto::Owner,
        retention_days: None,
        keep_count: None,
        max_bytes: None,
        effective_since: None,
        mutable: false,
        fail_closed: true,
    }
}

/// 配置策略：天数来自配置，可热更新（`effective_since` 由分区在运行期补）。
pub const fn config_policy(retention_days: Option<u32>, mutable: bool) -> ReclaimPolicyDto {
    ReclaimPolicyDto {
        source: StoragePolicySourceDto::Config,
        retention_days,
        keep_count: None,
        max_bytes: None,
        effective_since: None,
        mutable,
        fail_closed: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEG: &str = registry::OPTIONAL_SEGMENTS[0];

    struct FakeSection {
        id: &'static str,
        entries: &'static [SectionEntry],
    }

    impl StorageSection for FakeSection {
        fn id(&self) -> &'static str {
            self.id
        }

        fn entries(&self) -> &'static [SectionEntry] {
            self.entries
        }

        fn stats(&self, key: &'static str) -> Option<Result<SectionStats, SectionError>> {
            self.entries
                .iter()
                .any(|entry| entry.key == key)
                .then_some(Ok(SectionStats::default()))
        }
    }

    fn leaked(entries: Vec<SectionEntry>) -> &'static [SectionEntry] {
        Box::leak(entries.into_boxed_slice())
    }

    fn leaked_str(text: String) -> &'static str {
        Box::leak(text.into_boxed_str())
    }

    fn decl() -> EntryDecl {
        EntryDecl {
            action: StorageReclaimActionDto::None,
            eligibility: StorageEligibilityDto::NotEligible,
            reason: StorageReclaimReasonDto::BusinessData,
            policy: no_policy(),
            auto_active: false,
            counts_self_logs: false,
            risk: RiskTier::Forbidden,
            note: None,
        }
    }

    fn section_entry(
        entry: &'static str,
        segments: &'static [&'static str],
        key: &'static str,
    ) -> SectionEntry {
        SectionEntry {
            entry,
            key,
            segments,
            scope: RootScope::Data,
            coverage: RootCoverage::RecursiveTree,
            owner: "FakeOwner",
            inventory: true,
            decl: decl(),
        }
    }

    /// 合法单段条目：`key` 就是首段本身（避免在测试里写任何带子段的字面量）。
    fn valid_entry(entry: &'static str) -> SectionEntry {
        section_entry(entry, &[SEG], SEG)
    }

    /// 合法两段条目：`key` 由首段 + 子段在运行期拼出，仍满足 `key == segments.join("/")`。
    fn nested_entry(entry: &'static str, child: &'static str) -> SectionEntry {
        let segments = leaked_segments(child);
        let key = leaked_str(format!("{SEG}/{child}"));
        section_entry(entry, segments, key)
    }

    fn leaked_segments(child: &'static str) -> &'static [&'static str] {
        Box::leak(vec![SEG, child].into_boxed_slice())
    }

    fn handle(id: &'static str, entries: &'static [SectionEntry]) -> StorageSectionHandle {
        Arc::new(FakeSection { id, entries })
    }

    #[test]
    fn empty_registry_has_no_sections() {
        let registry = SectionRegistry::new();
        assert!(registry.is_empty());
        assert!(registry.snapshot().is_empty());
    }

    #[test]
    fn first_segment_outside_optional_segments_is_rejected() {
        let registry = SectionRegistry::new();
        let segments = Box::leak(vec!["elsewhere"].into_boxed_slice());
        let entries = leaked(vec![section_entry("X1", segments, "elsewhere")]);
        let error = registry
            .register(handle("fake", entries))
            .expect_err("首段不在 OPTIONAL_SEGMENTS 必须被拒");
        assert!(error.contains("OPTIONAL_SEGMENTS"), "{error}");
    }

    #[test]
    fn key_must_equal_segments_join() {
        let registry = SectionRegistry::new();
        let entries = leaked(vec![section_entry(
            "X1",
            leaked_segments("child"),
            "mismatch",
        )]);
        let error = registry
            .register(handle("fake", entries))
            .expect_err("key 与 segments 不一致必须被拒");
        assert!(error.contains("segments"), "{error}");
    }

    #[test]
    fn duplicate_core_entry_is_rejected() {
        let registry = SectionRegistry::new();
        let core = &registry::MANAGED_ROOTS[0];
        let entries = leaked(vec![valid_entry(core.entry.id())]);
        let error = registry
            .register(handle("fake", entries))
            .expect_err("与 core 条目重名必须被拒");
        assert!(error.contains("core"), "{error}");
    }

    #[test]
    fn empty_entry_list_is_rejected() {
        let registry = SectionRegistry::new();
        let error = registry
            .register(handle("fake", &[]))
            .expect_err("空清单必须被拒");
        assert!(error.contains("不得为空"), "{error}");
    }

    #[test]
    fn duplicate_section_id_and_key_are_rejected() {
        let registry = SectionRegistry::new();
        registry
            .register(handle("first", leaked(vec![valid_entry("X1")])))
            .expect("首个分区应注册成功");

        let error = registry
            .register(handle("first", leaked(vec![nested_entry("X2", "child")])))
            .expect_err("分区标识重复必须被拒");
        assert!(error.contains("已注册"), "{error}");

        let error = registry
            .register(handle("second", leaked(vec![valid_entry("X2")])))
            .expect_err("条目 key 重复必须被拒");
        assert!(error.contains("重名"), "{error}");

        let error = registry
            .register(handle("third", leaked(vec![valid_entry("X1")])))
            .expect_err("条目 id 重复必须被拒");
        assert!(error.contains("重名"), "{error}");
    }

    #[test]
    fn snapshot_does_not_grow_after_later_registration() {
        let registry = SectionRegistry::new();
        registry
            .register(handle("first", leaked(vec![valid_entry("X1")])))
            .expect("首个分区应注册成功");
        let snapshot = registry.snapshot();
        assert_eq!(snapshot.len(), 1);

        registry
            .register(handle("second", leaked(vec![nested_entry("X2", "child")])))
            .expect("第二个分区应注册成功");
        assert_eq!(snapshot.len(), 1, "快照是当时那份，不随后续注册增长");
        assert_eq!(registry.snapshot().len(), 2);
    }

    #[test]
    fn section_error_maps_to_fixed_availability() {
        assert_eq!(
            SectionError::Locked.availability(),
            StorageAvailabilityDto::Locked
        );
        assert_eq!(
            SectionError::Unavailable.availability(),
            StorageAvailabilityDto::Unavailable
        );
        assert_eq!(
            SectionError::Unreadable.availability(),
            StorageAvailabilityDto::Unreadable
        );
    }
}
