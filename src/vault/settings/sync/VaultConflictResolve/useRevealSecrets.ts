import { useCallback, useEffect, useRef, useState } from "react";

import { vaultService } from "../../../core/vault.service";
import { vaultErrorMessage } from "../../../core/display/vaultError";
import type { ConflictSide } from "./types";

/**
 * 遮蔽字段的按需显示：点击后取回明文并按 `expiresAt` 起计时器，
 * 到时自动重新遮蔽；弹窗关闭或卸载时统一清理计时器。
 */
export function useRevealSecrets(
  conflictPath: string,
  onError: (text: string) => void,
) {
  const [revealed, setRevealed] = useState<Record<string, string>>({});
  const [revealingKey, setRevealingKey] = useState<string | null>(null);
  const revealTimers = useRef<Record<string, number>>({});

  const clearTimers = useCallback(() => {
    Object.values(revealTimers.current).forEach((timer) =>
      window.clearTimeout(timer),
    );
    revealTimers.current = {};
  }, []);

  useEffect(() => () => clearTimers(), [clearTimers]);

  const revealField = async (side: ConflictSide, field: string) => {
    const key = `${side}:${field}`;
    setRevealingKey(key);
    try {
      const result = await vaultService.revealConflictField(
        conflictPath,
        side,
        field,
      );
      setRevealed((prev) => ({ ...prev, [key]: result.value }));
      const timer = revealTimers.current[key];
      if (timer) window.clearTimeout(timer);
      const ttl = Math.max(0, result.expiresAt - Date.now());
      revealTimers.current[key] = window.setTimeout(() => {
        delete revealTimers.current[key];
        setRevealed((prev) => {
          const next = { ...prev };
          delete next[key];
          return next;
        });
      }, ttl);
    } catch (e) {
      onError(vaultErrorMessage(e));
    } finally {
      setRevealingKey(null);
    }
  };

  const reveal = (side: ConflictSide, field: string): void => {
    void revealField(side, field);
  };

  const reset = useCallback(() => {
    setRevealed({});
    setRevealingKey(null);
    clearTimers();
  }, [clearTimers]);

  return { revealed, revealingKey, reveal, reset };
}
