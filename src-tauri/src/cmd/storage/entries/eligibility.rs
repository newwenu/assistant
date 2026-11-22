//! 资格与确认档位派生：把观测态 + 视图自述折算成 `eligibility` / `reason` / `confirmation`。

use crate::app_dirs::registry::CoreEntry;
use crate::cmd::storage::section::RiskTier;
use crate::protocol::dto::{
    StorageAvailabilityDto, StorageConfirmLevelDto, StorageEligibilityDto, StorageReclaimReasonDto,
};

use super::{EntrySample, EntryView};

/// 资格按「当时代码真实可执行性」定档（§3.1 末条硬规则）。
///
/// 分区自述的资格优先（`EntrySample::eligibility`）——锁态 / 观测失败该给哪一档由分区决定
/// （安全域 repo 锁态 `locked`、DB 观测失败 `notEligible + unknown`，§7.1 特例 2）；core 侧随
/// 观测态变化的只有 `E1` / `E3`（判据来自观测数字与 owner 自述的策略）与 `E14`（运行期由
/// WebView2 持有 profile 句柄），其余条目一律取视图里那一档。core **不**按 `availability`
/// 反推资格：把 `unavailable` 说成 `locked` 会让用户去解锁一个本来就不受保护读不到的东西
/// （§7.1 特例 2 明令禁止）。
pub(super) fn eligibility_for(
    view: &EntryView,
    sample: &EntrySample,
) -> (StorageEligibilityDto, Option<StorageReclaimReasonDto>) {
    if let Some((eligibility, reason)) = sample.eligibility {
        return (eligibility, Some(reason));
    }
    // 动态判据只属于 `E1` / `E3` / `E14`：其余 core 条目与分区条目一律取视图那一档。
    // 枚举穷尽（含 `None`），新增 core 条目时编译器会点名这里，不会静默退化。
    let dynamic = match view.core {
        Some(CoreEntry::Attachments) => orphans_eligibility(sample),
        Some(CoreEntry::ToolResults) => tool_results_eligibility(sample),
        Some(CoreEntry::WebViewProfile) => webview_eligibility(sample),
        Some(
            CoreEntry::AttachmentsDerived
            | CoreEntry::ToolIndex
            | CoreEntry::MascotAssets
            | CoreEntry::Database
            | CoreEntry::Logs
            | CoreEntry::Crashes
            | CoreEntry::Config
            | CoreEntry::OrbConfig
            | CoreEntry::MascotConfig,
        )
        | None => None,
    };
    if let Some((eligibility, reason)) = dynamic {
        return (eligibility, Some(reason));
    }
    if view.eligibility == StorageEligibilityDto::Eligible {
        // `eligible` 的 reason 可空，但统一填该条目的唯一解（§3.1），前端不必分支。
        return (StorageEligibilityDto::Eligible, Some(view.reason));
    }
    (view.eligibility, Some(view.reason))
}

/// 确认档位：第一步看资格（`eligible` 之外恒 `none`），第二步才按风险档（§3.6 硬约束 5–6）。
///
/// 风险档由条目自述，不由动作推断：低 / 中 → `confirm`，高 → `strongConfirm`，
/// 禁止档即便出现 `eligible` 也不给确认入口（`none`）。
pub(super) fn confirmation_for(
    view: &EntryView,
    eligibility: StorageEligibilityDto,
) -> StorageConfirmLevelDto {
    if eligibility != StorageEligibilityDto::Eligible {
        return StorageConfirmLevelDto::None;
    }
    match view.risk {
        RiskTier::Low | RiskTier::Medium => StorageConfirmLevelDto::Confirm,
        RiskTier::High => StorageConfirmLevelDto::StrongConfirm,
        RiskTier::Forbidden => StorageConfirmLevelDto::None,
    }
}

