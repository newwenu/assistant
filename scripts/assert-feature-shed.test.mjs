import { describe, expect, it } from "vitest";
import {
  disabledDepCrates,
  featureClosure,
  normalEdgeReachable,
  parseCargoFeatures,
} from "./assert-feature-shed.mjs";

const CARGO_TEXT = `[package]
name = "demo"

[features]
default = ["alpha"]
alpha = ["dep:alpha-core"]
beta = ["dep:beta-core", "shared"]

[dependencies]
alpha-core = { path = "crates/alpha", optional = true }
beta-core = { path = "crates/beta", optional = true }
`;

function syntheticMetadata() {
  const packages = [
    { id: "root", name: "assistant-app" },
    { id: "shared", name: "shared" },
    { id: "dev-only", name: "dev-only" },
    { id: "beta-core", name: "beta-core" },
  ];
  return {
    packages,
    resolve: {
      nodes: [
        {
          id: "root",
          deps: [
            {
              name: "shared",
              pkg: "shared",
              dep_kinds: [{ kind: null, target: null }],
            },
            {
              name: "dev-only",
              pkg: "dev-only",
              dep_kinds: [{ kind: "dev", target: null }],
            },
          ],
        },
        { id: "shared", deps: [] },
        { id: "dev-only", deps: [] },
        { id: "beta-core", deps: [] },
      ],
    },
  };
}

describe("assert-feature-shed", () => {
  it("解析 [features] 段为 feature -> 依赖项", () => {
    const features = parseCargoFeatures(CARGO_TEXT);
    expect(features.get("default")).toEqual(["alpha"]);
    expect(features.get("alpha")).toEqual(["dep:alpha-core"]);
    expect(features.get("beta")).toEqual(["dep:beta-core", "shared"]);
  });

  it("特性闭包展开特性引用，忽略 dep: 与 crate/feature 形式", () => {
    const features = parseCargoFeatures(CARGO_TEXT);
    expect([...featureClosure(features, ["default"])].sort()).toEqual([
      "alpha",
      "default",
    ]);
    expect([...featureClosure(features, ["beta"])].sort()).toEqual([
      "beta",
      "shared",
    ]);
  });

  it("禁用集合是清单中未被启用闭包覆盖的全部特性", () => {
    const features = parseCargoFeatures(CARGO_TEXT);
    const result = disabledDepCrates(
      features,
      ["alpha", "beta", "shared"],
      ["alpha"],
    );
    expect(result.disabled).toEqual(["beta", "shared"]);
    expect(result.crates).toEqual(["beta-core"]);
  });

  it("被启用侧引用的共享特性不算禁用，其 dep: crate 不计入泄漏", () => {
    const features = parseCargoFeatures(CARGO_TEXT);
    const result = disabledDepCrates(
      features,
      ["alpha", "beta", "shared"],
      ["alpha", "shared"],
    );
    expect(result.disabled).toEqual(["beta"]);
    expect(result.crates).toEqual(["beta-core"]);
  });

  it("normal-edge 可达集合排除 dev 依赖", () => {
    const reachable = normalEdgeReachable(syntheticMetadata(), "assistant-app");
    expect([...reachable].sort()).toEqual(["assistant-app", "shared"]);
  });

  it("normal-edge 可达集合归一化下划线", () => {
    const reachable = normalEdgeReachable(syntheticMetadata(), "assistant-app");
    expect(reachable.has("beta-core")).toBe(false);
  });
});
