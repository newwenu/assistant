import type { SettingsPageManifest } from "@/settings/core/types/pageTypes";
import type { PreparedRoute } from "./aggregate";

export function resolveSidebarKeys(
  pathname: string,
  routes: readonly PreparedRoute[],
  pages: readonly SettingsPageManifest[],
): string[] {
  const route = routes.find((item) => item.path === pathname);
  if (route) {
    return [route.navParent ?? route.path];
  }

  const settingsPrefix = "/settings/";
  if (pathname.startsWith(settingsPrefix)) {
    const pageId = pathname.slice(settingsPrefix.length).split("/")[0];
    const page = pages.find((item) => item.id === pageId);
    if (page?.navParent) return [page.navParent];
  }

  return [pathname];
}

export function resolveSidebarTarget(
  key: string,
  routes: readonly PreparedRoute[],
  pages: readonly SettingsPageManifest[],
): string {
  const route = routes.find((item) => item.navParent === key);
  if (route) return route.path;

  const page = pages.find(
    (item) => item.navParent === key && item.visibility !== "deepLinkOnly",
  );
  if (page) return `/settings/${page.id}`;

  return key;
}
