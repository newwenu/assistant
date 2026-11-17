import type {
  ScreenshotCropArgsDto,
  ScreenshotCropResultDto,
  ScreenshotSessionDto,
} from "@/types/protocol.contract";
import { tauriInvoke } from "@/services/system/tauriInvoke";
import { notifyError } from "@/stores/system/notifyStore";

/** 没有活动会话：拉取兜底与裁剪都靠它区分「正常空态」与真错误。 */
export const NO_SESSION = "screenshot.err.no_session";

/** macOS 未授权：错误描述换成「如何授权」的引导文案，而不是重复报错原因。 */
export const PERMISSION_DENIED = "screenshot.err.permission_denied";

const DESCRIPTION_KEYS: Record<string, string> = {
  [PERMISSION_DENIED]: "screenshot.permission_guide",
};

export const CHAT_FOCUS_INPUT_EVENT = "chat-focus-input";

let focusRequestPending = false;

export const screenshotService = {
  start: (): Promise<void> => tauriInvoke("start_screenshot"),

  getSession: (): Promise<ScreenshotSessionDto> =>
    tauriInvoke<ScreenshotSessionDto>("get_screenshot_session", undefined, {
      expectedError: true,
    }),

  crop: (args: ScreenshotCropArgsDto): Promise<ScreenshotCropResultDto> =>
    tauriInvoke<ScreenshotCropResultDto>("crop_screenshot", { args }),

  cancel: (token: string): Promise<void> =>
    tauriInvoke("cancel_screenshot", { token }),

  permissionGranted: (): Promise<boolean> =>
    tauriInvoke<boolean>("screenshot_permission_granted"),
};

/** 错误码本身就是 i18n 键；未知码由 `translateKey` 原样回显，不新建通知组件。 */
export function notifyScreenshotError(error: unknown): void {
  const code = typeof error === "string" && error ? error : "";
  if (code === NO_SESSION) return;
  const key = code || "screenshot.err.capture_failed";
  notifyError(key, DESCRIPTION_KEYS[key] ?? key);
}

export function requestChatInputFocus(): void {
  focusRequestPending = true;
  window.dispatchEvent(new Event(CHAT_FOCUS_INPUT_EVENT));
}

/**
 * 订阅方挂载时消费一次：展开是异步的（先请求 Rust 改窗口尺寸再切 store），
 * 覆盖层发请求时输入框可能还没挂载，只靠事件会丢一次聚焦。
 */
export function consumeChatInputFocusRequest(): boolean {
  const pending = focusRequestPending;
  focusRequestPending = false;
  return pending;
}
