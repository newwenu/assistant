/**
 * 工作区切换的调用序列（I38h §9.1 / §9.4）。
 *
 * 顺序固定：先 `register_workspace` 登记目录簿，成功后才 `patch_conversation` 写绑定。
 * 任何一步失败都**不做乐观更新**，由调用方保留原值并提示后端错误。
 */
export interface WorkspaceSwitchDeps {
  /** 登记目录簿；失败即中止，不写会话绑定。 */
  register: (path: string) => Promise<unknown>;
  /** 写会话绑定；返回 `false` 表示失败。 */
  patch: (path: string) => Promise<boolean>;
}

export type WorkspaceSwitchResult =
  | { kind: "switched" }
  | { kind: "register_failed"; error: unknown }
  | { kind: "patch_failed"; error: unknown };

export async function switchWorkspace(
  deps: WorkspaceSwitchDeps,
  path: string,
): Promise<WorkspaceSwitchResult> {
  try {
    await deps.register(path);
  } catch (error) {
    return { kind: "register_failed", error };
  }
  try {
    return (await deps.patch(path))
      ? { kind: "switched" }
      : { kind: "patch_failed", error: undefined };
  } catch (error) {
    return { kind: "patch_failed", error };
  }
}