/// `E1` 的资格随孤儿数量变化（§3.1）：有孤儿才 `eligible`，无孤儿 `notEligible` + `referenced`。
///
/// 孤儿数量只取 `reclaimable`（与顶层孤儿列表同源）。三态必须分开：`Some(n>0)` 才是
/// `orphan`；`Some(0)` 是「算出来确实没有孤儿」→ `referenced`；`None` 是「引用集合取不到、
/// 判据不可算」→ `unknown`（§3.4 Q2）。**不得**把 `None` 当 0 条孤儿，那会把「不知道」
/// 说成「仍被引用」。
fn orphans_eligibility(
    sample: &EntrySample,
) -> Option<(StorageEligibilityDto, StorageReclaimReasonDto)> {
    match sample.availability {
        StorageAvailabilityDto::Available | StorageAvailabilityDto::Missing => {
            let judged = match sample.reclaimable {
                Some((_, count)) if count > 0 => (
                    StorageEligibilityDto::Eligible,
                    StorageReclaimReasonDto::Orphan,
                ),
                Some(_) => (
                    StorageEligibilityDto::NotEligible,
                    StorageReclaimReasonDto::Referenced,
                ),
                None => (
                    StorageEligibilityDto::NotEligible,
                    StorageReclaimReasonDto::Unknown,
                ),
            };
            Some(judged)
        }
        StorageAvailabilityDto::NotImplemented => Some((
            StorageEligibilityDto::NotImplemented,
            StorageReclaimReasonDto::NoReclaimApi,
        )),
        _ => Some((
            StorageEligibilityDto::NotEligible,
            StorageReclaimReasonDto::Unknown,
        )),
    }
}

/// `E3` 的资格随 owner 自述的保留天数变化（§3.1）：`> 0` 才 `eligible`。
///
/// 观测不可用时不推断：取不到数 → `notEligible` + `unknown`（§3.4 的 Q2），
/// 绝不因「大概能删」而提前标 `eligible`。
fn tool_results_eligibility(
    sample: &EntrySample,
) -> Option<(StorageEligibilityDto, StorageReclaimReasonDto)> {
    match sample.availability {
        StorageAvailabilityDto::Available | StorageAvailabilityDto::Missing => {
            let days = sample
                .policy
                .as_ref()
                .and_then(|policy| policy.retention_days)
                .unwrap_or(0);
            if days > 0 {
                Some((
                    StorageEligibilityDto::Eligible,
                    StorageReclaimReasonDto::Expired,
                ))
            } else {
                Some((
                    StorageEligibilityDto::NotEligible,
                    StorageReclaimReasonDto::PolicyDisabled,
                ))
            }
        }
        _ => Some((
            StorageEligibilityDto::NotEligible,
            StorageReclaimReasonDto::Unknown,
        )),
    }
}

/// `E14`：资格必须保持 `eligible`，否则前端没有清理入口，待执行标记永远写不进去
/// （`entryView` 只在 `eligible` 时给按钮）。
///
/// 「运行期删不掉」不是资格问题，而是**执行时机**问题：WebView2 持有 profile 句柄，
/// 因此执行器只登记待执行、返回 `scheduled`，由下次启动在 `Builder::run` 之前真正删除
/// （§13.3.5）。把资格说成 `locked` 会让用户永远无法登记，与「登记即管理」相反。
///
/// 观测失败时不推断：取不到数 → `notEligible` + `unknown`（fail-closed），
/// 不给一个连大小都不知道的条目开清理入口。
fn webview_eligibility(
    sample: &EntrySample,
) -> Option<(StorageEligibilityDto, StorageReclaimReasonDto)> {
    match sample.availability {
        StorageAvailabilityDto::Available | StorageAvailabilityDto::Missing => Some((
            StorageEligibilityDto::Eligible,
            StorageReclaimReasonDto::DerivedCache,
        )),
        _ => Some((
            StorageEligibilityDto::NotEligible,
            StorageReclaimReasonDto::Unknown,
        )),
    }
}
