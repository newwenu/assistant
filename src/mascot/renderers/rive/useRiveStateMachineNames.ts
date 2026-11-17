import { useEffect, useState } from "react";
import { RiveFile } from "@rive-app/react-webgl2";

import { createLogger } from "@/services/system/logger";

const log = createLogger("rive-probe");

const cache = new Map<string, string[]>();
const pending = new Map<string, Promise<string[]>>();

/**
 * 不挂画布地加载素材，读出默认画板的状态机名。
 *
 * 挂载时就必须带上状态机名：不传的话 Rive 会播放画板的第一个线性动画并打出
 * `default-state-machine` 告警，等加载完再靠重挂补救会闪一帧错误姿势。
 */
function probeStateMachineNames(src: string): Promise<string[]> {
  const hit = cache.get(src);
  if (hit) return Promise.resolve(hit);
  const running = pending.get(src);
  if (running) return running;

  const task = (async () => {
    const file = new RiveFile({ src });
    try {
      await file.init();
      const artboard = file.getInstance().defaultArtboard();
      try {
        const count = artboard.stateMachineCount();
        const names: string[] = [];
        for (let i = 0; i < count; i += 1) {
          const name = artboard.stateMachineByIndex(i)?.name;
          if (name) names.push(name);
        }
        cache.set(src, names);
        return names;
      } finally {
        artboard.delete();
      }
    } catch (error) {
      log.debug("probe failed", { src, error });
      throw error;
    } finally {
      pending.delete(src);
      file.cleanup();
    }
  })();

  pending.set(src, task);
  return task;
}

export interface StateMachineProbe {
  names: string[] | null;
  ready: boolean;
}

interface ProbeResult {
  src: string | null;
  names: string[] | null;
  settled: boolean;
}

function snapshot(src: string | null): ProbeResult {
  const names = src ? (cache.get(src) ?? null) : null;
  return { src, names, settled: src === null || names !== null };
}

/** `src` 为 null 表示无需探测（内置素材自带状态机名，或素材来自 buffer）。 */
export function useRiveStateMachineNames(
  src: string | null,
): StateMachineProbe {
  const [result, setResult] = useState<ProbeResult>(() => snapshot(src));

  useEffect(() => {
    if (!src) {
      setResult(snapshot(null));
      return;
    }
    const hit = cache.get(src);
    if (hit) {
      setResult({ src, names: hit, settled: true });
      return;
    }
    let cancelled = false;
    setResult({ src, names: null, settled: false });
    probeStateMachineNames(src)
      .then((names) => {
        if (!cancelled) setResult({ src, names, settled: true });
      })
      .catch(() => {
        if (!cancelled) setResult({ src, names: null, settled: true });
      });
    return () => {
      cancelled = true;
    };
  }, [src]);

  if (result.src !== src) {
    return { names: src ? (cache.get(src) ?? null) : null, ready: false };
  }
  return { names: result.names, ready: result.settled };
}
