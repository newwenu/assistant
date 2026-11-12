import { createLogger } from "@/services/logger";
import {
  getBuiltinCategories,
  getBuiltinConfSlots,
  getBuiltinDefinitions,
  getBuiltinPages,
  getBuiltinSections,
  getBuiltinVersion,
  getOverviewPage,
} from "./builtinRegistry";
import {
  EMPTY_EXTENSION_INPUT,
  type SettingsExtensionInput,
  type SettingsSnapshot,
  type SnapshotCategory,
  type SnapshotSidebarEntry,
  type SnapshotSource,
  type Violation,
} from "./types/snapshot";
import type {
  SettingsCategoryId,
  SettingsPageId,
  SettingsPageManifest,
} from "./types/pageTypes";
import type {
  SettingDefinition,
  SettingSectionDef,
} from "./field/fieldRegistry";
import type { ConfSlot } from "./types/confSlot";

const log = createLogger("settings-snapshot");

let extensionInput: SettingsExtensionInput = EMPTY_EXTENSION_INPUT;
let bound = false;
let cache: { key: string; value: SettingsSnapshot } | null = null;
let queried = false;

export function bindExtensionInput(input: SettingsExtensionInput): void {
  if (
    import.meta.env.DEV &&
    queried &&
    input.id === extensionInput.id &&
    input !== extensionInput
  ) {
    throw new Error(
      `[settings] extension input re-bound in place without a new id: ${input.id}`,
    );
  }
  extensionInput = input;
  bound = true;
}

export function getExtensionInputId(): string {
  return extensionInput.id;
}

function assertBound(): void {
  if (import.meta.env.DEV && !bound) {
    throw new Error(
      "[settings] snapshot queried before bindExtensionInput(); check main.tsx startup order",
    );
  }
}

function groupBy<T>(
  items: readonly T[],
  keyOf: (item: T) => string | undefined,
): ReadonlyMap<string, readonly T[]> {
  const map = new Map<string, T[]>();
  for (const item of items) {
    const key = keyOf(item);
    if (!key) continue;
    const bucket = map.get(key);
    if (bucket) bucket.push(item);
    else map.set(key, [item]);
  }
  return new Map([...map].map(([k, v]) => [k, Object.freeze(v)]));
}

function collectDuplicates<T>(
  items: readonly T[],
  keyOf: (item: T) => string,
  rule: Violation["rule"],
  label: string,
  violations: Violation[],
): T[] {
  const seen = new Set<string>();
  const kept: T[] = [];
  for (const item of items) {
    const key = keyOf(item);
    if (seen.has(key)) {
      violations.push({
        rule,
        target: key,
        message: `duplicate ${label}: ${key}`,
      });
      continue;
    }
    seen.add(key);
    kept.push(item);
  }
  return kept;
}

function unknownRefs<T>(
  items: readonly T[],
  refOf: (item: T) => string | undefined,
  known: ReadonlySet<string>,
  rule: Violation["rule"],
  describe: (item: T, ref: string) => string,
  violations: Violation[],
): T[] {
  const kept: T[] = [];
  for (const item of items) {
    const ref = refOf(item);
    if (ref !== undefined && !known.has(ref)) {
      violations.push({ rule, target: ref, message: describe(item, ref) });
      continue;
    }
    kept.push(item);
  }
  return kept;
}

