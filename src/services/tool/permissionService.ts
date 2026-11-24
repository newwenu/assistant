import { tauriInvoke as invoke } from "../system/tauriInvoke";
import type {
  ApprovalRuleDto,
  GrantScopeDto,
  PermissionAuditEntryDto,
  PermissionAuditPageDto,
  PermissionGrantDto,
  ProjectPermissionStatusDto,
} from "../../types/protocol.contract";

export type PermissionGrant = PermissionGrantDto;
export type PermissionAuditEntry = PermissionAuditEntryDto;
export type PermissionAuditPage = PermissionAuditPageDto;
export type ProjectPermissionStatus = ProjectPermissionStatusDto;

/**
 * 「已授权操作」查询、撤销与作用域修改（I38d §3.3）。
 *
 * 主键 `grantKey` 对前端不透明：列表展示只用后端归一好的 `scopeSummary`，
 * 撤销时原样回传主键，避免前端重算键导致删错行。
 */
export const permissionService = {
  list: (): Promise<PermissionGrant[]> =>
    invoke<PermissionGrant[]>("list_permission_grants").then((r) => r ?? []),

  revoke: (grantKey: string): Promise<boolean> =>
    invoke<boolean>("revoke_permission_grant", { grantKey }),

  clear: (): Promise<void> => invoke("clear_permission_grants"),

  /**
   * 项目规则状态（I70 §7.5）：路径、解析结果、原文与已解析规则。
   *
   * `workspaceRoot` 为空表示未绑定工作区，后端返回 `null`；
   * 前端据此把「未绑定工作区」与「文件缺失」分成两种展示。
   */
  projectStatus: (
    workspaceRoot: string | null,
  ): Promise<ProjectPermissionStatus | null> =>
    invoke<ProjectPermissionStatus | null>("project_permission_status_cmd", {
      workspaceRoot: workspaceRoot || null,
    }).then((r) => r ?? null),

  /**
   * 保存项目规则：整体写入 `.assistant/permissions.json`，文件不存在时新建。
   *
   * 规则与模式级用户规则同一份校验；非法时后端返回 `Err` 且不写盘。
   * 返回写入后的最新状态（含重新解析出的规则），调用方据此对齐草稿。
   */
  saveProjectRules: (
    workspaceRoot: string,
    rules: ApprovalRuleDto[],
  ): Promise<ProjectPermissionStatus> =>
    invoke<ProjectPermissionStatus>("project_rule_save_cmd", {
      workspaceRoot,
      rules,
    }),

  /**
   * 修改授权作用域（会话级 ⇄ 全局）。
   *
   * `scope = "session"` 时必须给出 `conversationId`，由后端校验；
   * 返回 `false` 表示源行已不存在，调用方应重新拉取列表对齐。
   */
  setScope: (
    grantKey: string,
    scope: GrantScopeDto,
    conversationId?: string | null,
  ): Promise<boolean> =>
    invoke<boolean>("set_permission_grant_scope", {
      args: { grantKey, scope, conversationId: conversationId ?? null },
    }),

  /**
   * 历史决策追溯（I50 §3.7）。
   *
   * `limit` 是**每页条数**，由后端夹取（默认 100、上限 500，非正值回落默认）；
   * `since` / `until` 是毫秒闭区间，翻页时用 `until` 冻结窗口，新写入不会挤偏分页；
   * `offset` 是页内起点（非正值回落 0）。
   *
   * 返回 `hasMore` 由后端多取一行判定——「本页条数 == limit」无法区分
   * 「刚好取完」与「还有更早记录」。面板不缓存、不做增量合并，每次整段替换（EF1）。
   */
  listAudit: (
    limit?: number | null,
    since?: number | null,
    until?: number | null,
    offset?: number | null,
  ): Promise<PermissionAuditPage> =>
    invoke<PermissionAuditPage>("list_permission_audit", {
      limit: limit ?? null,
      since: since ?? null,
      until: until ?? null,
      offset: offset ?? null,
    }).then((r) => r ?? { entries: [], hasMore: false }),
};
