// 参考: OpenBitFun/crates/execution/agent-runtime/src/compression_prefetch.rs — Context Compaction

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

pub const DEFAULT_COMPACTION_THRESHOLD_PERCENT: u8 = 80;
pub const MAX_SUMMARY_REQUEST_TOKENS: u32 = 4096;

pub fn default_compact_summary_prompt() -> String {
    "Generate a handoff summary for the following conversation so that an AI in a new session can continue without reading the original.\n\nOutput format (strictly follow):\n\n## Handoff Summary\n\n**Goal**: The user's core objective, 1-2 sentences\n\n**Progress**:\n- Key completed steps, one per line, in chronological order\n\n**Todo**:\n- Outstanding items, if any\n\n**Key Information**:\n- File paths, config values, variable names, error codes, and other concrete values needed going forward\n- Each item must state the concrete value; avoid vague references like \"some file\" or \"some value\"\n\n**Tool Call Summary**:\n- Executed tool names and key results, one per line\n- E.g. `read_file → config.rs content read`, `run_command → compilation passed`\n\nRules:\n1. Self-contained: do not reference \"above\" or \"earlier\"; state all information directly\n2. Concrete: write full file paths, full variable names, raw error codes\n3. Concise: omit step-by-step details; keep only key decisions and milestones\n4. Actionable: pasting into a new conversation lets the AI continue immediately without follow-up questions\n5. Tool calls: keep tool name + result summary; omit full output".to_string()
}

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