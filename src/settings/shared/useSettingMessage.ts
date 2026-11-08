import { App } from "antd";
import { translateKey } from "@/i18n";

const DEFAULT_SAVE_KEY = "common.actions.saved";
const DEFAULT_FAILED_KEY = "common.actions.save_failed_desc";

function errorDetail(raw: unknown): string {
  return raw instanceof Error ? raw.message : String(raw);
}

export interface SettingMessageAPI {
  success: (key: string, options?: Record<string, unknown>) => void;
  error: (key: string, raw: unknown) => void;
  warning: (key: string, options?: Record<string, unknown>) => void;
  info: (key: string, options?: Record<string, unknown>) => void;
  loading: (key: string, duration?: number) => () => void;
  showSaved: (key?: string | false) => void;
  showFailed: (raw: unknown, key?: string) => void;
  message: ReturnType<typeof App.useApp>["message"];
}

export function useSettingMessage(): SettingMessageAPI {
  const { message } = App.useApp();

  const success = (key: string, options?: Record<string, unknown>) => {
    message.success(translateKey(key, options));
  };

  const error = (key: string, raw: unknown) => {
    message.error(translateKey(key, { error: errorDetail(raw) }));
  };

  const warning = (key: string, options?: Record<string, unknown>) => {
    message.warning(translateKey(key, options));
  };

  const info = (key: string, options?: Record<string, unknown>) => {
    message.info(translateKey(key, options));
  };

  const loading = (key: string, duration?: number) => {
    return message.loading(translateKey(key), duration);
  };

  const showSaved = (key?: string | false) => {
    if (key === false) return;
    message.success(translateKey(key || DEFAULT_SAVE_KEY));
  };

  // 保存失败的默认提示。刻意不看 `SettingDefinition.saveMessage === false`：
  // 那个开关的语义是「成功本身有视觉反馈、不必弹条」，不代表用户不关心失败。
  const showFailed = (raw: unknown, key?: string) => {
    message.error(
      translateKey(key || DEFAULT_FAILED_KEY, { error: errorDetail(raw) }),
    );
  };

  return {
    success,
    error,
    warning,
    info,
    loading,
    showSaved,
    showFailed,
    message,
  };
}
