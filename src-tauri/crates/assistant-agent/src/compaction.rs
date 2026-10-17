// 参考: OpenBitFun/crates/execution/agent-runtime/src/compression_prefetch.rs — Context Compaction

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

pub const DEFAULT_COMPACTION_THRESHOLD_PERCENT: u8 = 80;
pub const MAX_SUMMARY_REQUEST_TOKENS: u32 = 4096;

pub fn default_compact_summary_prompt() -> String {
    assistant_i18n::t("chat.compaction.default_summary_prompt")
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
