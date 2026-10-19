import { invoke } from "@tauri-apps/api/core";
import { createLogger } from "./logger";

const log = createLogger("tauriInvoke");

export async function tauriInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke(command, args);
  } catch (e) {
    log.error(`${command} failed`, {
      error: e,
      command,
      argsKeys: args ? Object.keys(args) : [],
    });
    throw e;
  }
}
