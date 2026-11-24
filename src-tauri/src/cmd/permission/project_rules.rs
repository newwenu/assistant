use tauri::command;

use crate::adapters::{
    project_permission_status, save_project_permissions, ProjectPermissionStatus,
};
use crate::protocol::{ApprovalRuleDto, ProjectPermissionStatusDto};

fn status_dto(status: ProjectPermissionStatus) -> ProjectPermissionStatusDto {
    ProjectPermissionStatusDto {
        path: status.path,
        exists: status.exists,
        valid: status.valid,
        content: status.content,
        rules: status.rules.iter().map(Into::into).collect(),
    }
}

/// 项目规则状态（I70 §7.5）：报告 `permissions.json` 的路径、解析结果、原文与已解析规则。
///
/// 规则回传给设置页，用与模式级同一个 `ApprovalRulesEditor` 编辑后再整体写回（见
/// `project_rule_save_cmd`）；原文只在非法状态下有意义，用于定位写错的位置。
///
/// 未绑定工作区（`workspace_root` 为空）时返回 `None`，前端据此区分
/// 「未绑定工作区」与「文件缺失」两种不同状态。
///
/// `workspace_root` 只接受绝对路径：应用命令不受 capability 约束，前端可传任意字符串，
/// 相对路径会按进程 cwd 解析，等于允许读任意目录。
///
/// 只读路径**不要求目录存在**：目录被删除时仍要返回 `exists = false`，
/// 前端才能显示「文件缺失」而不是「未绑定工作区」（与目录簿的「失效」标签一致）。
/// 校验含 `canonicalize`（网络盘上可能长时间阻塞），因此离开主线程。
#[command]
pub async fn project_permission_status_cmd(
    workspace_root: Option<String>,
) -> Result<Option<ProjectPermissionStatusDto>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = workspace_root
            .as_deref()
            .map(str::trim)
            .filter(|raw| std::path::Path::new(raw).is_absolute());
        Ok(project_permission_status(root).map(status_dto))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 保存项目规则（I70 §7.5）：把编辑器里的规则整体写入 `.assistant/permissions.json`。
///
/// 文件不存在时新建（连同 `.assistant/` 目录），不再要求用户先自己建文件——
/// 「先打开外部编辑器」在文件缺失时没有落点，用户无从下手。
/// 规则先经与模式级同一份 [`crate::adapters::validate_approval_rules`] 校验，
/// 非法即 `Err` 且不写盘；成功后回传最新状态，前端无需再读一次。
///
/// 写盘前同样必须 `validate_workspace_root`：否则前端传 `C:/Windows/System32` 或
/// 相对路径即可在任意位置新建目录并写文件（`create_dir_all` + `fs::write`）。
#[command]
pub async fn project_rule_save_cmd(
    workspace_root: String,
    rules: Vec<ApprovalRuleDto>,
) -> Result<ProjectPermissionStatusDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = crate::tool_setup::validate_workspace_root(&workspace_root)
            .ok_or_else(|| assistant_i18n::t("error.workspace_invalid"))?;
        save_project_permissions(&root, &rules).map(status_dto)
    })
    .await
    .map_err(|e| e.to_string())?
}
