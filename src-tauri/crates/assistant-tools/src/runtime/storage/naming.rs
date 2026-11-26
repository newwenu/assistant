use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// 会话命名空间：会话 id 的 FNV-1a 64 位摘要（不引第三方哈希库，跨进程稳定）。
///
/// 不用 `DefaultHasher`：它的算法与密钥未承诺跨版本稳定，而定位符要跨重启有效。
/// 空 id / `None` 归入 `shared`，与 Agent 会话互不干扰。
pub fn session_namespace(session_id: Option<&str>) -> String {
    let Some(id) = session_id.filter(|id| !id.is_empty()) else {
        return "shared".to_string();
    };
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:016x}", hash)
}

/// 会话级 scratch 域根：`base.join(session_namespace(session_id))`（I73 §5.4 P4.0 / N1）。
///
/// 会话段拼法的**唯一实现点**：应用层只允许两处调用——注入
/// （`adapters/tool_exec.rs`）与会话结束清理（`cmd/conversation/mutate.rs`）。
/// 纯函数、无 IO，故库 crate 不解析基根也能复用同一实现。
pub fn scratch_root_for(base: &Path, session_id: Option<&str>) -> PathBuf {
    namespace_dir(base, &session_namespace(session_id))
}

/// 命名空间目录：`base.join(namespace)`。
///
/// 基根与段的 `join` **只允许**发生在本模块（I77 INV3 / V2）：唯一调用方是同文件的
/// [`scratch_root_for`]，因此保持私有，不再对外暴露第二种拼法。
fn namespace_dir(base: &Path, namespace: &str) -> PathBuf {
    base.join(namespace)
}

/// 文件名的唯一前缀：纳秒时间戳 + 进程内自增序号。
///
/// 只保证「同一目录内不撞名」，不承担不可猜测性：定位符本身不是凭据，
/// 越权访问由工作区边界与审批约束。
pub(super) fn unique_prefix() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or(0);
    format!("{:012x}{:04x}", nanos & 0xffff_ffff_ffff, seq & 0xffff)
}

/// 工具名只作可读提示，非法字符一律替换，避免路径分隔符与保留字符。
pub(super) fn safe_component(raw: &str) -> String {
    let sanitized: String = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let sanitized = sanitized.trim_matches('_');
    if sanitized.is_empty() {
        "tool".to_string()
    } else {
        sanitized.chars().take(32).collect()
    }
}

pub(super) fn result_file_name(tool_call_id: &str) -> String {
    let sanitized: String = tool_call_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() || sanitized.chars().all(|ch| ch == '_') {
        "result.txt".to_string()
    } else {
        format!("{}.txt", sanitized)
    }
}
