import { useEffect, useState, useSyncExternalStore } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { RiveManifestDto } from "@/types/protocol.contract";
import { isLocalAssetUrl } from "@/chat/attachment/paths";
import { settingService } from "@/services/settingService";
import { createLogger } from "@/services/system/logger";
import { SettingEvent } from "@/types/events";
import {
  bumpRiveManifestRevision,
  getRiveManifestRevision,
  subscribeRiveManifestRevision,
} from "./riveManifestRevision";

const log = createLogger("rive-manifest");

export interface RiveManifestState {
  manifest: RiveManifestDto | null;
  /** 声明读取完成（无论是否存在）；未完成时调用方应等待，避免按兜底配置先行挂载。 */
  ready: boolean;
}

interface ManifestSnapshot {
  key: string | null;
  manifest: RiveManifestDto | null;
}

/**
 * 读取与自定义素材同目录的伴生声明。
 * `assetType` 为 null 表示当前用的是内置素材，不读取声明；
 * `src` 变化（素材被替换、版本号变化）时重新读取。
 *
 * 声明只对安装进素材目录的那份文件生效，外部 URL 视为无声明，
 * 避免本地残留的声明漂移到别的素材上。
 *
 * 返回值按当前 key 判定：素材刚切换的那一帧立即是 `ready: false`，
 * 不会把上一个素材的声明当成新素材的声明。
 */
export function useRiveManifest(
  assetType: string | null,
  src?: string | null,
): RiveManifestState {
  const key = assetType ? `${assetType}|${src ?? ""}` : null;
  const revision = useSyncExternalStore(
    subscribeRiveManifestRevision,
    getRiveManifestRevision,
  );
  const [snapshot, setSnapshot] = useState<ManifestSnapshot>({
    key: null,
    manifest: null,
  });

  useEffect(() => {
    let disposed = false;
    let unlisten: UnlistenFn | null = null;
    listen(SettingEvent.MascotManifestChanged, () => {
      bumpRiveManifestRevision();
    })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch((e: unknown) => {
        log.debug("manifest event unavailable", e);
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (assetType === null) {
      setSnapshot({ key: null, manifest: null });
      return;
    }
    let cancelled = false;
    const settle = (manifest: RiveManifestDto | null) => {
      if (!cancelled) setSnapshot({ key, manifest });
    };
    void (async () => {
      if (!isLocalAssetUrl(src ?? "")) {
        settle(null);
        return;
      }
      settle(await settingService.getMascotManifest(assetType));
    })().catch((e: unknown) => {
      log.warn("read manifest failed", e);
      settle(null);
    });
    return () => {
      cancelled = true;
    };
  }, [key, assetType, src, revision]);

  if (key === null) return { manifest: null, ready: true };
  if (snapshot.key !== key) return { manifest: null, ready: false };
  return { manifest: snapshot.manifest, ready: true };
}
