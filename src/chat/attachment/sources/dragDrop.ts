import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { DragDropEvent } from "@tauri-apps/api/webview";
import type { Event } from "@tauri-apps/api/event";

export function subscribeDragDrop(
  onPaths: (paths: string[]) => void,
): () => void {
  const webview = getCurrentWebview();
  let disposed = false;
  let unlisten: (() => void) | undefined;

  void webview
    .onDragDropEvent((event: Event<DragDropEvent>) => {
      if (disposed) return;
      if (event.payload.type === "drop") onPaths(event.payload.paths);
    })
    .then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });

  return () => {
    disposed = true;
    unlisten?.();
    unlisten = undefined;
  };
}
