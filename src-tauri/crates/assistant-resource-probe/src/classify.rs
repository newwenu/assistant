//! 角色判定与平台口径常量（I76 §2.3、§2.4）。
//!
//! 三层：归属（已在树内，由 tree 保证）→ 家族（镜像名粗筛）→ 细分（命令行 `--type=`）。

use assistant_protocol::{MemoryBasisDto, ProcessRoleDto};

use crate::source::RawProcess;

/// 平台口径常量（I76 §2.4）。`memory_additive` 与 `memory_equals_footprint`
/// 相互独立，禁止合并成一个布尔。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformProfile {
    pub memory_basis: MemoryBasisDto,
    pub memory_additive: bool,
    pub memory_equals_footprint: bool,
    pub webview_detached: bool,
    /// 镜像名家族（小写比较），只用于粗筛，不用于归属判定。
    pub webview_families: &'static [&'static str],
}

/// 第 ② ③ 层判定：家族 → 细分。第 ① 层「归属」由 tree 保证，不在此重复判断。
pub fn role(process: &RawProcess, root_pid: u32, families: &[&str]) -> ProcessRoleDto {
    if process.pid == root_pid {
        return ProcessRoleDto::Host;
    }

    let exe_name = process.exe_name.to_ascii_lowercase();
    let is_family = families
        .iter()
        .any(|family| exe_name == family.to_ascii_lowercase());
    if !is_family {
        return ProcessRoleDto::OtherChild;
    }

    // 命令行读不到 → 细分不可信，落 webviewUnknown（必须计入自身合计，I76 §2.3）。
    if process.cmd.is_empty() {
        return ProcessRoleDto::WebviewUnknown;
    }

    match process
        .cmd
        .iter()
        .find_map(|arg| arg.strip_prefix("--type="))
    {
        Some("renderer") => ProcessRoleDto::WebviewRenderer,
        Some("gpu-process") => ProcessRoleDto::WebviewGpu,
        // 未知细分值走 utility，不新增枚举值（前端兜底不窄化）。
        Some(_) => ProcessRoleDto::WebviewUtility,
        // 有命令行但没有 `--type=`：浏览器主进程。
        None => ProcessRoleDto::WebviewBrowser,
    }
}

/// 该角色是否归入自身合计（I76 §2.3）。
pub fn is_self_role(role: ProcessRoleDto) -> bool {
    matches!(
        role,
        ProcessRoleDto::Host
            | ProcessRoleDto::WebviewBrowser
            | ProcessRoleDto::WebviewRenderer
            | ProcessRoleDto::WebviewGpu
            | ProcessRoleDto::WebviewUtility
            | ProcessRoleDto::WebviewUnknown
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAMILIES: &[&str] = &["msedgewebview2.exe", "msedge.exe", "chrome.exe"];

    fn webview(cmd: &[&str]) -> RawProcess {
        RawProcess::new(200, 100, 60, "msedgewebview2.exe").with_cmd(cmd)
    }

    #[test]
    fn root_is_host() {
        let root = RawProcess::new(100, 1, 50, "assistant-app.exe");
        assert_eq!(role(&root, 100, FAMILIES), ProcessRoleDto::Host);
    }

    /// 宿主自身即使镜像名撞上家族也必须是 host。
    #[test]
    fn root_wins_over_family() {
        let root = RawProcess::new(100, 1, 50, "chrome.exe");
        assert_eq!(role(&root, 100, FAMILIES), ProcessRoleDto::Host);
    }

    #[test]
    fn family_split_by_type_flag() {
        assert_eq!(
            role(&webview(&["x.exe", "--type=renderer"]), 100, FAMILIES),
            ProcessRoleDto::WebviewRenderer
        );
        assert_eq!(
            role(&webview(&["x.exe", "--type=gpu-process"]), 100, FAMILIES),
            ProcessRoleDto::WebviewGpu
        );
        assert_eq!(
            role(
                &webview(&["x.exe", "--type=utility", "--lang=zh"]),
                100,
                FAMILIES
            ),
            ProcessRoleDto::WebviewUtility
        );
        assert_eq!(
            role(&webview(&["x.exe", "--type=something-new"]), 100, FAMILIES),
            ProcessRoleDto::WebviewUtility
        );
        assert_eq!(
            role(&webview(&["x.exe"]), 100, FAMILIES),
            ProcessRoleDto::WebviewBrowser
        );
    }

    /// 命令行不可读 → webviewUnknown，而不是误判成浏览器主进程。
    #[test]
    fn unreadable_cmd_is_webview_unknown() {
        assert_eq!(
            role(&webview(&[]), 100, FAMILIES),
            ProcessRoleDto::WebviewUnknown
        );
    }

    #[test]
    fn non_family_is_other_child() {
        let other =
            RawProcess::new(200, 100, 60, "node.exe").with_cmd(&["node.exe", "--type=renderer"]);
        assert_eq!(role(&other, 100, FAMILIES), ProcessRoleDto::OtherChild);
    }

    #[test]
    fn self_role_set_includes_webview_unknown() {
        assert!(is_self_role(ProcessRoleDto::WebviewUnknown));
        assert!(is_self_role(ProcessRoleDto::Host));
        assert!(!is_self_role(ProcessRoleDto::OtherChild));
    }
}
