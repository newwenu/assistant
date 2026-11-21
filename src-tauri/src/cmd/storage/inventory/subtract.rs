//! 声明减法（§3.11.3）：纯函数、无 IO；登记值只来自清单里的条目，其余类恒 `0`。

use crate::cmd::storage::entries::{EntrySample, EntryView};
use crate::protocol::dto::{StorageInventoryClassDto, StorageInventoryNodeDto};

use super::classify::layer_of;
use super::RawNode;

/// 减法（§3.11.3，纯函数、无 IO）：登记值只来自清单里首段命中的条目，其余类恒 `0`。
///
/// 清单把 core 与分区条目混在同一序列里（X15），因此这里不再查登记表：
/// 分区条目与会话条目共用同一条「首段命中」规则。
pub fn subtract_declared(
    node: &RawNode,
    views: &[EntryView],
    samples: &[EntrySample],
) -> (Option<u64>, Option<u64>) {
    if node.class != StorageInventoryClassDto::Declared {
        return (Some(0), Some(0));
    }
    let mut bytes: Option<u64> = Some(0);
    let mut count: Option<u64> = Some(0);
    for (view, sample) in views.iter().zip(samples.iter()) {
        if !view
            .segments
            .first()
            .is_some_and(|segment| *segment == node.name)
        {
            continue;
        }
        bytes = match (bytes, sample.bytes) {
            (Some(acc), Some(value)) => Some(acc.saturating_add(value)),
            _ => None,
        };
        count = match (count, sample.count) {
            (Some(acc), Some(value)) => Some(acc.saturating_add(value)),
            _ => None,
        };
    }
    (bytes, count)
}

fn residual(scan: Option<u64>, declared: Option<u64>) -> Option<u64> {
    match (scan, declared) {
        (Some(scan), Some(declared)) => Some(scan.saturating_sub(declared)),
        _ => None,
    }
}

pub(super) fn node_dto(
    node: &RawNode,
    views: &[EntryView],
    samples: &[EntrySample],
) -> StorageInventoryNodeDto {
    let (declared_bytes, declared_count) = subtract_declared(node, views, samples);
    let over_registered = matches!(
        (declared_bytes, node.scan_bytes),
        (Some(declared), Some(scan)) if declared > scan
    );
    StorageInventoryNodeDto {
        name: node.name.clone(),
        class: node.class,
        scope: layer_of(node.scope),
        entry: node.entry.map(str::to_string),
        scan_bytes: node.scan_bytes,
        scan_count: node.scan_count,
        declared_bytes,
        declared_count,
        residual_bytes: residual(node.scan_bytes, declared_bytes),
        residual_count: residual(node.scan_count, declared_count),
        over_registered,
        children: node.children.clone(),
    }
}

pub(super) fn sum_unregistered(
    nodes: &[RawNode],
    pick: impl Fn(&RawNode) -> Option<u64>,
) -> Option<u64> {
    let mut total: Option<u64> = Some(0);
    for node in nodes {
        if node.class != StorageInventoryClassDto::Unregistered {
            continue;
        }
        total = match (total, pick(node)) {
            (Some(acc), Some(value)) => Some(acc.saturating_add(value)),
            _ => None,
        };
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_dirs::registry::RootScope;
    use crate::cmd::storage::entries::{set_sample, views};
    use crate::protocol::dto::StorageAvailabilityDto;

    /// 清单 + 全 `available` 样本：core 8 条每条 `1024` 字节 / 1 个。
    fn fixtures() -> (Vec<EntryView>, Vec<EntrySample>) {
        let views = views(&[]);
        let samples = views
            .iter()
            .map(|_| EntrySample::observed(1024, 1, Some(100), Some(200)))
            .collect();
        (views, samples)
    }

    fn raw(name: &str, class: StorageInventoryClassDto, scan: Option<u64>) -> RawNode {
        RawNode {
            name: name.to_string(),
            scope: RootScope::Data,
            class,
            entry: None,
            scan_bytes: scan,
            scan_count: scan.map(|_| 1),
            children: Vec::new(),
            truncated: false,
        }
    }

    #[test]
    fn subtraction_only_applies_to_declared_nodes() {
        let (views, samples) = fixtures();
        let declared = raw(
            "attachments",
            StorageInventoryClassDto::Declared,
            Some(4_000),
        );
        let (declared_bytes, declared_count) = subtract_declared(&declared, &views, &samples);
        // attachments 首段下有 E1 与 E2 两条登记。
        assert_eq!(declared_bytes, Some(2048));
        assert_eq!(declared_count, Some(2));

        let unregistered = raw("stray", StorageInventoryClassDto::Unregistered, Some(900));
        let (bytes, count) = subtract_declared(&unregistered, &views, &samples);
        assert_eq!(bytes, Some(0), "未登记节点没有减数");
        assert_eq!(count, Some(0));
    }

    #[test]
    fn declared_unknown_degrades_residual_to_null() {
        let (views, mut samples) = fixtures();
        set_sample(
            &views,
            &mut samples,
            "E1",
            EntrySample::degraded(StorageAvailabilityDto::Unreadable),
        );
        let node = raw(
            "attachments",
            StorageInventoryClassDto::Declared,
            Some(4_000),
        );
        let (declared, _) = subtract_declared(&node, &views, &samples);
        assert_eq!(declared, None, "任一登记值为 null 时登记值整体为 null");
        let dto = node_dto(&node, &views, &samples);
        assert_eq!(dto.residual_bytes, None);
        assert!(!dto.over_registered);
    }

    #[test]
    fn over_registered_is_flagged() {
        let (views, mut samples) = fixtures();
        set_sample(
            &views,
            &mut samples,
            "E1",
            EntrySample::observed(5_000, 1, None, None),
        );
        let node = raw(
            "attachments",
            StorageInventoryClassDto::Declared,
            Some(1_000),
        );
        let dto = node_dto(&node, &views, &samples);
        assert!(dto.over_registered, "登记值大于扫描值时报警");
        assert_eq!(dto.residual_bytes, Some(0), "余量不得为负");
    }

    #[test]
    fn unregistered_sums_degrade_to_null() {
        let nodes = vec![
            raw("stray", StorageInventoryClassDto::Unregistered, Some(100)),
            raw("other", StorageInventoryClassDto::Unregistered, None),
        ];
        assert_eq!(sum_unregistered(&nodes, |node| node.scan_bytes), None);

        let nodes = vec![
            raw("stray", StorageInventoryClassDto::Unregistered, Some(100)),
            raw("attachments", StorageInventoryClassDto::Declared, Some(50)),
        ];
        assert_eq!(
            sum_unregistered(&nodes, |node| node.scan_bytes),
            Some(100),
            "declared 不计入未登记总量"
        );
    }
}
