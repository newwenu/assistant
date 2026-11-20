import { useCallback, useEffect, useRef, useState } from "react";
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

export type SnapshotPhase = "initial" | "ready" | "refreshing";

type LoadMode = "initial" | "refresh";

export interface StorageSnapshotHandle {
  snapshot: StorageBreakdownDto | null;
  phase: SnapshotPhase;
  error: string | null;
  refresh: () => void;
  retry: () => void;
  deleteAttachments: (ids: string[]) => Promise<AttachmentDeleteResultDto>;
  scanInventory: () => Promise<StorageInventorySnapshotStatusDto>;
}

function errorText(raw: unknown): string {
  return raw instanceof Error ? raw.message : String(raw);
}

export function useStorageSnapshot(): StorageSnapshotHandle {
  const [snapshot, setSnapshot] = useState<StorageBreakdownDto | null>(null);
  const [phase, setPhase] = useState<SnapshotPhase>("initial");
  const [error, setError] = useState<string | null>(null);

  const inFlightRef = useRef<Promise<void> | null>(null);
  const hasDataRef = useRef(false);
  const aliveRef = useRef(true);

  useEffect(() => {
    aliveRef.current = true;
    return () => {
      aliveRef.current = false;
    };
  }, []);

  const load = useCallback((mode: LoadMode): Promise<void> => {
    const running = inFlightRef.current;
    if (running !== null) return running;

    setPhase(mode === "initial" ? "initial" : "refreshing");

    const task = (async (): Promise<void> => {
      try {
        const data = await storageService.getStorageBreakdown();
        if (!aliveRef.current) return;
        hasDataRef.current = true;
        setSnapshot(data);
        setError(null);
        setPhase("ready");
      } catch (raw) {
        if (!aliveRef.current) return;
        setError(errorText(raw));
        setPhase(hasDataRef.current ? "ready" : "initial");
      } finally {
        inFlightRef.current = null;
      }
    })();

    inFlightRef.current = task;
    return task;
  }, []);

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
    let unlisten: UnlistenFn | undefined;
    void listen(STORAGE_INVENTORY_UPDATED, () => {
      void load("refresh");
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch((raw: unknown) => {
        log.error("failed to subscribe storage inventory event", raw);
      });

    return () => {
      unlisten?.();
    };
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
