import { useEffect } from "react";

/**
 * orb 收起前的浮层清理广播（唯一来源）。
 *
 * antd 的 Select / Dropdown / Tooltip / Popover / Modal 都会把弹层挂到
 * `getPopupContainer`（默认 `document.body`），并不在 `.orb-panel` 子树内。
 * 收起只是把面板本身透明化并缩放，弹层不受影响，窗口缩成球大小后
 * 残留弹层会以原尺寸铺满整个 orb 窗口。因此「收起」必须同时关闭浮层状态。
 */
export const ORB_CLOSE_OVERLAYS_EVENT = "orb-close-overlays";

/** 广播关闭所有浮层：`collapseOrbPanel()` 在收起动画开始前调用。 */
export function closeOrbOverlays(): void {
  // 非浏览器环境（如 node 单测）里 `window` 可能只是空壳，广播是尽力而为。
  if (typeof window === "undefined") return;
  if (typeof window.dispatchEvent !== "function") return;
  window.dispatchEvent(new CustomEvent(ORB_CLOSE_OVERLAYS_EVENT));
}

/** 订阅关闭广播，返回取消订阅函数。 */
export function onOrbCloseOverlays(handler: () => void): () => void {
  if (typeof window === "undefined") return () => {};
  if (typeof window.addEventListener !== "function") return () => {};
  window.addEventListener(ORB_CLOSE_OVERLAYS_EVENT, handler);
  return () => window.removeEventListener(ORB_CLOSE_OVERLAYS_EVENT, handler);
}

/**
 * 组件侧订阅：凡是持有弹层 open 状态的组件都必须调用，
 * 否则收起后该弹层会残留在球上。
 */
export function useOrbOverlayClose(handler: () => void): void {
  useEffect(() => onOrbCloseOverlays(handler), [handler]);
}

/**
 * antd 浮层容器：orb 窗口内挂到 `[data-orb-window]` 根节点，
 * 其它窗口（主窗口等）退回 `document.body`。
 *
 * 供不读 `ConfigProvider.getPopupContainer` 的组件使用（如 `Modal` 的 `getContainer`），
 * 保证浮层一定落在 orb 的 DOM 子树里，能被收起兜底规则隐藏。
 */
export function resolveOverlayContainer(): HTMLElement {
  if (typeof document === "undefined") {
    throw new Error("resolveOverlayContainer requires a DOM");
  }
  return (
    document.querySelector<HTMLElement>("[data-orb-window]") ?? document.body
  );
}
