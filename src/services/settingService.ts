import { tauriInvoke as invoke } from "./tauriInvoke";

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

  patchConf: (group: string, patch: Record<string, unknown>): Promise<void> =>
    invoke("patch_conf", { group, patch: patchToSnakeCase(patch) }),

  appQuit: (): Promise<void> => invoke("app_quit"),
};
