import { invoke } from "@tauri-apps/api/core";
import { createLogger } from "../system/logger";

const log = createLogger("tauriInvoke");

export interface TauriInvokeOptions {
  /** 预期会失败的空态查询：错误码交给调用方判定，不写成 error 日志。 */
  expectedError?: boolean;
}

export async function tauriInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
  options?: TauriInvokeOptions,
): Promise<T> {
  try {
    return await invoke(command, args);
  } catch (e) {
    const context = {
      error: e,
      command,
      argsKeys: args ? Object.keys(args) : [],
    };
    if (options?.expectedError) {
      log.debug(`${command} failed`, context);
    } else {
      log.error(`${command} failed`, context);
    }
    throw e;
  }
}
