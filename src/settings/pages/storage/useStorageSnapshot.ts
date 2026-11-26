import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { storageService } from "@/services/desktop/storageService";
import { createLogger } from "@/services/system/logger";
import type {
  AttachmentDeleteResultDto,
  StorageBreakdownDto,
  StorageInventorySnapshotStatusDto,
} from "@/types/protocol.contract";

const log = createLogger("storageSnapshot");

export const STORAGE_INVENTORY_UPDATED = "storage:inventory-updated";

/** 后台补算完成 / 失败（I79 §6.2）：载荷为空，前端只据此整页重拉。 */
export const STORAGE_ENTRY_STATS_UPDATED = "storage:entry-stats-updated";

/** 影响快照的事件：任一到达都整页重拉，尾随合并（I79 §9.1）。 */
export const STORAGE_SNAPSHOT_EVENTS = [
  STORAGE_INVENTORY_UPDATED,
  STORAGE_ENTRY_STATS_UPDATED,
] as const;

export interface SnapshotEventSource {
  listen: (event: string, handler: () => void) => Promise<UnlistenFn>;
}

/**
 * 订阅全部快照事件（I79 §6.2 / §9.1）：载荷恒 `()`，不按 key 打补丁，只触发整页重拉。
 *
 * 返回退订函数；订阅是异步完成的，若退订先于订阅就绪，则就绪后立即退订，避免泄漏。
 */
export function subscribeSnapshotEvents(
  source: SnapshotEventSource,
  onEvent: () => void,
): () => void {
  const unlisteners: UnlistenFn[] = [];
  let disposed = false;
  for (const eventName of STORAGE_SNAPSHOT_EVENTS) {
    void source
      .listen(eventName, onEvent)
      .then((fn) => {
        if (disposed) fn();
        else unlisteners.push(fn);
      })
      .catch((raw: unknown) => {
        log.error(`failed to subscribe ${eventName}`, raw);
      });
  }
  return () => {
    disposed = true;
    for (const unlisten of unlisteners) unlisten();
  };
}

export type SnapshotPhase = "initial" | "ready" | "refreshing";

export type LoadMode = "initial" | "refresh";

export interface StorageSnapshotHandle {
  snapshot: StorageBreakdownDto | null;
  phase: SnapshotPhase;
  error: string | null;
  refresh: () => void;
  retry: () => void;
  deleteAttachments: (ids: string[]) => Promise<AttachmentDeleteResultDto>;
  scanInventory: () => Promise<StorageInventorySnapshotStatusDto>;
}

export interface SnapshotLoaderCallbacks<T> {
  onPhase: (phase: SnapshotPhase) => void;
  onData: (data: T) => void;
  onError: (message: string) => void;
  isAlive: () => boolean;
}

export interface SnapshotLoader {
  load: (mode: LoadMode) => Promise<void>;
}

function errorText(raw: unknown): string {
  return raw instanceof Error ? raw.message : String(raw);
}

/**
 * 拉取协调（I79 §9.1）：单飞 + **尾随刷新**。
 *
 * 事件到达时若已有请求在飞，不丢弃——登记一次 pending，当前请求结束后再跑一次
 * （旧实现「在飞即丢弃」会吃掉最后一次补算完成事件）。尾随期间再来的事件继续登记，
 * 因此每个事件都保证「在它之后至少还有一次拉取」，不会丢更新。
 */
export function createSnapshotLoader<T>(
  fetch: () => Promise<T>,
  callbacks: SnapshotLoaderCallbacks<T>,
): SnapshotLoader {
  let inFlight: Promise<void> | null = null;
  let pending = false;
  let hasData = false;

  async function run(mode: LoadMode): Promise<void> {
    callbacks.onPhase(mode === "initial" ? "initial" : "refreshing");
    try {
      const data = await fetch();
      if (!callbacks.isAlive()) return;
      hasData = true;
      callbacks.onData(data);
      callbacks.onPhase("ready");
    } catch (raw) {
      if (!callbacks.isAlive()) return;
      callbacks.onError(errorText(raw));
      callbacks.onPhase(hasData ? "ready" : "initial");
    }
  }

  function load(mode: LoadMode): Promise<void> {
    const running = inFlight;
    if (running !== null) {
      pending = true;
      return running;
    }

    const task = (async (): Promise<void> => {
      let next: LoadMode = mode;
      while (true) {
        await run(next);
        if (!pending) return;
        pending = false;
        next = "refresh";
      }
    })().finally(() => {
      inFlight = null;
      pending = false;
    });

    inFlight = task;
    return task;
  }

  return { load };
}

export function useStorageSnapshot(): StorageSnapshotHandle {
  const [snapshot, setSnapshot] = useState<StorageBreakdownDto | null>(null);
  const [phase, setPhase] = useState<SnapshotPhase>("initial");
  const [error, setError] = useState<string | null>(null);

  const aliveRef = useRef(true);

  useEffect(() => {
    aliveRef.current = true;
    return () => {
      aliveRef.current = false;
    };
  }, []);

  const loader = useMemo(
    () =>
      createSnapshotLoader(() => storageService.getStorageBreakdown(), {
        onPhase: (next) => setPhase(next),
        onData: (data) => {
          setSnapshot(data);
          setError(null);
        },
        onError: (message) => setError(message),
        isAlive: () => aliveRef.current,
      }),
    [],
  );

  const load = useCallback(
    (mode: LoadMode): Promise<void> => loader.load(mode),
    [loader],
  );

  const refresh = useCallback((): void => {
    void load("refresh");
  }, [load]);

  const retry = useCallback((): void => {
    void load("initial");
  }, [load]);

  useEffect(() => {
    void load("initial");
  }, [load]);

  const deleteAttachments = useCallback(
    async (ids: string[]): Promise<AttachmentDeleteResultDto> => {
      const result = await storageService.deleteAttachments(ids);
      await load("refresh");
      return result;
    },
    [load],
  );

  const scanInventory =
    useCallback(async (): Promise<StorageInventorySnapshotStatusDto> => {
      return storageService.refreshStorageInventory();
    }, []);

  useEffect(() => {
    return subscribeSnapshotEvents({ listen }, () => {
      void load("refresh");
    });
  }, [load]);

  return {
    snapshot,
    phase,
    error,
    refresh,
    retry,
    deleteAttachments,
    scanInventory,
  };
}
