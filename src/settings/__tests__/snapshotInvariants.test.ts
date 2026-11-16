import { describe, expect, it } from "vitest";
import { SettingOutlined } from "@ant-design/icons";
import { definePage } from "@/settings/core/registry/definePage";
import { validateSnapshot } from "@/settings/core/runtime/snapshot";
import { EMPTY_EXTENSION_INPUT } from "@/settings/core/types/snapshot";
import type {
  SnapshotSidebarEntry,
  SnapshotSource,
  ViolationRule,
} from "@/settings/core/types/snapshot";
import type {
  SettingsPageDefinition,
  SettingsPageManifest,
} from "@/settings/core/types/pageTypes";
import type {
  SettingDefinition,
  SettingSectionDef,
} from "@/settings/core/field/fieldRegistry";

function page(
  id: string,
  categoryId: string,
  extra: Partial<SettingsPageDefinition> = {},
): SettingsPageManifest {
  return definePage({
    id,
    categoryId,
    label: `label.${id}`,
    keywords: [],
    icon: SettingOutlined,
    order: 10,
    emptyPage: true,
    load: () => Promise.resolve({ default: () => null }),
    ...extra,
  });
}

function section(id: string, pageId: string): SettingSectionDef {
  return { id, pageId, title: `title.${id}` };
}

function nav(key: string): SnapshotSidebarEntry {
  return {
    key,
    label: `label.${key}`,
    icon: SettingOutlined,
    slot: "after-tools",
    order: 10,
  };
}

function field(key: string, sectionId?: string): SettingDefinition {
  return {
    key,
    group: "app",
    type: "boolean",
    default: false,
    label: `label.${key}`,
    section: sectionId,
  };
}

function baseSource(overrides: Partial<SnapshotSource> = {}): SnapshotSource {
  return {
    pages: [page("demo", "demo-category")],
    categories: [{ id: "demo-category", label: "label.demo", order: 10 }],
    sections: [section("demo-section", "demo")],
    fields: [field("demoFlag", "demo-section")],
    routes: EMPTY_EXTENSION_INPUT.routes,
    sidebar: EMPTY_EXTENSION_INPUT.sidebar,
    confs: [{ group: "app", invoke: "get_app_conf", pageId: "demo" }],
    ...overrides,
  };
}

function rules(source: SnapshotSource): ViolationRule[] {
  return validateSnapshot(source).map((violation) => violation.rule);
}

