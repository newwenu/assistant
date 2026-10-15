// 参考: OpenBitFun/crates/execution/agent-runtime/src/compression_prefetch.rs — Context Compaction

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

pub const DEFAULT_COMPACTION_THRESHOLD_PERCENT: u8 = 80;
pub const MAX_SUMMARY_REQUEST_TOKENS: u32 = 4096;

pub const DEFAULT_COMPACT_SUMMARY_PROMPT: &str = r#"请为以下对话生成交接摘要，使新对话中的 AI 无需查阅原文即可继续工作。

输出格式（严格遵循）：

## 交接摘要

**目标**：用户的核心目标，1-2 句话

**进展**：
- 已完成的关键步骤，每步一句，按时间顺序

**待办**：
- 未完成的事项，如有

**关键信息**：
- 文件路径、配置值、变量名、错误码等后续必需的具体值
- 每条信息必须写具体值，禁止写"某个文件""某个值"等模糊表述

**工具调用摘要**：
- 已执行的工具名称及关键结果，每条一行
- 如：`read_file → config.rs 内容已读取`、`run_command → 编译通过`

规则：
1. 自包含：摘要中不引用"上文""之前"等指代，所有信息直接给出
2. 具体：文件路径写完整路径，变量名写全名，错误码写原始值
3. 精炼：不记录每步操作细节，只保留关键节点和决策
4. 可操作：粘贴到新对话后 AI 可立即继续，无需追问上下文
5. 工具调用：保留工具名称 + 结果摘要，省略完整输出"#;

static TERMINATING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

pub fn is_terminating(conversation_id: &str) -> bool {
    let guard = TERMINATING.lock().expect("terminating mutex poisoned");
    guard.contains(conversation_id)
}

pub fn set_terminating(conversation_id: &str, value: bool) {
    let mut guard = TERMINATING.lock().expect("terminating mutex poisoned");
    if value {
        guard.insert(conversation_id.to_string());
    } else {
        guard.remove(conversation_id);
    }
}

pub fn should_terminate(
    prompt_tokens: u32,
    context_window_tokens: u32,
    auto_compact: bool,
    threshold_percent: u8,
) -> bool {
    if !auto_compact {
        return false;
    }
    if prompt_tokens == 0 || context_window_tokens == 0 {
        return false;
    }
    let threshold = threshold_percent as u64;
    (prompt_tokens as u64) >= (context_window_tokens as u64) * threshold / 100
}
