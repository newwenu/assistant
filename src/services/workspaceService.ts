import { tauriInvoke as invoke } from "./tauriInvoke";
import type { WorkspaceDto } from "../types/protocol.contract";

export type WorkspaceEntry = WorkspaceDto;

/**
 * 工作区目录簿与目录选择（I38h §8 / §9）。
 *
 * `chooseDir` 只弹系统对话框并返回路径，**不写库**；登记由 `register` 完成。
 * `forget` 只删目录簿行，不影响任何会话的绑定值。
 */
export const workspaceService = {
  list: (): Promise<WorkspaceEntry[]> =>
    invoke<WorkspaceEntry[]>("list_workspaces").then((r) => r ?? []),

  chooseDir: (): Promise<string | null> =>
    invoke<string | null>("choose_workspace_dir").then((r) => r ?? null),

  register: (path: string, name?: string | null): Promise<WorkspaceEntry> =>
    invoke<WorkspaceEntry>("register_workspace", { path, name: name ?? null }),

  forget: (path: string): Promise<void> => invoke("forget_workspace", { path }),
};