describe("settings snapshot invariants", () => {
  it("accepts a self-consistent extension input", () => {
    expect(validateSnapshot(baseSource())).toEqual([]);
  });

  it("reports duplicate ids for every collection", () => {
    const source = baseSource({
      pages: [page("demo", "demo-category"), page("demo", "demo-category")],
      categories: [
        { id: "demo-category", label: "a", order: 1 },
        { id: "demo-category", label: "b", order: 2 },
      ],
      sections: [
        section("demo-section", "demo"),
        section("demo-section", "demo"),
      ],
      fields: [
        field("demoFlag", "demo-section"),
        field("demoFlag", "demo-section"),
      ],
      routes: [
        {
          path: "/demo",
          element: () => null,
          boundary: "demo",
          order: 1,
          navParent: "/demo",
          visibility: "sidebar",
          keywords: [],
        },
        {
          path: "/demo",
          element: () => null,
          boundary: "demo",
          order: 2,
          navParent: "/demo",
          visibility: "sidebar",
          keywords: [],
        },
      ],
      sidebar: [
        {
          key: "/demo",
          label: "demo",
          icon: SettingOutlined,
          slot: "after-tools",
          order: 1,
        },
        {
          key: "/demo",
          label: "demo",
          icon: SettingOutlined,
          slot: "after-tools",
          order: 2,
        },
      ],
    });

    expect(rules(source)).toEqual([
      "duplicate-page-id",
      "duplicate-category-id",
      "duplicate-section-id",
      "duplicate-field-key",
      "duplicate-route-path",
      "duplicate-sidebar-key",
    ]);
  });

  it("reports a page whose category is missing", () => {
    const source = baseSource({
      pages: [page("demo", "ghost-category")],
      sections: [],
      fields: [],
      confs: [],
    });

    expect(rules(source)).toEqual(["unknown-page-category"]);
    expect(validateSnapshot(source)[0].target).toBe("ghost-category");
  });

  it("reports a section whose page is missing", () => {
    const source = baseSource({
      sections: [section("demo-section", "ghost-page")],
      fields: [],
      confs: [],
    });

    expect(rules(source)).toEqual(["unknown-section-page"]);
    expect(validateSnapshot(source)[0].target).toBe("ghost-page");
  });

  it("reports a field whose section is missing", () => {
    const source = baseSource({
      fields: [field("demoFlag", "ghost-section")],
      confs: [],
    });

    expect(rules(source)).toEqual(["unknown-field-section"]);
    expect(validateSnapshot(source)[0].target).toBe("ghost-section");
  });

  it("collects every violation instead of failing fast", () => {
    const source = baseSource({
      pages: [page("demo", "ghost-category")],
      sections: [section("demo-section", "ghost-page")],
      fields: [field("demoFlag", "ghost-section")],
      confs: [],
    });

    expect(rules(source)).toEqual([
      "unknown-page-category",
      "unknown-section-page",
      "unknown-field-section",
    ]);
  });

  it("reports violations together with their target and message", () => {
    const source = baseSource({ fields: [field("demoFlag", "ghost-section")] });
    const [violation] = validateSnapshot(source);

    expect(violation).toEqual({
      rule: "unknown-field-section",
      target: "ghost-section",
      message: 'setting "demoFlag" references unknown section "ghost-section"',
    });
  });

  it("reports a field whose conf group has no slot", () => {
    const source = baseSource({
      fields: [{ ...field("demoFlag", "demo-section"), group: "vault" }],
      confs: [],
    });

    expect(rules(source)).toEqual(["unknown-conf-group"]);
    expect(validateSnapshot(source)[0].target).toBe("vault");
  });

  it("reports a conf group declared twice", () => {
    const source = baseSource({
      confs: [
        { group: "app", invoke: "get_app_conf", pageId: "demo" },
        { group: "vault", invoke: "get_vault_conf" },
        { group: "vault", invoke: "get_vault_conf_alt" },
      ],
    });

    expect(rules(source)).toEqual(["duplicate-conf-group"]);
  });

  it("reports a conf slot whose page is missing", () => {
    const source = baseSource({
      confs: [{ group: "app", invoke: "get_app_conf", pageId: "ghost-page" }],
    });

    expect(rules(source)).toEqual(["unknown-conf-page"]);
    expect(validateSnapshot(source)[0].target).toBe("ghost-page");
  });

  it("reports a conf group with settings but no page", () => {
    const source = baseSource({
      confs: [{ group: "app", invoke: "get_app_conf" }],
    });

    expect(rules(source)).toEqual(["conf-group-without-page"]);
    expect(validateSnapshot(source)[0].target).toBe("app");
  });

  it("reports a conf slot pointing at a page without its settings", () => {
    const source = baseSource({
      pages: [page("demo", "demo-category"), page("empty", "demo-category")],
      confs: [{ group: "app", invoke: "get_app_conf", pageId: "empty" }],
    });

    expect(rules(source)).toEqual(["conf-page-without-fields"]);
    expect(validateSnapshot(source)[0].target).toBe("app");
  });

  it("reports a page with no sections unless emptyPage is declared", () => {
    const source = baseSource({
      pages: [page("demo", "demo-category", { emptyPage: false })],
      sections: [],
      fields: [],
      confs: [],
    });

    expect(rules(source)).toEqual(["page-without-sections"]);
    expect(validateSnapshot(source)[0].target).toBe("demo");
  });

  it("reports a page navParent that is not a sidebar key", () => {
    const source = baseSource({
      pages: [page("demo", "demo-category", { navParent: "ghost-nav" })],
      sidebar: [nav("demo-nav")],
    });

    expect(rules(source)).toEqual(["unknown-nav-parent", "orphan-sidebar-key"]);
    expect(validateSnapshot(source)[0].target).toBe("ghost-nav");
  });

  it("reports a route navParent that is not a sidebar key", () => {
    const source = baseSource({
      routes: [
        {
          path: "/demo",
          element: () => null,
          boundary: "demo",
          order: 1,
          navParent: "ghost-nav",
          visibility: "sidebar",
          keywords: [],
        },
      ],
    });

    expect(rules(source)).toEqual(["unknown-nav-parent"]);
    expect(validateSnapshot(source)[0].target).toBe("ghost-nav");
  });

  it("reports a sidebar entry nothing points at", () => {
    const source = baseSource({ sidebar: [nav("demo-nav")] });

    expect(rules(source)).toEqual(["orphan-sidebar-key"]);
    expect(validateSnapshot(source)[0].target).toBe("demo-nav");
  });

  it("reports a deepLinkOnly page that declares navParent", () => {
    const source = baseSource({
      pages: [
        page("demo", "demo-category", {
          navParent: "demo-nav",
          visibility: "deepLinkOnly",
        }),
      ],
      sidebar: [nav("demo-nav")],
    });

    expect(rules(source)).toEqual(["deep-link-with-nav-parent"]);
    expect(validateSnapshot(source)[0].target).toBe("demo");
  });

  it("reports a deepLinkOnly route that declares navParent", () => {
    const source = baseSource({
      routes: [
        {
          path: "/demo",
          element: () => null,
          boundary: "demo",
          order: 1,
          navParent: "demo-nav",
          visibility: "deepLinkOnly",
          keywords: [],
        },
      ],
      sidebar: [nav("demo-nav")],
    });

    expect(rules(source)).toEqual(["deep-link-with-nav-parent"]);
    expect(validateSnapshot(source)[0].target).toBe("/demo");
  });

  it("accepts a page and a route that share one sidebar entry", () => {
    const source = baseSource({
      pages: [page("demo", "demo-category", { navParent: "demo-nav" })],
      routes: [
        {
          path: "/demo",
          element: () => null,
          boundary: "demo",
          order: 1,
          navParent: "demo-nav",
          visibility: "sidebar",
          keywords: [],
        },
      ],
      sidebar: [nav("demo-nav")],
    });

    expect(validateSnapshot(source)).toEqual([]);
  });
});
