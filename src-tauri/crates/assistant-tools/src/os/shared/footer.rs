//! 工具输出 footer 的唯一登记表（I57 §3.3）。
//!
//! footer 是 `[<tag>: <detail>]` 独占一行的尾注，由工具在正文之后追加，**不是文件内容**。
//! 本模块同时承担三件事，且三者必须同源：
//!
//! - **生成**：生产方（`read` / `super_read` / `grep` / `request_tool`）用 [`render`]，
//!   tag 只能取自本模块的常量。
//! - **登记**：[`TAGS`] 是白名单的唯一来源，新增 tag 只改这里。
//! - **识别**：[`trailing_marker`] 供 `edit` 在 0 匹配诊断里指出「`old_string` 把 footer
//!   当成了文件内容」。
//!
//! 为什么必须同源：原先生产方各写字符串字面量、`edit` 另存一份白名单，`super_read`
//! 新增的 `index` / `column` 只加在生产方，消费方静默漏判——两侧的一致只能靠单测事后
//! 发现。收敛到一处后，生产方漏登记会直接编译不过（tag 只能来自常量），识别方也读同一份。

/// 编码来源（`read` / `super_read`）。
pub(crate) const ENCODING: &str = "encoding";
/// 含非法字节、未能无损解码的文件（`grep`）。
pub(crate) const NON_UTF8: &str = "non-utf8";
/// 手册清单被裁（`request_tool`）。
pub(crate) const CATALOG: &str = "catalog";
/// 超宽行被切（`read` / `super_read`）。
pub(crate) const LONG_LINES: &str = "long-lines";
/// 行索引本次是新建还是复用（`super_read`）。
pub(crate) const INDEX: &str = "index";
/// 可见段从行内第几列开始（`super_read`）。
pub(crate) const COLUMN: &str = "column";
/// 因不可读而跳过的文件（`grep`）。
pub(crate) const SKIPPED: &str = "skipped";
/// 有损解码的行（`grep`）。
pub(crate) const LOSSY: &str = "lossy";
/// 搜索因时间预算提前停止（`grep`）。
pub(crate) const SEARCH: &str = "search";
/// 输出窗口被预算截断（`read` / `super_read` / `grep` / `list` / `glob`）。
pub(crate) const TRUNCATED: &str = "truncated";
/// 能力升级提示（`read` 建议改用 `super_read`）。
pub(crate) const HINT: &str = "hint";

/// footer tag 白名单。新增 tag 必须先在这里登记，否则 `edit` 不认。
pub(crate) const TAGS: &[&str] = &[
    ENCODING, NON_UTF8, CATALOG, LONG_LINES, INDEX, COLUMN, SKIPPED, LOSSY, SEARCH, TRUNCATED, HINT,
];

/// 生成一行 footer：`[<tag>: <detail>]`。
///
/// 形状与 [`trailing_marker`] 的解析互为逆运算，因此两侧不各写一份格式串。
pub(crate) fn render(tag: &str, detail: impl std::fmt::Display) -> String {
    format!("[{}: {}]", tag, detail)
}

/// 若 `text` 的最后一行是已登记的 footer，返回该行（已 trim）。
///
/// 只识别、不剥离：自动剥离会把「模型把 footer 当文件内容」的误解静默修正为成功。
pub(crate) fn trailing_marker(text: &str) -> Option<&str> {
    let last = text.trim_end_matches(['\n', '\r']).lines().last()?.trim();
    let inner = last.strip_prefix('[')?.strip_suffix(']')?;
    let tag = inner.split_once(':')?.0.trim();
    TAGS.contains(&tag).then_some(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每个登记项都必须能被识别——生成与识别互逆，否则 `edit` 的诊断会漏。
    #[test]
    fn every_registered_tag_round_trips() {
        for tag in TAGS {
            let line = render(tag, "detail");
            assert_eq!(
                trailing_marker(&line),
                Some(line.as_str()),
                "tag '{}' 无法被 trailing_marker 识别",
                tag
            );
        }
    }

    /// 白名单不得有重复项：重复登记意味着两处维护，正是本模块要消灭的形态。
    #[test]
    fn tags_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for tag in TAGS {
            assert!(seen.insert(tag), "tag '{}' 重复登记", tag);
        }
    }

    /// 未登记的 tag 不被当成 footer：白名单是封闭集合，不能靠形状猜测。
    #[test]
    fn unregistered_tags_are_not_footers() {
        assert_eq!(trailing_marker("[unknown: detail]"), None);
        assert_eq!(
            trailing_marker("[code]"),
            None,
            "无 detail 的方括号不是 footer"
        );
        assert_eq!(trailing_marker("plain text"), None);
    }

    /// footer 必须独占最后一行：紧贴正文的方括号内容不是 footer。
    #[test]
    fn only_the_last_line_counts() {
        assert_eq!(
            trailing_marker("body\n[encoding: GBK]"),
            Some("[encoding: GBK]")
        );
        assert_eq!(trailing_marker("[encoding: GBK]\nmore body"), None);
    }
}
