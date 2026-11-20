//! 可选能力 provider 的唯一缝（I61 §3.10）：`E7`–`E11` 的 stats 由外部注入。
//!
//! 本文件**无条件编译**，trait 与别名都不受 `#[cfg(feature = ...)]` 门控：
//! 关闭特性构建靠「`try_state` 取不到」写降级分支，而不是靠编译期裁剪。
//! 参数与返回值只用普通数据，密码库域 crate 的类型一个都不能出现在这里
//! （否则关闭特性构建直接编不过，G1）。

use std::sync::{Arc, Mutex, MutexGuard};

/// 单个 owner 的观测快照：`Option` 表达「取不到」，禁止用 `0` 冒充（G6）。
///
/// owner 侧把「目录不存在」映射成 `Some(0)`（`missing` 语义），把「读失败」
/// 映射成 `Err(ProbeError)`；因此 `None` 只在 owner 确实无法给出该维度时出现。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProbeStats {
    pub bytes: Option<u64>,
    pub count: Option<u64>,
    pub oldest_at: Option<i64>,
    pub newest_at: Option<i64>,
}

/// 取数失败的封闭三值：管理器按固定映射降级，不再细分错误文案。
///
/// - `Locked` → `availability = locked`（存在但受保护读不到，下一步是解锁）；
/// - `Unavailable` → `availability = unavailable`（文件正被替换 / 迁移，下一步是重试）；
/// - `Unreadable` → `availability = unreadable`（权限 / IO 失败）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeError {
    Locked,
    Unavailable,
    Unreadable,
}

/// `E8` 的裁剪回报：只用普通数据，provider 之外不出现任何 owner 类型（G1）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BackupPruneReport {
    pub removed: u64,
    pub removed_bytes: u64,
    pub failed: u64,
}

/// 可选能力的观测 provider；方法**同步阻塞**（管理器在 `spawn_blocking` 里调）。
///
/// `async fn` 在 trait 里不对象安全，且这些 owner 入口本身都是同步 `metadata` /
/// 目录枚举，无需异步。
///
/// 只有两个通用槽（G17）：provider 不认识任何条目 id，只认识「能力名 + 登记表 key」。
/// 于是新增 / 调整 `E7`–`E11` 条目不需要动 trait，也不会出现「第 N 个方法对应第 N 行」的错位。
pub trait StorageOwnerProbe: Send + Sync {
    /// 槽位名：登记表 `RootMount::Feature` 携带的能力名（不是类型名 / feature 名）。
    fn capability(&self) -> &'static str;

    /// 按登记表 `key` 取该条目的观测快照。
    ///
    /// `None` = 该 key 不归本 provider（未登记或属于别的能力），**不得**静默兜底成
    /// 「空目录」；`Some(Err)` = 归本 provider 但取数失败。
    fn stats(&self, key: &'static str) -> Option<Result<ProbeStats, ProbeError>>;

    /// `E11` 的策略自述：保留天数 + 本次注入时刻（秒）。
    ///
    /// `None` = 取不到 `SecurityDbState`，此时 `E11` 的 `policy` 两个数字填 `null`，
    /// 但**不**整条降级（stats 仍可能取到）。
    fn audit_policy(&self) -> Option<(u32, i64)>;

    /// `E8` 的回收入口（T3）：把备份裁到 `keep` 份，返回删除条数与字节。
    ///
    /// `None` = 本 provider 不提供该能力（未装配安全域的构建）：此时 `E8` 保持
    /// `notImplemented` + `noReclaimApi`，管理器不得把它当成「已装配但为空」。
    /// `keep` 由管理器从条目策略（owner 常量）传入，provider 不硬编码份数。
    fn prune_backups(&self, _keep: u32) -> Option<Result<BackupPruneReport, ProbeError>> {
        None
    }
}

/// 注入与取用必须用同一别名：类型不一致会静默取不到，表现为「永远降级」。
pub type StorageOwnerProbeHandle = Arc<dyn StorageOwnerProbe>;

/// 能力名 → provider 的通用槽（G18）：注册表**永远存在**，与特性开关无关。
///
/// 插件在 setup 阶段把自己的 provider 注册进来；关掉特性时没有任何 provider 注册，
/// 查询一律落空 → 采样写 `notImplemented`、对账写 `optionalNotMounted`，
/// 不需要 `#[cfg]` 分支，也不需要 core 知道是哪个 crate 提供的。
#[derive(Default)]
pub struct ProbeRegistry {
    probes: Mutex<Vec<StorageOwnerProbeHandle>>,
}

impl ProbeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个 provider；同一能力名只能注册一次（重复注册返回 `Err` 且保留先到者）。
    pub fn register(&self, probe: StorageOwnerProbeHandle) -> Result<(), String> {
        let capability = probe.capability();
        if capability.is_empty() {
            return Err("capability 不得为空".to_string());
        }
        let mut probes = self.lock();
        if probes.iter().any(|probe| probe.capability() == capability) {
            return Err(format!("capability `{capability}` 已注册"));
        }
        probes.push(probe);
        Ok(())
    }

