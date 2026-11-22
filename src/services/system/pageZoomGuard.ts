const ZOOM_KEYS = new Set(["+", "=", "-", "0"]);

let registered = false;

function hasZoomModifier(event: WheelEvent | KeyboardEvent): boolean {
  return event.ctrlKey || event.metaKey;
}

/**
 * 主窗口开启了 `zoomHotkeysEnabled`（否则 WebView2 会吞掉触控板捏合手势），
 * 该开关同时打开了整页缩放：`ctrl+wheel` 与 `ctrl+±/0`。画布自己处理过的
 * 事件已 `preventDefault`，这里只兜住画布之外的漏网事件，保证捏合只缩放画布。
 */
function blockWheelZoom(event: WheelEvent): void {
  if (event.defaultPrevented) return;
  if (!hasZoomModifier(event)) return;
  event.preventDefault();
}

function blockKeyZoom(event: KeyboardEvent): void {
  if (event.defaultPrevented) return;
  if (!hasZoomModifier(event)) return;
  if (!ZOOM_KEYS.has(event.key)) return;
  event.preventDefault();
}

export function registerPageZoomGuard(): void {
  if (registered) return;
  registered = true;
  window.addEventListener("wheel", blockWheelZoom, { passive: false });
  window.addEventListener("keydown", blockKeyZoom);
}
