import { describe, it, expect, beforeEach } from "vitest";

describe("settingRegistry", () => {
  beforeEach(async () => {
    const mod = await import("./settingRegistry");
    const defs = mod.getAllDefinitions();
    const registry = mod as unknown as { definitions: unknown[] };
    registry.definitions = [];
  });

  it("defineSetting registers and returns definition", async () => {
    const { defineSetting, getDefinitionsByGroup } = await import(
      "./settingRegistry"
    );
    const def = defineSetting({
      key: "test_bool",
      group: "app",
      type: "boolean",
      default: false,
      label: "Test Bool",
    });
    expect(def.key).toBe("test_bool");
    const appDefs = getDefinitionsByGroup("app");
    expect(appDefs.some((d) => d.key === "test_bool")).toBe(true);
  });

  it("getDefinitionsByGroup filters by group", async () => {
    const { defineSetting, getDefinitionsByGroup } = await import(
      "./settingRegistry"
    );
    defineSetting({
      key: "a1",
      group: "app",
      type: "boolean",
      default: false,
      label: "A1",
    });
    defineSetting({
      key: "o1",
      group: "orb",
      type: "boolean",
      default: false,
      label: "O1",
    });
    expect(getDefinitionsByGroup("app").length).toBeGreaterThanOrEqual(1);
    expect(getDefinitionsByGroup("orb").length).toBeGreaterThanOrEqual(1);
    expect(
      getDefinitionsByGroup("app").every((d) => d.group === "app"),
    ).toBe(true);
  });

  it("registerSettingType and getSettingTypeRenderer", async () => {
    const { registerSettingType, getSettingTypeRenderer } = await import(
      "./settingRegistry"
    );
    const mockComponent = () => null;
    registerSettingType("custom_type", { component: mockComponent });
    const renderer = getSettingTypeRenderer("custom_type");
    expect(renderer).toBeDefined();
    expect(renderer!.component).toBe(mockComponent);
  });

  it("getSettingTypeRenderer returns undefined for unregistered type", async () => {
    const { getSettingTypeRenderer } = await import("./settingRegistry");
    expect(getSettingTypeRenderer("nonexistent")).toBeUndefined();
  });
});