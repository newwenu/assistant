// 参考: OpenBitFun/crates/execution/agent-runtime/src/compression_prefetch.rs — Context Compaction

pub fn default_compact_summary_prompt() -> String {
    "Generate a handoff summary for the following conversation so that an AI in a new session can continue without reading the original.\n\nOutput format (strictly follow):\n\n## Handoff Summary\n\n**Goal**: The user's core objective, 1-2 sentences\n\n**Progress**:\n- Key completed steps, one per line, in chronological order\n\n**Todo**:\n- Outstanding items, if any\n\n**Key Information**:\n- File paths, config values, variable names, error codes, and other concrete values needed going forward\n- Each item must state the concrete value; avoid vague references like \"some file\" or \"some value\"\n\n**Tool Call Summary**:\n- Executed tool names and key results, one per line\n- E.g. `read_file → config.rs content read`, `run_command → compilation passed`\n\nRules:\n1. Self-contained: do not reference \"above\" or \"earlier\"; state all information directly\n2. Concrete: write full file paths, full variable names, raw error codes\n3. Concise: omit step-by-step details; keep only key decisions and milestones\n4. Actionable: pasting into a new conversation lets the AI continue immediately without follow-up questions\n5. Tool calls: keep tool name + result summary; omit full output".to_string()
}

/// 窗口同时约束输入与输出，判定输入是否超限时必须先扣掉输出预留与安全裕度
pub const COMPACTION_SAFETY_TOKENS: u32 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactDecision {
    NotNeeded,
    Terminate,
    /// 窗口未知：不可判定，调用方必须记录日志而不是当成「不超限」静默跳过
    Undetermined,
}

/// 压缩判定：`P >= (W - max_tokens - safety) * ratio / 100`。
///
/// `ratio` 是「可用预算」的使用比例，不是「占完整窗口的比例」——分母已扣除输出预留，
/// 配置项因此命名为 `compact_threshold_percent_of_budget`（旧名 `compact_threshold_percent`
/// 在旧公式下指占完整窗口的百分比，同名不同义会导致用户按旧理解设置后实际阈值漂移）。
pub fn evaluate_compaction(
    prompt_tokens: u32,
    context_window_tokens: Option<u32>,
    auto_compact: bool,
    threshold_percent_of_budget: u8,
    reserved_output_tokens: u32,
) -> CompactDecision {
    if !auto_compact {
        return CompactDecision::NotNeeded;
    }
    let Some(context_window_tokens) = context_window_tokens.filter(|&v| v > 0) else {
        return CompactDecision::Undetermined;
    };

    let budget = (context_window_tokens as u64)
        .saturating_sub(reserved_output_tokens as u64 + COMPACTION_SAFETY_TOKENS as u64);
    if budget == 0 {
        return CompactDecision::Terminate;
    }
    if prompt_tokens == 0 {
        return CompactDecision::NotNeeded;
    }

    let threshold = budget * (threshold_percent_of_budget as u64) / 100;
    if (prompt_tokens as u64) >= threshold {
        CompactDecision::Terminate
    } else {
        CompactDecision::NotNeeded
    }
}
