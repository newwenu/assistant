import { createLogger } from "@/services/logger";

const log = createLogger("settings-registry");

export function insertUnique<T>(
  list: T[],
  keys: Set<string>,
  key: string,
  item: T,
  kind: string,
): boolean {
  if (keys.has(key)) {
    const message = `[settings] duplicate ${kind}: ${key}`;
    if (import.meta.env.DEV) throw new Error(message);
    log.error(message);
    return false;
  }
  keys.add(key);
  list.push(item);
  return true;
}
