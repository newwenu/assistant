use super::types::{PersistedInfo, TruncatedInfo, TruncatedUnit};
use crate::os::footer;

/// `[truncated: showing 2 of 4 lines, 8 of 100 bytes]` → 三态。
///
/// 三态口径（I46 compact §2.2 D2，门禁 3）：
/// - 无 `[truncated:` → `None`；
/// - 含 `total unknown` → `unit = Unknown`，`returned` / `total` 均为 `None`；
/// - 其余为精确态，`returned` / `total` 均有值。
pub fn parse_truncation(text: &str) -> Option<TruncatedInfo> {
    // 前缀取自登记表，生产方改 tag 时这里不会静默失配。
    let prefix = format!("[{}:", footer::TRUNCATED);
    let start = text.rfind(&prefix)?;
    let body = &text[start + prefix.len()..];
    let body = body.split(']').next().unwrap_or_default().trim();

    // 显式未知优先：无法计数时不得保留半套数字（否则「未知」与「精确」无法区分）。
    if body.contains("total unknown") {
        return Some(TruncatedInfo {
            unit: TruncatedUnit::Unknown,
            returned: None,
            total: None,
        });
    }

    let mut returned = None;
    let mut total = None;
    let mut unit = TruncatedUnit::Unknown;
    if let Some(rest) = body.strip_prefix("showing ") {
        let mut halves = rest.splitn(2, " of ");
        if let (Some(returned_text), Some(total_text)) = (halves.next(), halves.next()) {
            returned = returned_text.trim().parse::<u64>().ok();
            let total_text = total_text.trim();
            let digits: String = total_text
                .chars()
                .take_while(|character| character.is_ascii_digit())
                .collect();
            total = digits.parse::<u64>().ok();
            if total_text.contains("line") {
                unit = TruncatedUnit::Lines;
            } else if total_text.contains("item")
                || total_text.contains("match")
                || total_text.contains("file")
                || total_text.contains("entry")
            {
                // `match` / `file` / `entry` 是条目计数的同义写法，统一归入 `Items`。
                unit = TruncatedUnit::Items;
            } else if total_text.contains("byte") {
                unit = TruncatedUnit::Bytes;
            }
        }
    }
    if unit == TruncatedUnit::Unknown && returned.is_some() && total.is_some() {
        // 旧文本带数字但单位词不认识：归入条目计数。
        // `Unknown` 只保留给显式 `total unknown`，否则门禁 3 的双向等价不成立。
        unit = TruncatedUnit::Items;
    }
    Some(TruncatedInfo {
        unit,
        returned,
        total,
    })
}

/// `<persisted-output path="..." bytes="N">` → 持久化投影（仅文本历史回退用）。
pub fn parse_persisted_tag(text: &str) -> Option<PersistedInfo> {
    let start = text.find("<persisted-output")?;
    let body = &text[start..];
    let end = body.find('>')?;
    let tag = &body[..end];
    let path = attribute(tag, "path")?;
    let bytes = attribute(tag, "bytes").and_then(|value| value.parse::<u64>().ok());
    Some(PersistedInfo { path, bytes })
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{}=\"", name);
    let start = tag.find(&needle)? + needle.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
