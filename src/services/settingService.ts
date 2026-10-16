import { invoke } from "@tauri-apps/api/core";
import type {
  AppConf,
  OrbConf,
  MascotConf,
} from "../settings/core/types/confTypes";

function toSnakeCaseKey(key: string): string {
  return key.replace(/[A-Z]/g, (ch) => `_${ch.toLowerCase()}`);
}

function patchToSnakeCase(
  patch: Record<string, unknown>,
): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(patch)) {
    result[toSnakeCaseKey(key)] = value;
  }
  return result;
}

export interface InstallAssetResult {
  installedPath: string;
  bytes: number;
  version: number;
}

export const settingService = {
  getAppConf: (): Promise<AppConf> => invoke<AppConf>("get_app_conf"),

  getOrbConf: (): Promise<OrbConf> => invoke<OrbConf>("get_orb_conf"),

  getMascotConf: (): Promise<MascotConf> =>
    invoke<MascotConf>("get_mascot_conf"),

  getAppVersion: (): Promise<string> => invoke<string>("get_app_version"),

  getRustcVersion: (): Promise<string> => invoke<string>("get_rustc_version"),

  installMascotAsset: (
    assetType: string,
    filePath: string,
  ): Promise<InstallAssetResult> =>
    invoke<InstallAssetResult>("install_mascot_asset", {
      assetType,
      filePath,
    }),

  uninstallMascotAsset: (assetType: string): Promise<void> =>
    invoke("uninstall_mascot_asset", { assetType }),

  emitThemeChanged: (resolved: string): Promise<void> =>
    invoke("emit_theme_changed", { resolved }),

  patchConf: (group: string, patch: Record<string, unknown>): Promise<void> =>
    invoke("patch_conf", { group, patch: patchToSnakeCase(patch) }),
};
