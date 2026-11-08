import { tauriInvoke as invoke } from "./tauriInvoke";
import type {
  GrantScopeDto,
  PermissionGrantDto,
} from "../types/protocol.contract";

export type PermissionGrant = PermissionGrantDto;

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
};
