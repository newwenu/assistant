import { createLogger } from "@/services/system/logger";

const log = createLogger("moduleLoader");

const MODULE_LOAD_ERROR_PATTERNS = [
  "Failed to fetch dynamically imported module",
  "importing a module script failed",
  "Unable to preload CSS",
  "error in async boundary",
  "ChunkLoadError",
  "Loading chunk",
  "Unexpected token",
];

export function isModuleLoadError(error: unknown): boolean {
  if (!(error instanceof Error)) return false;
  const msg = error.message;
  return MODULE_LOAD_ERROR_PATTERNS.some((p) => msg.includes(p));
}

const RETRY_DELAYS = [250, 750];

export async function importWithRetry<T>(
  loader: () => Promise<T>,
  maxRetries: number = 2,
): Promise<T> {
  try {
    return await loader();
  } catch (error) {
    if (!isModuleLoadError(error)) {
      throw error;
    }

    for (let attempt = 0; attempt < maxRetries; attempt++) {
      const delay =
        RETRY_DELAYS[attempt] ?? RETRY_DELAYS[RETRY_DELAYS.length - 1];
      await new Promise((resolve) => setTimeout(resolve, delay));

      log.info(`Retrying module import (attempt ${attempt + 1}/${maxRetries})`);
      try {
        return await loader();
      } catch (retryError) {
        if (!isModuleLoadError(retryError)) {
          throw retryError;
        }
      }
    }

    throw error;
  }
}

interface ModuleLoaderState<T> {
  pending: Promise<T> | undefined;
  result: T | undefined;
  error: Error | undefined;
}

export function createModuleLoader<T>(loader: () => Promise<T>): {
  load: () => Promise<T>;
  reset: () => void;
} {
  const state: ModuleLoaderState<T> = {
    pending: undefined,
    result: undefined,
    error: undefined,
  };

  const load = (): Promise<T> => {
    if (state.result !== undefined) {
      return Promise.resolve(state.result);
    }

    if (state.pending) {
      return state.pending;
    }

    state.pending = importWithRetry(loader)
      .then((module) => {
        state.result = module;
        state.error = undefined;
        state.pending = undefined;
        return module;
      })
      .catch((error: unknown) => {
        const err = error instanceof Error ? error : new Error(String(error));
        state.error = err;
        state.pending = undefined;
        throw err;
      });

    return state.pending;
  };

  const reset = (): void => {
    state.pending = undefined;
    state.result = undefined;
    state.error = undefined;
  };

  return { load, reset };
}

type RecoveryCallback = () => void;

const recoverySubscribers = new Set<RecoveryCallback>();
const failedModules = new Set<RecoveryCallback>();

export function subscribeToModuleRecovery(
  callback: RecoveryCallback,
): () => void {
  recoverySubscribers.add(callback);
  return () => {
    recoverySubscribers.delete(callback);
    failedModules.delete(callback);
  };
}

export function markModuleFailed(callback: RecoveryCallback): void {
  failedModules.add(callback);
}

export function retryFailedModuleLoads(): void {
  if (failedModules.size === 0) return;
  log.info("Triggering module recovery for failed subscribers", {
    count: failedModules.size,
  });
  for (const callback of failedModules) {
    try {
      callback();
    } catch (error) {
      log.error("Recovery callback error", {
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }
  failedModules.clear();
}

if (import.meta.hot) {
  import.meta.hot.on("vite:afterUpdate", () => {
    retryFailedModuleLoads();
  });
  import.meta.hot.on("vite:ws:connect", () => {
    retryFailedModuleLoads();
  });
}
