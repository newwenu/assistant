import { describe, expect, it } from "vitest";
import { SettingOutlined } from "@ant-design/icons";
import { definePage } from "@/settings/core/definePage";
import { resolveSidebarKeys, resolveSidebarTarget } from "@/features/nav";
import type { PreparedRoute } from "@/features/aggregate";
import type { SettingsPageManifest } from "@/settings/core/types/pageTypes";

function route(path: string, navParent?: string): PreparedRoute {
  return {
    path,
    element: () => null,
    boundary: path,
    order: 0,
    navParent,
    visibility: "sidebar",
    keywords: [],
  };
}

function page(
  id: string,
  navParent?: string,
  visibility: "sidebar" | "deepLinkOnly" = "sidebar",
): SettingsPageManifest {
  return definePage({
    id,
    categoryId: "demo",
    label: `label.${id}`,
    keywords: [],
    icon: SettingOutlined,
    order: 10,
    navParent,
    visibility,
    emptyPage: true,
    load: () => Promise.resolve({ default: () => null }),
  });
}

describe("resolveSidebarKeys", () => {
  it("highlights the sidebar key a route declares as navParent", () => {
    expect(
      resolveSidebarKeys("/vault", [route("/vault", "vault")], []),
    ).toEqual(["vault"]);
  });

  it("falls back to the route path when navParent is absent", () => {
    expect(resolveSidebarKeys("/demo", [route("/demo")], [])).toEqual([
      "/demo",
    ]);
  });

  it("highlights the sidebar key a settings page declares as navParent", () => {
    expect(
      resolveSidebarKeys("/settings/demo", [], [page("demo", "demo-nav")]),
    ).toEqual(["demo-nav"]);
  });

  it("falls back to the page path when the page has no navParent", () => {
    expect(resolveSidebarKeys("/settings/demo", [], [page("demo")])).toEqual([
      "/settings/demo",
    ]);
  });

  it("gives two pages sharing one sidebar entry the same selection", () => {
    const pages = [page("demo-a", "demo-nav"), page("demo-b", "demo-nav")];
    expect(resolveSidebarKeys("/settings/demo-a", [], pages)).toEqual([
      "demo-nav",
    ]);
    expect(resolveSidebarKeys("/settings/demo-b", [], pages)).toEqual([
      "demo-nav",
    ]);
  });

  it("is independent of the declaration order of routes", () => {
    const ordered = [route("/demo-a", "nav-a"), route("/demo-b", "nav-b")];
    const shuffled = [route("/demo-b", "nav-b"), route("/demo-a", "nav-a")];

    expect(resolveSidebarKeys("/demo-a", ordered, [])).toEqual(["nav-a"]);
    expect(resolveSidebarKeys("/demo-a", shuffled, [])).toEqual(["nav-a"]);
  });

  it("ignores an unknown pathname", () => {
    expect(resolveSidebarKeys("/unknown", [route("/demo")], [])).toEqual([
      "/unknown",
    ]);
  });
});

describe("resolveSidebarTarget", () => {
  it("lands on the route that declares the key as navParent", () => {
    expect(
      resolveSidebarTarget("demo-nav", [route("/demo", "demo-nav")], []),
    ).toBe("/demo");
  });

  it("lands on the settings page when only a page declares the key", () => {
    expect(
      resolveSidebarTarget("demo-nav", [], [page("demo", "demo-nav")]),
    ).toBe("/settings/demo");
  });

  it("prefers a route over a settings page sharing the same key", () => {
    expect(
      resolveSidebarTarget(
        "demo-nav",
        [route("/demo", "demo-nav")],
        [page("demo", "demo-nav")],
      ),
    ).toBe("/demo");
  });

  it("keeps a path-shaped key working for entries without navParent", () => {
    expect(resolveSidebarTarget("/demo", [], [])).toBe("/demo");
  });
});
