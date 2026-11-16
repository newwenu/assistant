import { createLogger } from "@/services/system/logger";

const log = createLogger("sideEffect");

export type SideEffectHandler = (
  prev: Record<string, unknown>,
  next: Record<string, unknown>,
) => void | Promise<void>;

const sideEffectHandlers = new Map<string, SideEffectHandler>();

export function registerSideEffect(
  id: string,
  handler: SideEffectHandler,
): void {
  sideEffectHandlers.set(id, handler);
}

export function runSideEffects(
  id: string,
  prev: Record<string, unknown>,
  next: Record<string, unknown>,
): void {
  const handler = sideEffectHandlers.get(id);
  if (handler) {
    const result = handler(prev, next);
    if (result instanceof Promise) {
      result.catch((err: unknown) => log.warn(`sideEffect:${id} failed`, err));
    }
  }
}

export function getRegisteredSideEffects(): string[] {
  return [...sideEffectHandlers.keys()];
}
