//! 样本槽位寻址：请求与采样只认条目 `key`，位置由视图清单现算（不写下标常量）。
//!
//! 视图清单里 core 条目在前、分区条目在后，条数随已挂载分区变化（X15），
//! 因此所有寻址都必须带清单，不能再靠固定长度的数组下标。

use super::{EntrySample, EntryView};

/// key → 样本槽位；清单里没有的 key 返回 `None`（未挂载 = 行不存在）。
pub fn index_of_key(views: &[EntryView], key: &str) -> Option<usize> {
    views.iter().position(|view| view.key == key)
}

/// 条目 id → 样本槽位；清单里没有该条目返回 `None`。
pub fn index_of_entry(views: &[EntryView], entry: &str) -> Option<usize> {
    views.iter().position(|view| view.entry == entry)
}

/// 按视图取样本；视图与样本由同一清单产出，槽位必定对齐。
pub fn sample_for<'a>(
    views: &[EntryView],
    samples: &'a [EntrySample],
    key: &str,
) -> &'a EntrySample {
    let slot = index_of_key(views, key).expect("视图清单必须包含该 key");
    &samples[slot]
}

/// 按条目 id 覆写样本：条目 id 是对外契约，槽位现算。
pub fn set_sample(
    views: &[EntryView],
    samples: &mut [EntrySample],
    entry: &str,
    sample: EntrySample,
) {
    let slot = views
        .iter()
        .position(|view| view.entry == entry)
        .expect("视图清单必须包含该条目");
    samples[slot] = sample;
}
