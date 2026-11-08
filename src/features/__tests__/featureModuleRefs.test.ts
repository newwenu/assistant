import { describe, expect, it } from "vitest";
import { SettingOutlined } from "@ant-design/icons";
import { vaultModule } from "@/features/vault/module";
import type { FeatureModule } from "@/features/types";

const modules: readonly FeatureModule[] = [vaultModule];

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

const FULL_SLOT_MODULE: FeatureModule = {
  id: "full-slot",
  ipc: { namespace: "full-slot" },
  routes: [
    {
      path: "/full-slot",
      load: () => Promise.resolve({ default: () => null }),
      boundary: "full-slot",
      navParent: "full-slot-nav",
    },
  ],
  sidebar: [
    {
      key: "full-slot-nav",
      label: "full-slot",
      icon: SettingOutlined,
      slot: "after-tools",
    },
  ],
  categories: [{ id: "full-slot-category", label: "full-slot", order: 1 }],
  settings: {
    pages: [
      {
        id: "full-slot",
        categoryId: "full-slot-category",
        navParent: "full-slot-nav",
        label: "full-slot",
        keywords: [],
        icon: SettingOutlined,
        order: 1,
        load: () => Promise.resolve({ default: () => null }),
      },
    ],
    sections: [
      { id: "full-slot-section", pageId: "full-slot", title: "full-slot" },
    ],
    fields: [
      {
        key: "fullSlotFlag",
        group: "orb",
        section: "full-slot-section",
        type: "boolean",
        default: false,
        label: "full-slot",
      },
    ],
  },
  confs: [{ group: "orb", invoke: "get_orb_conf", pageId: "full-slot" }],
};

describe("特性模块 id 引用完整性", () => {
  modules.forEach((module) => {
    it(`"${module.id}" 的跨槽位引用全部命中`, () => {
      expect(checkModule(module)).toEqual([]);
    });
  });

  it("合成模块覆盖全部八类槽位且自洽", () => {
    expect(FULL_SLOT_MODULE.routes).toHaveLength(1);
    expect(FULL_SLOT_MODULE.sidebar).toHaveLength(1);
    expect(FULL_SLOT_MODULE.categories).toHaveLength(1);
    expect(FULL_SLOT_MODULE.settings?.pages).toHaveLength(1);
    expect(FULL_SLOT_MODULE.settings?.sections).toHaveLength(1);
    expect(FULL_SLOT_MODULE.settings?.fields).toHaveLength(1);
    expect(FULL_SLOT_MODULE.confs).toHaveLength(1);
    expect(FULL_SLOT_MODULE.ipc?.namespace).toBe("full-slot");
    expect(checkModule(FULL_SLOT_MODULE)).toEqual([]);
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
