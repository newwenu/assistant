import { tauriInvoke as invoke } from "../services/tauriInvoke";

export interface OsResult<T> {
  ok: boolean;
  data?: T;
  error?: string;
}

interface FsLinkCreateResult {
  source: string;
  target: string;
  kind: string;
  created: boolean;
}

interface FsLinkCheckResult {
  source: string;
  isLink: boolean;
  kind: string | null;
  target: string | null;
}

async function callOsTool<T>(
  tool: string,
  params: Record<string, unknown>,
): Promise<OsResult<T>> {
  return invoke<OsResult<T>>("os_tool_exec", {
    request: { tool, action: params.action, params },
  });
}

export const os = {
  fsLink: {
    create(
      source: string,
      target: string,
      kind: "symlink" | "hardlink" | "junction" = "symlink",
    ): Promise<OsResult<FsLinkCreateResult>> {
      return callOsTool<FsLinkCreateResult>("fs_link", {
        action: "create",
        source,
        target,
        kind,
      });
    },

    check(source: string): Promise<OsResult<FsLinkCheckResult>> {
      return callOsTool<FsLinkCheckResult>("fs_link", {
        action: "check",
        source,
      });
    },
  },
};
