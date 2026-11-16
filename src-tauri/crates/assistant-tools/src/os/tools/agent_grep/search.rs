use globset::GlobSet;
use grep_regex::RegexMatcher;
use grep_searcher::{BinaryDetection, Encoding as SearcherEncoding, SearcherBuilder};
use std::path::Path;
use std::time::Instant;

use crate::os::walk;

use super::outcome::SearchOutcome;
use super::probe::probe_non_utf8;
use super::sink::FileSink;
use super::MAX_FILESIZE;

/// 同步搜索，必须在 `spawn_blocking` 内调用。
///
/// 内部 deadline 早于外层超时：`spawn_blocking` 的线程无法被外层取消，
/// 到点必须自己停下并返回已收集结果，否则外层只能放弃 future 而线程继续空转。
pub(super) fn search(
    base: &Path,
    matcher: &RegexMatcher,
    include: Option<&GlobSet>,
    context: u64,
    encoding: Option<SearcherEncoding>,
    deadline: Instant,
) -> Result<SearchOutcome, String> {
    let mut outcome = SearchOutcome::new();
    // 显式指定 `encoding` 时整个 scope 已按该编码转码，非 UTF-8 文件也被搜到了，
    // 探测结论没有信息量，跳过（[grep §3.4]）。
    let explicit_encoding = encoding.is_some();

    // `binary_detection` 默认是 `None`（等于不做检测），必须显式开启：
    // 否则二进制文件会被当成文本，产出乱码命中行（上级 §14.7）。
    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .before_context(context as usize)
        .after_context(context as usize)
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .encoding(encoding)
        .build();

    for entry in walk::builder(base).build() {
        if outcome.stopped() {
            break;
        }
        if Instant::now() >= deadline {
            outcome.timed_out = true;
            break;
        }

        let Ok(entry) = entry else {
            continue;
        };
        let relative = match decide_file(&entry, base, include) {
            FileDecision::Search(relative) => relative,
            FileDecision::Large => {
                outcome.large_skipped += 1;
                continue;
            }
            FileDecision::Skip => continue,
        };

        let mut sink = FileSink::new(relative, deadline, &mut outcome);
        let search_result = searcher.search_path(matcher, entry.path(), &mut sink);
        let failed = search_result.is_err();
        sink.finish(failed);
        drop(sink);
        if failed {
            outcome.unreadable += 1;
        }
    }

    // 冷路径：只在零命中、未超时且未显式指定 `encoding` 时执行，热路径零成本。
    if outcome.match_count == 0 && !outcome.timed_out && !explicit_encoding {
        outcome.probe = Some(probe_non_utf8(base, include, deadline));
    }

    Ok(outcome)
}

pub(super) enum FileDecision {
    /// 参与搜索，携带相对 `base` 的 `/` 分隔路径（`include` 与输出都用它）。
    Search(String),
    /// 超过单文件体积上限：不搜、不探测，计入 `largeSkipped`。
    Large,
    /// 目录、符号链接、`include` 不匹配。
    Skip,
}

/// 统一的「这个条目要不要搜」判定：普通文件 + 体积上限 + `include` 过滤。
pub(super) fn decide_file(
    entry: &ignore::DirEntry,
    base: &Path,
    include: Option<&GlobSet>,
) -> FileDecision {
    if !entry
        .file_type()
        .map(|kind| kind.is_file())
        .unwrap_or(false)
    {
        return FileDecision::Skip;
    }
    if entry.metadata().map(|meta| meta.len()).unwrap_or(0) > MAX_FILESIZE {
        return FileDecision::Large;
    }
    let relative = match entry.path().strip_prefix(base) {
        Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
        Err(_) => return FileDecision::Skip,
    };
    match include {
        Some(include) if !include.is_match(&relative) => FileDecision::Skip,
        _ => FileDecision::Search(relative),
    }
}
