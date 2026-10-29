import { App } from "antd";
import { translateKey } from "@/i18n";

const DEFAULT_SAVE_KEY = "common.actions.saved";

export interface SettingMessageAPI {
  success: (key: string, options?: Record<string, unknown>) => void;
  error: (key: string, raw: unknown) => void;
  warning: (key: string, options?: Record<string, unknown>) => void;
  info: (key: string, options?: Record<string, unknown>) => void;
  loading: (key: string, duration?: number) => () => void;
  showSaved: (key?: string | false) => void;
  message: ReturnType<typeof App.useApp>["message"];
}

export function useSettingMessage(): SettingMessageAPI {
  const { message } = App.useApp();

  const success = (key: string, options?: Record<string, unknown>) => {
    message.success(translateKey(key, options));
  };

  const error = (key: string, raw: unknown) => {
    const detail = raw instanceof Error ? raw.message : String(raw);
    message.error(translateKey(key, { error: detail }));
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

  return { success, error, warning, info, loading, showSaved, message };
}
