import { describe, expect, it } from "vitest";
import { SettingOutlined } from "@ant-design/icons";
import { probeModule } from "@/features/_probe/module";
import { vaultModule } from "@/features/vault/module";
import type { FeatureModule } from "@/features/types";

const modules: readonly FeatureModule[] = [vaultModule, probeModule];

function checkModule(module: FeatureModule): string[] {
  const problems: string[] = [];
  const sidebarKeys = new Set((module.sidebar ?? []).map((entry) => entry.key));
  const categoryIds = new Set(
    (module.categories ?? []).map((category) => category.id),
  );
  const pageIds = new Set(
    (module.settings?.pages ?? []).map((page) => page.id),
  );
  const sectionIds = new Set(
    (module.settings?.sections ?? []).map((section) => section.id),
  );
  const confGroups = new Set((module.confs ?? []).map((slot) => slot.group));

  const requireRef = (
    owner: string,
    field: string,
    ref: string | undefined,
    known: ReadonlySet<string>,
  ): void => {
    if (ref === undefined) return;
    if (!known.has(ref)) {
      problems.push(`${owner} 的 ${field} 指向未声明项 "${ref}"`);
    }
  };

  (module.routes ?? []).forEach((route) => {
    requireRef(
      `route "${route.path}"`,
      "navParent",
      route.navParent,
      sidebarKeys,
    );
  });

  (module.settings?.pages ?? []).forEach((page) => {
    requireRef(`page "${page.id}"`, "categoryId", page.categoryId, categoryIds);
    requireRef(`page "${page.id}"`, "navParent", page.navParent, sidebarKeys);
  });

  (module.settings?.sections ?? []).forEach((section) => {
    requireRef(`section "${section.id}"`, "pageId", section.pageId, pageIds);
  });

  (module.settings?.fields ?? []).forEach((field) => {
    requireRef(`setting "${field.key}"`, "group", field.group, confGroups);
    requireRef(`setting "${field.key}"`, "section", field.section, sectionIds);
  });

  (module.confs ?? []).forEach((slot) => {
    requireRef(`conf group "${slot.group}"`, "pageId", slot.pageId, pageIds);
  });

  const referencedSidebarKeys = new Set<string>();
  for (const route of module.routes ?? []) {
    if (route.navParent) referencedSidebarKeys.add(route.navParent);
  }
  for (const page of module.settings?.pages ?? []) {
    if (page.navParent) referencedSidebarKeys.add(page.navParent);
  }
  for (const key of sidebarKeys) {
    if (!referencedSidebarKeys.has(key)) {
      problems.push(
        `sidebar key "${key}" 未被任何 route 或 page 的 navParent 引用`,
      );
    }
  }

  return problems;
}

describe("特性模块 id 引用完整性", () => {
  modules.forEach((module) => {
    it(`"${module.id}" 的跨槽位引用全部命中`, () => {
      expect(checkModule(module)).toEqual([]);
    });
  });

  it("探针模块覆盖全部八类槽位", () => {
    expect(probeModule.routes).toHaveLength(1);
    expect(probeModule.sidebar).toHaveLength(1);
    expect(probeModule.categories).toHaveLength(1);
    expect(probeModule.settings?.pages).toHaveLength(1);
    expect(probeModule.settings?.sections).toHaveLength(1);
    expect(probeModule.settings?.fields).toHaveLength(2);
    expect(probeModule.confs).toHaveLength(1);
    expect(probeModule.ipc?.namespace).toBe("probe");
  });

  it("故意写错的引用会被逐条报出", () => {
    const broken: FeatureModule = {
      id: "broken",
      ipc: { namespace: "broken" },
      routes: [
        {
          path: "/broken",
          load: () => Promise.resolve({ default: () => null }),
          boundary: "broken",
          navParent: "ghost-sidebar",
        },
      ],
      sidebar: [
        {
          key: "broken-sidebar",
          label: "broken",
          icon: SettingOutlined,
          slot: "after-tools",
        },
      ],
      categories: [{ id: "broken-category", label: "broken" }],
      settings: {
        pages: [
          {
            id: "broken",
            categoryId: "ghost-category",
            label: "broken",
            keywords: [],
            icon: SettingOutlined,
            order: 1,
            load: () => Promise.resolve({ default: () => null }),
          },
        ],
        sections: [
          { id: "broken-section", pageId: "ghost-page", title: "broken" },
        ],
        fields: [
          {
            key: "brokenFlag",
            group: "app",
            section: "ghost-section",
            type: "boolean",
            default: false,
            label: "broken",
          },
        ],
      },
      confs: [{ group: "orb", invoke: "get_orb_conf", pageId: "ghost" }],
    };

    const problems = checkModule(broken);
    expect(problems).toEqual([
      'route "/broken" 的 navParent 指向未声明项 "ghost-sidebar"',
      'page "broken" 的 categoryId 指向未声明项 "ghost-category"',
      'section "broken-section" 的 pageId 指向未声明项 "ghost-page"',
      'setting "brokenFlag" 的 group 指向未声明项 "app"',
      'setting "brokenFlag" 的 section 指向未声明项 "ghost-section"',
      'conf group "orb" 的 pageId 指向未声明项 "ghost"',
      'sidebar key "broken-sidebar" 未被任何 route 或 page 的 navParent 引用',
    ]);
  });
});
