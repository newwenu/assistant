import type { WorkspaceEntry } from "../../services/desktop/workspaceService";

/** 会话工作区 chip 的三态（I38h §9.3）。 */
export type WorkspaceDisplayState = "unconfigured" | "valid" | "missing";

export interface WorkspaceDisplay {
  state: WorkspaceDisplayState;
  /** 展示名：目录簿名称，缺失时回退为路径末段。 */
  name: string;
  /** 绑定值原样（含可能的 `\\?\` 前缀），仅用于比较与持久化。 */
  path: string;
  /** 展示用路径：剥离 Windows verbatim 前缀后的可读形式。 */
  displayPath: string;
}

/**
 * 剥离 Windows verbatim 前缀（`\\?\` / `\\?\UNC\`）。
 *
 * 存储与解析始终使用 canonical 原样值（绑定比较必须逐字节一致），
 * 该前缀只在展示层去除，避免 UI 出现 `\\?\G:\...` 这类不可读路径。
 */
export function stripVerbatimPrefix(path: string): string {
  if (path.startsWith("\\\\?\\UNC\\")) return `\\\\${path.slice(8)}`;
  if (path.startsWith("\\\\?\\")) return path.slice(4);
  return path;
}

/** 路径末段，兼容 Windows 分隔符与尾部分隔符；先剥离 verbatim 前缀。 */
export function workspaceBasename(path: string): string {
  const trimmed = stripVerbatimPrefix(path).replace(/[\\/]+$/, "");
  const parts = trimmed.split(/[\\/]/);
  return parts[parts.length - 1] || trimmed;
}

/**
 * 由绑定值（空串 = 未配置）与目录簿条目推导展示态。
 * 目录簿只作展示：未命中条目视为有效，仅当条目明确 `missing` 才标记失效。
 */
export function workspaceDisplay(
  root: string,
  entries: WorkspaceEntry[],
): WorkspaceDisplay {
  if (root === "") {
    return { state: "unconfigured", name: "", path: "", displayPath: "" };
  }
  const entry = entries.find((e) => e.path === root);
  return {
    state: entry?.missing ? "missing" : "valid",
    name: entry?.name ?? workspaceBasename(root),
    path: root,
    displayPath: stripVerbatimPrefix(root),
  };
}
