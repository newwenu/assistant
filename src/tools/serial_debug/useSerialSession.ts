import { useCallback, useEffect, useRef, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { translateKey } from "@/i18n";
import { createLogger } from "@/services/system/logger";
import type {
  SerialDebugErrorCodeDto,
  SerialRxBatchDto,
  SerialSessionDto,
} from "@/types/protocol.contract";
import { base64ToBytes } from "./byteCodec";
import {
  onSerialRx,
  onSerialStatus,
  serialDebugService,
} from "./serialDebug.service";

const log = createLogger("serialDebug");

const ERROR_KEYS: Record<SerialDebugErrorCodeDto, string> = {
  notOpen: "tools.serial_debug.errorNotOpen",
  alreadyOpen: "tools.serial_debug.errorAlreadyOpen",
  portNotFound: "tools.serial_debug.errorPortNotFound",
  permissionDenied: "tools.serial_debug.errorPermissionDenied",
  busy: "tools.serial_debug.errorBusy",
  io: "tools.serial_debug.errorIo",
  unsupported: "tools.serial_debug.errorUnsupported",
  invalidInput: "tools.serial_debug.errorInvalidInput",
};

export function serialDebugErrorText(
  code: SerialDebugErrorCodeDto | null,
): string {
  return translateKey(
    code === null ? "tools.serial_debug.errorIo" : ERROR_KEYS[code],
  );
}

export interface SerialLine {
  sessionId: number;
  seq: number;
  atMs: number;
  bytes: Uint8Array;
}

const MAX_LINES = 5000;
const MAX_BYTES = 2 * 1024 * 1024;
const MAX_QUEUED = 1000;

export interface UseSerialSessionResult {
  session: SerialSessionDto | null;
  lines: SerialLine[];
  truncated: boolean;
  paused: boolean;
  applySession: (next: SerialSessionDto) => void;
  setPaused: (paused: boolean) => void;
  clear: () => void;
  refreshStatus: () => Promise<void>;
}

export function useSerialSession(): UseSerialSessionResult {
  const [session, setSession] = useState<SerialSessionDto | null>(null);
  const [lines, setLines] = useState<SerialLine[]>([]);
  const [truncated, setTruncated] = useState(false);
  const [paused, setPausedState] = useState(false);

  const sessionRef = useRef<SerialSessionDto | null>(null);
  const pausedRef = useRef(false);
  const queueRef = useRef<SerialLine[]>([]);
  const lastSeqRef = useRef(0);
  const bytesRef = useRef(0);
  const rafRef = useRef<number | null>(null);

  const resetBuffers = useCallback(() => {
    queueRef.current = [];
    lastSeqRef.current = 0;
    bytesRef.current = 0;
    if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
    }
    setLines([]);
    setTruncated(false);
  }, []);

  const applySession = useCallback(
    (next: SerialSessionDto) => {
      const previous = sessionRef.current;
      sessionRef.current = next;
      setSession(next);
      if (previous === null || previous.sessionId !== next.sessionId) {
        resetBuffers();
      }
    },
    [resetBuffers],
  );

  const flush = useCallback(() => {
    rafRef.current = null;
    if (pausedRef.current) return;
    const queued = queueRef.current;
    if (queued.length === 0) return;
    queueRef.current = [];
    const addedBytes = queued.reduce((sum, line) => sum + line.bytes.length, 0);
    bytesRef.current += addedBytes;
    setLines((previous) => {
      let next = previous.concat(queued);
      let dropped = false;
      let total = bytesRef.current;
      while (next.length > MAX_LINES || total > MAX_BYTES) {
        const head = next[0];
        if (head === undefined) break;
        next = next.slice(1);
        total -= head.bytes.length;
        dropped = true;
      }
      bytesRef.current = total;
      if (dropped) setTruncated(true);
      return next;
    });
  }, []);

  const schedule = useCallback(() => {
    if (rafRef.current !== null) return;
    rafRef.current = requestAnimationFrame(flush);
  }, [flush]);

  const handleBatch = useCallback(
    (batch: SerialRxBatchDto) => {
      const current = sessionRef.current;
      if (current === null || !current.open) return;
      if (batch.sessionId !== current.sessionId) return;
      if (batch.seq <= lastSeqRef.current) return;
      lastSeqRef.current = batch.seq;
      queueRef.current.push({
        sessionId: batch.sessionId,
        seq: batch.seq,
        atMs: batch.receivedAtMs,
        bytes: base64ToBytes(batch.dataBase64),
      });
      if (queueRef.current.length > MAX_QUEUED) {
        queueRef.current = queueRef.current.slice(-MAX_QUEUED);
        setTruncated(true);
      }
      if (!pausedRef.current) schedule();
    },
    [schedule],
  );

  const refreshStatus = useCallback(async () => {
    applySession(await serialDebugService.status());
  }, [applySession]);

  const setPaused = useCallback(
    (value: boolean) => {
      pausedRef.current = value;
      setPausedState(value);
      if (!value) schedule();
    },
    [schedule],
  );

  const clear = useCallback(() => {
    resetBuffers();
  }, [resetBuffers]);

  useEffect(() => {
    let disposed = false;
    let unlistenRx: UnlistenFn | undefined;
    let unlistenStatus: UnlistenFn | undefined;

    void serialDebugService
      .status()
      .then((next) => {
        if (!disposed) applySession(next);
      })
      .catch(() => undefined);

    void onSerialRx(handleBatch).then((fn) => {
      if (disposed) fn();
      else unlistenRx = fn;
    });

    void onSerialStatus((next) => {
      if (disposed) return;
      if (next.error !== null) {
        log.debug(`status error ${next.error.code}`, next.error.message);
      }
      applySession(next);
    }).then((fn) => {
      if (disposed) fn();
      else unlistenStatus = fn;
    });

    return () => {
      disposed = true;
      unlistenRx?.();
      unlistenStatus?.();
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
    };
  }, [applySession, handleBatch]);

  return {
    session,
    lines,
    truncated,
    paused,
    applySession,
    setPaused,
    clear,
    refreshStatus,
  };
}
