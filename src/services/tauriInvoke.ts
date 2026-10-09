import { invoke } from "@tauri-apps/api/core";

export async function tauriInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return (await invoke(command, args)) as T;
  } catch (e) {
    console.error(`[tauriInvoke] ${command} failed:`, e);
    throw e;
  }
}
