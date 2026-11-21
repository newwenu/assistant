import { describe, expect, it } from "vitest";
import {
  featureBundleGuard,
  findFeatureModules,
} from "./feature-bundle-guard.mjs";

function syntheticBundle(moduleIds) {
  return {
    "assets/index.js": {
      type: "chunk",
      moduleIds,
      modules: Object.fromEntries(moduleIds.map((id) => [id, { code: "" }])),
    },
  };
}

describe("feature-bundle-guard", () => {
  it("产物含特性模块时报告违规", () => {
    expect(
      findFeatureModules(
        ["src/App.tsx", "src/features/alpha/module.ts"],
        ["alpha"],
      ),
    ).toEqual([{ feature: "alpha", module: "src/features/alpha/module.ts" }]);
  });

  it("产物不含特性模块时判通过", () => {
    expect(
      findFeatureModules(
        ["src/App.tsx", "src/features/registry.ts"],
        ["alpha"],
      ),
    ).toEqual([]);
  });

  it("归一化路径分隔符与查询串", () => {
    expect(
      findFeatureModules(
        ["\\repo\\src\\features\\alpha\\module.ts"],
        ["alpha"],
      ),
    ).toEqual([
      { feature: "alpha", module: "/repo/src/features/alpha/module.ts" },
    ]);
    expect(
      findFeatureModules(["src/features/alpha/module.ts?worker"], ["alpha"]),
    ).toEqual([{ feature: "alpha", module: "src/features/alpha/module.ts" }]);
  });

  it("同名前缀不误报", () => {
    expect(
      findFeatureModules(["src/features/alphabet/index.ts"], ["alpha"]),
    ).toEqual([]);
  });

  it("合成 chunk 列表：含特性模块的关闭态产物构建失败", () => {
    const plugin = featureBundleGuard(["alpha"]);
    expect(() =>
      plugin.generateBundle(
        {},
        syntheticBundle(["src/features/alpha/module.ts"]),
      ),
    ).toThrow(/仍含特性模块/);
  });

  it("合成 chunk 列表：不含特性模块的关闭态产物通过", () => {
    const plugin = featureBundleGuard(["alpha"]);
    expect(() =>
      plugin.generateBundle({}, syntheticBundle(["src/App.tsx"])),
    ).not.toThrow();
  });

  it("没有 chunk 或模块时失败", () => {
    const plugin = featureBundleGuard(["alpha"]);
    expect(() => plugin.generateBundle({}, {})).toThrow(/没有 chunk/);
    expect(() => plugin.generateBundle({}, syntheticBundle([]))).toThrow(
      /未解析到任何模块/,
    );
  });

  it("候选根包含 src/tools/ 时报告工具实现模块", () => {
    expect(
      findFeatureModules(
        ["src/App.tsx", "src/tools/alpha/ClockLayout.tsx"],
        ["alpha"],
        ["src/features/", "src/tools/"],
      ),
    ).toEqual([
      { feature: "alpha", module: "src/tools/alpha/ClockLayout.tsx" },
    ]);
    const plugin = featureBundleGuard(
      ["alpha"],
      ["src/features/", "src/tools/"],
    );
    expect(() =>
      plugin.generateBundle(
        {},
        syntheticBundle(["src/tools/alpha/ClockLayout.tsx"]),
      ),
    ).toThrow(/仍含特性模块/);
  });
});
