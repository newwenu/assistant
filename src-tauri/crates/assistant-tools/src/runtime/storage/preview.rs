/// 单结果落盘阈值：超过即落盘，与单轮预算无关。
pub const PER_TOOL_LIMIT_CHARS: usize = 50_000;

/// 单轮内联预算：所有内联结果回填文本的合计上限。
pub const PER_ROUND_LIMIT_CHARS: usize = 200_000;

/// 落盘回填预览的头部窗口（字符）。
pub const PREVIEW_HEAD_CHARS: usize = 600;

/// 落盘回填预览的尾部窗口（字符）。
pub const PREVIEW_TAIL_CHARS: usize = 600;

/// 落盘后回填的预览字符数（头尾合计）。
pub const PREVIEW_CHARS: usize = PREVIEW_HEAD_CHARS + PREVIEW_TAIL_CHARS;

/// 选择本轮需要落盘的结果。
///
/// 规则：单结果超 `PER_TOOL_LIMIT_CHARS` 直接落盘；其余按字符数**降序**累加，
/// 累计超出 `PER_ROUND_LIMIT_CHARS` 的才落盘。降序保证了落盘数量最小：
/// 先把最大的挪出内联预算，剩下的内联总量才可能装得下。
///
/// 入参与返回都按原始顺序，调用方无需重排。
pub fn select_persist_targets(sizes: &[usize]) -> Vec<bool> {
    select_persist_targets_with_limits(sizes, &[])
}

/// 同上，但允许逐条覆盖单结果阈值（`None` = 用全局 `PER_TOOL_LIMIT_CHARS`）。
///
/// `limits` 短于 `sizes` 时缺位按 `None` 处理：调用方少给一条不该 panic，
/// 也不该跳过落盘判定。单轮预算不受覆盖影响，仍是硬上限。
pub fn select_persist_targets_with_limits(sizes: &[usize], limits: &[Option<usize>]) -> Vec<bool> {
    let mut persist = vec![false; sizes.len()];
    let mut candidates: Vec<usize> = Vec::new();

    for (index, &size) in sizes.iter().enumerate() {
        let limit = limits
            .get(index)
            .copied()
            .flatten()
            .unwrap_or(PER_TOOL_LIMIT_CHARS);
        if size > limit {
            persist[index] = true;
        } else {
            candidates.push(index);
        }
    }

    candidates.sort_by(|&left, &right| sizes[right].cmp(&sizes[left]));

    let mut inline_budget = 0usize;
    for index in candidates {
        let size = sizes[index];
        if inline_budget + size > PER_ROUND_LIMIT_CHARS {
            persist[index] = true;
        } else {
            inline_budget += size;
        }
    }

    persist
}

/// 落盘后的回填文本：引用 + 头 `PREVIEW_HEAD_CHARS` + 省略标记 + 尾 `PREVIEW_TAIL_CHARS`。
///
/// 尾部窗口不可省：工具自己的省略 footer（`[truncated: ...]` / `[encoding: ...]`）
/// 与命令行类输出的错误末行都在原文末尾，只留头等于把它们全部丢掉。
///
/// 标记行**不得复用** `[truncated:` 语法：那是「工具结果被采集侧截断」的专用口径，
/// 复用会让 `compact.truncated` 把「存储预览被裁」误读成「结果不完整」。
pub fn persisted_reference(path: &str, content: &str) -> String {
    let total = content.chars().count();
    if total <= PREVIEW_CHARS {
        return format!("<persisted-output path=\"{}\">\n{}", path, content);
    }

    let head: String = content.chars().take(PREVIEW_HEAD_CHARS).collect();
    let tail: String = content.chars().skip(total - PREVIEW_TAIL_CHARS).collect();
    let omitted = total - PREVIEW_HEAD_CHARS - PREVIEW_TAIL_CHARS;
    format!(
        "<persisted-output path=\"{}\">\n{}\n[… {} chars omitted; full output in the file above …]\n{}",
        path, head, omitted, tail
    )
}

/// 流式落盘后的回填文本：标签 + 省略标记 + 尾窗口（**不取头窗口**）。
///
/// 命令输出的信息密度集中在末尾（失败栈、测试摘要、退出前的现场），头部多是启动横幅
/// 与进度条重绘——因此预览整块取自文件末，不与头部分享预算。整段生产者
/// [`persisted_reference`] 保留头尾窗口：那里预览的是文档，开头就是内容本身。
///
/// 标签与省略标记必须与 [`persisted_reference`] **逐字节一致**：
/// 两个生产者共用同一个 `parse_persisted_tag` 回退路径。
pub fn persisted_reference_from_tail(path: &str, tail: &str, total_chars: usize) -> String {
    if total_chars <= PREVIEW_CHARS {
        return format!("<persisted-output path=\"{}\">\n{}", path, tail);
    }

    let omitted = total_chars - PREVIEW_CHARS;
    format!(
        "<persisted-output path=\"{}\">\n[… {} chars omitted; full output in the file above …]\n{}",
        path, omitted, tail
    )
}