    /// 按能力名取 provider；未注册返回 `None`（调用方按「能力未挂载」降级）。
    pub fn find(&self, capability: &str) -> Option<StorageOwnerProbeHandle> {
        self.lock()
            .iter()
            .find(|probe| probe.capability() == capability)
            .cloned()
    }

    /// 取一份当前已注册 provider 的快照（注册发生在 setup 阶段，扫描期间不再变）。
    pub fn snapshot(&self) -> Vec<StorageOwnerProbeHandle> {
        self.lock().clone()
    }

    /// 已注册的能力名（去重；注册表保证唯一）。
    pub fn capabilities(&self) -> Vec<&'static str> {
        self.lock().iter().map(|probe| probe.capability()).collect()
    }

    fn lock(&self) -> MutexGuard<'_, Vec<StorageOwnerProbeHandle>> {
        self.probes
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_dirs::registry::SECURITY_OBSERVABILITY;

    /// 假 provider：只认自己声明的能力名与 key 集合，其余一律 `None`。
    struct FakeProbe {
        capability: &'static str,
        served: &'static [&'static str],
    }

    impl StorageOwnerProbe for FakeProbe {
        fn capability(&self) -> &'static str {
            self.capability
        }

        fn stats(&self, key: &'static str) -> Option<Result<ProbeStats, ProbeError>> {
            if !self.served.contains(&key) {
                return None;
            }
            Some(Ok(ProbeStats {
                bytes: Some(1024),
                count: Some(1),
                oldest_at: Some(100),
                newest_at: Some(200),
            }))
        }

        fn audit_policy(&self) -> Option<(u32, i64)> {
            Some((30, 500))
        }
    }

    fn register(
        registry: &ProbeRegistry,
        capability: &'static str,
        served: &'static [&'static str],
    ) {
        registry
            .register(Arc::new(FakeProbe { capability, served }))
            .expect("首次注册必须成功");
    }

    #[test]
    fn empty_registry_answers_nothing() {
        let registry = ProbeRegistry::new();
        assert!(registry.capabilities().is_empty());
        assert!(registry.snapshot().is_empty());
        assert!(registry.find(SECURITY_OBSERVABILITY).is_none());
    }

    #[test]
    fn single_slot_answers_its_keys_only() {
        let registry = ProbeRegistry::new();
        register(&registry, SECURITY_OBSERVABILITY, &["security/security.db"]);

        assert_eq!(registry.capabilities(), vec![SECURITY_OBSERVABILITY]);
        let probe = registry.find(SECURITY_OBSERVABILITY).expect("已注册");
        assert!(probe.stats("security/security.db").is_some());
        assert!(
            probe.stats("security/audit").is_none(),
            "非本 provider 的 key 必须返回 None"
        );
        assert!(registry.find("other").is_none());
    }

    #[test]
    fn two_capabilities_do_not_cross_talk() {
        let registry = ProbeRegistry::new();
        register(&registry, SECURITY_OBSERVABILITY, &["security/security.db"]);
        register(&registry, "other-observability", &["other/root"]);

        let security = registry.find(SECURITY_OBSERVABILITY).unwrap();
        assert!(security.stats("security/security.db").is_some());
        assert!(security.stats("other/root").is_none());

        let other = registry.find("other-observability").unwrap();
        assert!(other.stats("other/root").is_some());
        assert!(other.stats("security/security.db").is_none());
    }

    #[test]
    fn duplicate_capability_is_rejected_without_overwrite() {
        let registry = ProbeRegistry::new();
        register(&registry, SECURITY_OBSERVABILITY, &["security/security.db"]);

        let duplicate = registry.register(Arc::new(FakeProbe {
            capability: SECURITY_OBSERVABILITY,
            served: &["security/audit"],
        }));
        assert!(duplicate.is_err(), "同能力名重复注册必须返回 Err");
        assert_eq!(
            registry.capabilities(),
            vec![SECURITY_OBSERVABILITY],
            "不得重复"
        );

        let probe = registry.find(SECURITY_OBSERVABILITY).unwrap();
        assert!(
            probe.stats("security/audit").is_none(),
            "先注册者不得被覆盖"
        );
    }

    #[test]
    fn empty_capability_is_rejected() {
        let registry = ProbeRegistry::new();
        assert!(registry
            .register(Arc::new(FakeProbe {
                capability: "",
                served: &[],
            }))
            .is_err());
        assert!(registry.capabilities().is_empty());
    }

    #[test]
    fn snapshot_ignores_later_registrations() {
        let registry = ProbeRegistry::new();
        register(&registry, SECURITY_OBSERVABILITY, &["security/security.db"]);
        let snapshot = registry.snapshot();
        register(&registry, "other-observability", &["other/root"]);

        assert_eq!(snapshot.len(), 1, "快照不随后续注册增长");
        assert_eq!(snapshot[0].capability(), SECURITY_OBSERVABILITY);
        assert_eq!(registry.snapshot().len(), 2);
    }
}