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

interface ProjectedSnapshot {
  pages: {
    id: string;
    categoryId: string;
    label: string;
    order: number;
    keywords: string[];
    navParent: string | null;
    visibility: string;
    emptyPage: boolean;
  }[];
  categories: {
    id: string;
    label: string;
    order: number;
    pages: string[];
  }[];
  sections: {
    id: string;
    pageId: string;
    title: string;
    description: string | null;
    keywords: string[] | null;
  }[];
  fields: {
    key: string;
    group: string;
    section: string | null;
    type: string;
    default: unknown;
    label: string | null;
  }[];
  routes: {
    path: string;
    boundary: string;
    order: number;
    navParent: string | null;
    visibility: string;
    keywords: string[];
  }[];
  sidebar: { key: string; label: string; slot: string; order: number }[];
  confPageMap: [string, string][];
}

function project(snapshot: SettingsSnapshot): ProjectedSnapshot {
  return {
    pages: snapshot.pages.map((page) => ({
      id: page.id,
      categoryId: page.categoryId,
      label: page.label,
      order: page.order,
      keywords: [...page.keywords],
      navParent: page.navParent ?? null,
      visibility: page.visibility,
      emptyPage: page.emptyPage,
    })),
    categories: snapshot.categories.map((category) => ({
      id: category.id,
      label: category.label,
      order: category.order,
      pages: category.pages.map((page) => page.id),
    })),
    sections: snapshot.sections.map((section) => ({
      id: section.id,
      pageId: section.pageId,
      title: section.title,
      description: section.description ?? null,
      keywords: section.keywords ? [...section.keywords] : null,
    })),
    fields: snapshot.fields.map((field) => ({
      key: field.key,
      group: field.group,
      section: field.section ?? null,
      type: field.type,
      default: field.default ?? null,
      label: field.label ?? null,
    })),
    routes: snapshot.routes.map((route) => ({
      path: route.path,
      boundary: route.boundary,
      order: route.order,
      navParent: route.navParent ?? null,
      visibility: route.visibility,
      keywords: [...route.keywords],
    })),
    sidebar: snapshot.sidebar.map((entry) => ({
      key: entry.key,
      label: entry.label,
      slot: entry.slot,
      order: entry.order,
    })),
    confPageMap: [...snapshot.confPageMap].sort((a, b) =>
      a[0].localeCompare(b[0]),
    ),
  };
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
  return {
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
  };
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
