import { describe, expect, it } from "vitest";
import { readRepoFile } from "@/test-utils/paths";
import { getBuiltinConfSlots } from "@/settings/core/registry/builtinRegistry";
import {
  WINDOW_CONF_REQUIREMENTS,
  WINDOW_LABELS,
} from "@/settings/core/runtime/windowRequirements.generated";

const STATIC_VIEW_IMPORT =
  /^import\s+[^;]*from\s+["']@\/layouts\/(?:MainLayout|OrbWindow)["']/m;

describe("startup boundary", () => {
  it("App.tsx loads window views through dynamic import only", () => {
    const source = readRepoFile("src/App.tsx");
    expect(source).not.toMatch(STATIC_VIEW_IMPORT);
    expect(source).toContain('import("@/layouts/MainLayout")');
    expect(source).toContain('import("@/layouts/OrbWindow")');
  });

  it("main.tsx keeps loadFeatures after render", () => {
    const source = readRepoFile("src/main.tsx");
    const renderAt = source.indexOf(".render(");
    const loadFeaturesAt = source.indexOf("void loadFeatures();");
    expect(renderAt).toBeGreaterThan(-1);
    expect(loadFeaturesAt).toBeGreaterThan(renderAt);
  });

  it("main.tsx render-before chain has no feature loading and no full conf loop", () => {
    const source = readRepoFile("src/main.tsx");
    const bootstrapAt = source.indexOf("async function bootstrap");
    const renderAt = source.indexOf(".render(");
    expect(bootstrapAt).toBeGreaterThan(-1);
    const chain = source.slice(bootstrapAt, renderAt);
    expect(chain).not.toContain("loadFeatures");
    expect(chain).not.toContain("getConfGroups");
    expect(chain).toContain("await useThemeStore.getState().initLocal()");
  });

  it("G6: bootstrap reads first-frame groups from the generated window requirements", () => {
    const source = readRepoFile("src/main.tsx");
    const bootstrapAt = source.indexOf("async function bootstrap");
    const renderAt = source.indexOf(".render(");
    expect(bootstrapAt).toBeGreaterThan(-1);
    const chain = source.slice(bootstrapAt, renderAt);
    expect(chain).toContain("WINDOW_CONF_REQUIREMENTS");
    expect(source).not.toContain("ORB_FIRST_FRAME_GROUPS");
    expect(source).not.toContain("MAIN_FIRST_FRAME_GROUPS");
  });

  it("G6: every generated window conf requirement has a registered conf slot", () => {
    const registered = new Set(getBuiltinConfSlots().map((slot) => slot.group));
    const missing: string[] = [];
    for (const label of WINDOW_LABELS) {
      for (const group of WINDOW_CONF_REQUIREMENTS[label]) {
        if (!registered.has(group)) missing.push(`${label} -> ${group}`);
      }
    }
    expect(missing).toEqual([]);
  });
});
