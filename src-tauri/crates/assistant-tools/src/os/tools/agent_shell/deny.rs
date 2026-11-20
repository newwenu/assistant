//! 沙箱拒绝判定：纯函数（I56 §8.5）。
//!
//! 只在 `enforcement != None` 时启用。命中后工具**仍然返回 `ok: true`**，
//! 只是额外标注 `data.sandbox.denied`，由模型据此调整策略——判定结果不改变
//! 退出码语义，也不把「沙箱拦了」伪装成工具失败。

use crate::sandbox::SandboxEnforcement;

/// 关键词表（大小写不敏感）：命中任一即视为「可能是沙箱拦的」。
///
/// 表只服务**归因**，不服务拦截：漏判的后果是模型多试一次，误判的后果是
/// 模型误以为环境不许可。因此表宁可短而准。
const DENY_KEYWORDS: &[&str] = &[
    "operation not permitted",
    "permission denied",
    "read-only file system",
    "sandbox",
    "landlock",
    "access is denied",
    "failed to write file",
];

/// `sh` 语义的常规退出码：命令用法错误 / 不可执行 / 命令不存在。
///
/// 这三个码在任何环境下都会出现，命中它们时即使是沙箱环境也不能归因为拒绝。
const EXCLUDED_EXITS: &[i32] = &[2, 126, 127];

/// Linux `SIGSYS`（seccomp 拒绝系统调用的默认处置）。
#[cfg(target_os = "linux")]
const SIGSYS: i32 = 31;

/// macOS `SIGSYS`。
#[cfg(target_os = "macos")]
const SIGSYS: i32 = 12;

/// 其余平台（Windows 无信号语义）取 Linux 值，仅用于保持常量可编译。
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
const SIGSYS: i32 = 31;

/// 是否判定为「沙箱拒绝」：三个条件同时满足（I56 §8.5）。
///
/// 1. `enforcement != None`（弱边界环境下不存在「沙箱拦的」这回事）；
/// 2. 退出码非零——`exit` 为 `None` 表示 Unix 侧被信号杀死，也算非零；
///    排除 `2` / `126` / `127`；
/// 3. 退出码等于 `128 + SIGSYS`，或输出尾部命中关键词。
///
/// `stderr_and_stdout_tail` 由调用方取已落盘输出的尾部窗口，不得全量扫描：
/// 判定只需要「最后发生了什么」，全量扫描会让判定成本随输出线性增长。
pub fn is_sandbox_denied(
    enforcement: SandboxEnforcement,
    exit: Option<i32>,
    stderr_and_stdout_tail: &str,
) -> bool {
    if enforcement == SandboxEnforcement::None {
        return false;
    }
    if !is_nonzero_exit(exit) {
        return false;
    }
    if exit == Some(128 + SIGSYS) {
        return true;
    }
    let lowered = stderr_and_stdout_tail.to_ascii_lowercase();
    DENY_KEYWORDS
        .iter()
        .any(|keyword| lowered.contains(*keyword))
}

/// 非零退出：`None`（信号杀死）算非零，`Some(0)` 与排除码都不算。
fn is_nonzero_exit(exit: Option<i32>) -> bool {
    match exit {
        Some(0) => false,
        Some(code) => !EXCLUDED_EXITS.contains(&code),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_enforcement_never_denies() {
        for exit in [Some(0), Some(1), Some(159), None] {
            assert!(!is_sandbox_denied(
                SandboxEnforcement::None,
                exit,
                "permission denied"
            ));
        }
    }

    #[test]
    fn zero_exit_never_denies() {
        assert!(!is_sandbox_denied(
            SandboxEnforcement::Full,
            Some(0),
            "permission denied"
        ));
    }

    #[test]
    fn excluded_exit_codes_never_deny() {
        for code in [2, 126, 127] {
            assert!(!is_sandbox_denied(
                SandboxEnforcement::Full,
                Some(code),
                "permission denied"
            ));
        }
    }

    #[test]
    fn sigsys_exit_denies_without_keyword() {
        assert!(is_sandbox_denied(
            SandboxEnforcement::Full,
            Some(128 + SIGSYS),
            "boom"
        ));
    }

    #[test]
    fn keyword_match_is_case_insensitive() {
        for text in [
            "Operation not permitted",
            "PERMISSION DENIED",
            "Read-only file system",
            "sandbox: write blocked",
            "Landlock: denied",
            "Access is denied.",
            "Failed to write file",
        ] {
            assert!(
                is_sandbox_denied(SandboxEnforcement::Partial, Some(1), text),
                "{text}"
            );
        }
    }

    #[test]
    fn ordinary_failure_is_not_denied() {
        assert!(!is_sandbox_denied(
            SandboxEnforcement::Full,
            Some(1),
            "error: could not compile"
        ));
    }

    #[test]
    fn signal_kill_counts_as_nonzero() {
        assert!(is_sandbox_denied(
            SandboxEnforcement::Partial,
            None,
            "operation not permitted"
        ));
    }
}
