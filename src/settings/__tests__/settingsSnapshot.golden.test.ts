import { describe, expect, it } from "vitest";
import "@/settings/core/field/fieldDefinitions";
import { aggregate } from "@/features/aggregate";
import { vaultModule } from "@/features/vault/module";
import {
  bindExtensionInput,
  getSettingsSnapshot,
} from "@/settings/core/snapshot";
import { EMPTY_EXTENSION_INPUT } from "@/settings/core/types/snapshot";
import type { SettingsSnapshot } from "@/settings/core/types/snapshot";
import {
  getAllDefinitions,
  getAllSections,
  getDefinitionsByGroup,
  getDefinitionsBySection,
  getSectionById,
  getSectionsByPage,
} from "@/settings/core/query";
import { defineSection } from "@/settings/core/builtinRegistry";

interface ProjectedCollections {
  pages: {
    id: string;
    categoryId: string;
    order: number;
    visibility: string;
    navParent: string | null;
    emptyPage: boolean;
  }[];
  categories: { id: string; order: number }[];
  sections: { id: string; pageId: string }[];
  fields: {
    key: string;
    group: string;
    section: string | null;
    type: string;
  }[];
  routes: {
    path: string;
    boundary: string;
    order: number;
    visibility: string;
    navParent: string | null;
  }[];
  sidebar: { key: string; slot: string; order: number }[];
  confPageMap: [string, string][];
}

interface ProjectedSnapshot extends ProjectedCollections {
  counts: Record<keyof ProjectedCollections, number>;
}

function withCounts(collections: ProjectedCollections): ProjectedSnapshot {
  return {
    counts: {
      pages: collections.pages.length,
      categories: collections.categories.length,
      sections: collections.sections.length,
      fields: collections.fields.length,
      routes: collections.routes.length,
      sidebar: collections.sidebar.length,
      confPageMap: collections.confPageMap.length,
    },
    ...collections,
  };
}

function project(snapshot: SettingsSnapshot): ProjectedSnapshot {
  return withCounts({
    pages: snapshot.pages.map((page) => ({
      id: page.id,
      categoryId: page.categoryId,
      order: page.order,
      visibility: page.visibility,
      navParent: page.navParent ?? null,
      emptyPage: page.emptyPage,
    })),
    categories: snapshot.categories.map((category) => ({
      id: category.id,
      order: category.order,
    })),
    sections: snapshot.sections.map((section) => ({
      id: section.id,
      pageId: section.pageId,
    })),
    fields: snapshot.fields.map((field) => ({
      key: field.key,
      group: field.group,
      section: field.section ?? null,
      type: field.type,
    })),
    routes: snapshot.routes.map((route) => ({
      path: route.path,
      boundary: route.boundary,
      order: route.order,
      visibility: route.visibility,
      navParent: route.navParent ?? null,
    })),
    sidebar: snapshot.sidebar.map((entry) => ({
      key: entry.key,
      slot: entry.slot,
      order: entry.order,
    })),
    confPageMap: [...snapshot.confPageMap].sort((a, b) =>
      a[0].localeCompare(b[0]),
    ),
  });
}

function difference<T>(
  full: readonly T[],
  base: readonly T[],
  keyOf: (item: T) => string,
): T[] {
  const baseKeys = new Set(base.map(keyOf));
  return full.filter((item) => !baseKeys.has(keyOf(item)));
}

function diffProjection(
  full: ProjectedSnapshot,
  base: ProjectedSnapshot,
): ProjectedSnapshot {
  return withCounts({
    pages: difference(full.pages, base.pages, (page) => page.id),
    categories: difference(
      full.categories,
      base.categories,
      (category) => category.id,
    ),
    sections: difference(full.sections, base.sections, (section) => section.id),
    fields: difference(full.fields, base.fields, (field) => field.key),
    routes: difference(full.routes, base.routes, (route) => route.path),
    sidebar: difference(full.sidebar, base.sidebar, (entry) => entry.key),
    confPageMap: difference(
      full.confPageMap,
      base.confPageMap,
      (entry) => entry[0],
    ),
  });
}

function serialize(value: unknown): string {
  return `${JSON.stringify(value, null, 2)}\n`;
}

describe("settings snapshot golden", () => {
  it("vaultless projection matches the golden fixture", async () => {
    bindExtensionInput(EMPTY_EXTENSION_INPUT);
    const vaultless = project(getSettingsSnapshot());
    await expect(serialize(vaultless)).toMatchFileSnapshot(
      "./fixtures/snapshot.vaultless.json",
    );
  });

  it("full projection matches the golden fixture", async () => {
    bindExtensionInput(EMPTY_EXTENSION_INPUT);
    getSettingsSnapshot();
    const aggregated = aggregate([vaultModule]);
    bindExtensionInput({ id: "vault", ...aggregated });
    const full = project(getSettingsSnapshot());
    await expect(serialize(full)).toMatchFileSnapshot(
      "./fixtures/snapshot.full.json",
    );
  });

  it("vault diff equals exactly what vaultModule contributes", async () => {
    bindExtensionInput(EMPTY_EXTENSION_INPUT);
    const vaultless = project(getSettingsSnapshot());
    const aggregated = aggregate([vaultModule]);
    bindExtensionInput({ id: "vault", ...aggregated });
    const full = project(getSettingsSnapshot());
    const diff = diffProjection(full, vaultless);

    await expect(serialize(diff)).toMatchFileSnapshot(
      "./fixtures/snapshot.diff.json",
    );

    expect(diff.pages.map((page) => page.id)).toEqual(
      (vaultModule.settings?.pages ?? []).map((page) => page.id),
    );
    expect(diff.sections.map((section) => section.id)).toEqual(
      (vaultModule.settings?.sections ?? []).map((section) => section.id),
    );
    expect(diff.fields.map((field) => field.key)).toEqual(
      (vaultModule.settings?.fields ?? []).map((field) => field.key),
    );
    expect(diff.routes.map((route) => route.path)).toEqual(
      (vaultModule.routes ?? []).map((route) => route.path),
    );
    expect(diff.categories.map((category) => category.id)).toEqual(
      (vaultModule.categories ?? []).map((category) => category.id),
    );
    expect(diff.sidebar).toHaveLength((vaultModule.sidebar ?? []).length);
  });

  it("getAllDefinitions returns the same reference across calls", () => {
    const first = getAllDefinitions();
    const second = getAllDefinitions();
    expect(first).toBe(second);
  });

  it("all six queries read the same snapshot", () => {
    const snapshot = getSettingsSnapshot();
    expect(getAllDefinitions()).toBe(snapshot.fields);
    expect(getAllSections()).toBe(snapshot.sections);
    expect(getDefinitionsByGroup("vault")).toBe(
      snapshot.fieldsByGroup.get("vault"),
    );
    expect(getDefinitionsBySection("vault-general")).toBe(
      snapshot.fieldsBySection.get("vault-general"),
    );
    expect(getSectionsByPage("vault")).toBe(
      snapshot.sectionsByPage.get("vault"),
    );
    expect(getSectionById("vault-ssh")).toBe(
      snapshot.sectionMap.get("vault-ssh"),
    );
  });

  it("snapshot rebuilds after a builtin version bump", () => {
    const before = getSettingsSnapshot();
    expect(before.phase).toBe("bound");

    defineSection({
      id: "golden-probe",
      pageId: "general",
      title: "golden.probe.title",
    });

    const after = getSettingsSnapshot();
    expect(after).not.toBe(before);
    expect(after.version).toBe(before.version + 1);
    expect(after.sectionMap.get("golden-probe")).toBeDefined();
  });
});
