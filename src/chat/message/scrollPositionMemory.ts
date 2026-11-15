import type { SavedPosition } from "./scrollIntent";

/**
 * 会话级滚动位置记忆（I48 §7）：模块级 `Map`，不写 store、不持久化。
 *
 * 键与打开过的会话数同级，值只有 `{ top, pinned }`，不构成实际泄漏，
 * 因此不做清理——清理需要跨域删除联动，会让 UI 模块反向依赖 store。
 */
const positions = new Map<string, SavedPosition>();

export function readPosition(
  conversationId: string | null,
): SavedPosition | null {
  if (!conversationId) return null;
  return positions.get(conversationId) ?? null;
}

/**
 * 写入位置。调用方只应在两种时机调用：
 * 滚动节流回调内（覆盖同一会话内的移动）、组件卸载时（元素仍在、内容未换）。
 *
 * **禁止**在 `conversationId` 变化的 cleanup 中调用：此时新内容已渲染，
 * 浏览器可能已按新内容 clamp 过 `scrollTop`，会把旧会话写成 0。
 */
export function savePosition(
  conversationId: string | null,
  position: SavedPosition,
): void {
  if (!conversationId) return;
  positions.set(conversationId, position);
}
