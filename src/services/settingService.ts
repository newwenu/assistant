import { invoke } from "@tauri-apps/api/core";
import type {
  AppConf,
  OrbConf,
  MascotConf,
} from "../settings/core/types/confTypes";

export interface InstallAssetResult {
  installed_path: string;
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
    invoke("patch_conf", { group, patch }),
};