function normalize(source: SnapshotSource): {
  clean: SnapshotSource;
  violations: Violation[];
} {
  const violations: Violation[] = [];

  const pages = collectDuplicates(
    source.pages,
    (page) => page.id,
    "duplicate-page-id",
    "page id",
    violations,
  );
  const categories = collectDuplicates(
    source.categories,
    (category) => category.id,
    "duplicate-category-id",
    "category id",
    violations,
  );
  const sections = collectDuplicates(
    source.sections,
    (section) => section.id,
    "duplicate-section-id",
    "section id",
    violations,
  );
  const fields = collectDuplicates(
    source.fields,
    (field) => field.key,
    "duplicate-field-key",
    "setting key",
    violations,
  );
  const routes = collectDuplicates(
    source.routes,
    (route) => route.path,
    "duplicate-route-path",
    "route path",
    violations,
  );
  const sidebar = collectDuplicates(
    source.sidebar,
    (entry) => entry.key,
    "duplicate-sidebar-key",
    "sidebar key",
    violations,
  );
  const confs = collectDuplicates(
    source.confs,
    (slot) => slot.group,
    "duplicate-conf-group",
    "conf group",
    violations,
  );

  const categoryIds = new Set(categories.map((category) => category.id));
  const keptPages = unknownRefs(
    pages,
    (page) => page.categoryId,
    categoryIds,
    "unknown-page-category",
    (page, ref) => `page "${page.id}" references unknown category "${ref}"`,
    violations,
  );

  const pageIds = new Set<string>([
    getOverviewPage().id,
    ...keptPages.map((page) => page.id),
  ]);
  const keptSections = unknownRefs(
    sections,
    (section) => section.pageId,
    pageIds,
    "unknown-section-page",
    (section, ref) =>
      `section "${section.id}" references unknown page "${ref}"`,
    violations,
  );

  const sectionIds = new Set(keptSections.map((section) => section.id));
  const keptFieldsBySection = unknownRefs(
    fields,
    (field) => field.section,
    sectionIds,
    "unknown-field-section",
    (field, ref) =>
      `setting "${field.key}" references unknown section "${ref}"`,
    violations,
  );

  const confGroupIds = new Set(confs.map((slot) => slot.group));
  const keptFields = unknownRefs(
    keptFieldsBySection,
    (field) => field.group,
    confGroupIds,
    "unknown-conf-group",
    (field, ref) =>
      `setting "${field.key}" references unregistered conf group "${ref}"`,
    violations,
  );

  const sectionPageIds = new Map(
    keptSections.map((item) => [item.id, item.pageId] as const),
  );
  const fieldGroups = new Set(keptFields.map((item) => item.group));
  const groupPagePairs = new Set<string>();
  for (const item of keptFields) {
    const pageId =
      item.pageId ??
      (item.section === undefined
        ? undefined
        : sectionPageIds.get(item.section));
    if (pageId !== undefined) {
      groupPagePairs.add(`${item.group}\u0000${pageId}`);
    }
  }

  const keptConfs: ConfSlot[] = [];
  for (const slot of confs) {
    const pageId = slot.pageId;
    if (pageId === undefined) {
      if (fieldGroups.has(slot.group)) {
        violations.push({
          rule: "conf-group-without-page",
          target: slot.group,
          message: `conf group "${slot.group}" has settings but declares no pageId`,
        });
      }
      keptConfs.push(slot);
      continue;
    }
    if (!pageIds.has(pageId)) {
      violations.push({
        rule: "unknown-conf-page",
        target: pageId,
        message: `conf group "${slot.group}" references unknown page "${pageId}"`,
      });
      keptConfs.push({
        group: slot.group,
        invoke: slot.invoke,
        namespace: slot.namespace,
      });
      continue;
    }
    if (!groupPagePairs.has(`${slot.group}\u0000${pageId}`)) {
      violations.push({
        rule: "conf-page-without-fields",
        target: slot.group,
        message: `conf group "${slot.group}" declares page "${pageId}" but no setting of this group lives there`,
      });
      keptConfs.push({
        group: slot.group,
        invoke: slot.invoke,
        namespace: slot.namespace,
      });
      continue;
    }
    keptConfs.push(slot);
  }

  const sidebarKeys = new Set(sidebar.map((entry) => entry.key));
  const pagesWithNav = unknownRefs(
    keptPages,
    (page) => page.navParent,
    sidebarKeys,
    "unknown-nav-parent",
    (page, ref) =>
      `page "${page.id}" declares navParent "${ref}" which is not a sidebar key`,
    violations,
  );
  const routesWithNav = unknownRefs(
    routes,
    (route) => route.navParent,
    sidebarKeys,
    "unknown-nav-parent",
    (route, ref) =>
      `route "${route.path}" declares navParent "${ref}" which is not a sidebar key`,
    violations,
  );

  const referencedSidebarKeys = new Set<string>();
  for (const page of pagesWithNav) {
    if (page.navParent) referencedSidebarKeys.add(page.navParent);
  }
  for (const route of routesWithNav) {
    if (route.navParent) referencedSidebarKeys.add(route.navParent);
  }
  const keptSidebar: SnapshotSidebarEntry[] = [];
  for (const entry of sidebar) {
    if (!referencedSidebarKeys.has(entry.key)) {
      violations.push({
        rule: "orphan-sidebar-key",
        target: entry.key,
        message: `sidebar key "${entry.key}" is not referenced by any page or route navParent`,
      });
      continue;
    }
    keptSidebar.push(entry);
  }

  const finalPages = pagesWithNav.map((page) => {
    if (page.visibility === "deepLinkOnly" && page.navParent) {
      violations.push({
        rule: "deep-link-with-nav-parent",
        target: page.id,
        message: `page "${page.id}" is deepLinkOnly but declares navParent "${page.navParent}"`,
      });
      return { ...page, navParent: undefined };
    }
    return page;
  });
  const finalRoutes = routesWithNav.map((route) => {
    if (route.visibility === "deepLinkOnly" && route.navParent) {
      violations.push({
        rule: "deep-link-with-nav-parent",
        target: route.path,
        message: `route "${route.path}" is deepLinkOnly but declares navParent "${route.navParent}"`,
      });
      return { ...route, navParent: undefined };
    }
    return route;
  });

  const pagesWithSections = new Set(
    keptSections.map((section) => section.pageId),
  );
  for (const page of finalPages) {
    if (!page.emptyPage && !pagesWithSections.has(page.id)) {
      violations.push({
        rule: "page-without-sections",
        target: page.id,
        message: `page "${page.id}" has no sections and does not declare emptyPage: true`,
      });
    }
  }

  return {
    clean: {
      pages: finalPages,
      categories,
      sections: keptSections,
      fields: keptFields,
      routes: finalRoutes,
      sidebar: keptSidebar,
      confs: keptConfs,
    },
    violations,
  };
}

export function validateSnapshot(source: SnapshotSource): Violation[] {
  return normalize(source).violations;
}

function buildSnapshot(
  version: number,
  input: SettingsExtensionInput,
): SettingsSnapshot {
  const source: SnapshotSource = {
    pages: [...getBuiltinPages(), ...input.pages],
    categories: [...getBuiltinCategories(), ...input.categories],
    sections: [...getBuiltinSections(), ...input.sections],
    fields: [...getBuiltinDefinitions(), ...input.fields],
    routes: input.routes,
    sidebar: input.sidebar,
    confs: [...getBuiltinConfSlots(), ...input.confs],
  };

  const { clean, violations } = normalize(source);
  if (violations.length > 0) {
    const message = violations
      .map((violation) => `[${violation.rule}] ${violation.message}`)
      .join("\n");
    if (import.meta.env.DEV) {
      throw new Error(`[settings] snapshot invariants violated:\n${message}`);
    }
    log.error("snapshot invariants violated, dropping offending entries", {
      extensionId: input.id,
      violations,
    });
  }

  const pages: SettingsPageManifest[] = clean.pages
    .slice()
    .sort((a, b) => a.order - b.order);

  const pageMap = new Map<SettingsPageId, SettingsPageManifest>(
    [getOverviewPage(), ...pages].map((page) => [page.id, page]),
  );

  const categories: SnapshotCategory[] = clean.categories
    .slice()
    .sort((a, b) => a.order - b.order)
    .map((category) => ({
      id: category.id,
      label: category.label,
      order: category.order,
      pages: pages.filter((page) => page.categoryId === category.id),
    }))
    .filter((category) => category.pages.length > 0);

  const categoryMap = new Map<SettingsCategoryId, SnapshotCategory>(
    categories.map((category) => [category.id, category]),
  );

  const sections: readonly SettingSectionDef[] = clean.sections;
  const fields: readonly SettingDefinition[] = clean.fields;

  const sectionMap = new Map(sections.map((section) => [section.id, section]));
  const sectionsByPage = groupBy(sections, (section) => section.pageId);
  const fieldsByGroup = groupBy(fields, (field) => field.group);
  const fieldsBySection = groupBy(fields, (field) => field.section);

  const confPageMap = new Map<string, SettingsPageId>();
  for (const slot of clean.confs) {
    if (slot.pageId) confPageMap.set(slot.group, slot.pageId);
  }

  return Object.freeze({
    version,
    phase: bound ? "bound" : "builtin",
    pages: Object.freeze(pages),
    pageMap,
    categories: Object.freeze(categories),
    categoryMap,
    sections: Object.freeze(sections),
    sectionMap,
    sectionsByPage,
    fields: Object.freeze(fields),
    fieldsByGroup,
    fieldsBySection,
    routes: Object.freeze([...clean.routes]),
    sidebar: Object.freeze([...clean.sidebar]),
    confPageMap,
  });
}

export function getSettingsSnapshot(): SettingsSnapshot {
  assertBound();
  queried = true;

  const key = `${getBuiltinVersion()}:${extensionInput.id}`;
  if (cache && cache.key === key) return cache.value;

  const value = buildSnapshot(getBuiltinVersion(), extensionInput);
  cache = { key, value };
  return value;
}
