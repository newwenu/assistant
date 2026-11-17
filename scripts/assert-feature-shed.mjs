#!/usr/bin/env node
// CI-2：关闭态依赖图断言
// 禁用特性的 `dep:` crate 不得出现在 normal-edge 依赖图里，
// 否则「关闭」只是不注册，依赖仍被编进二进制。
// 用法: node scripts/assert-feature-shed.mjs [--features vault] [--package assistant-app]

import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const FEATURE_MANIFEST = path.join(REPO_ROOT, "config/features.json");
const SRC_TAURI = path.join(REPO_ROOT, "src-tauri");
const CARGO_MANIFEST = path.join(SRC_TAURI, "Cargo.toml");

function fail(message) {
  console.error(`[assert-feature-shed] ${message}`);
  process.exit(1);
}

function parseArgs(argv) {
  const options = { features: [], package: "assistant-app" };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2);
    const value = argv[index + 1];
    if (value === undefined || value.startsWith("--")) fail(`${arg} 缺少取值`);
    if (key === "features") {
      options.features = value
        .split(",")
        .map((item) => item.trim())
        .filter(Boolean);
    } else if (key === "package") {
      options.package = value;
    } else {
      fail(`未知参数 ${arg}`);
    }
    index += 1;
  }
  return options;
}

function readManifestIds() {
  const manifest = JSON.parse(fs.readFileSync(FEATURE_MANIFEST, "utf8"));
  const ids = Object.keys(manifest);
  if (ids.length === 0) fail("config/features.json 未声明任何特性");
  return ids;
}

export function parseCargoFeatures(text) {
  const lines = text.split(/\r?\n/);
  const start = lines.findIndex((line) => line.trim() === "[features]");
  if (start < 0) fail("src-tauri/Cargo.toml 缺少 [features] 段");
  const section = [];
  for (let index = start + 1; index < lines.length; index += 1) {
    if (/^\s*\[/.test(lines[index])) break;
    section.push(lines[index]);
  }
  const joined = section.join("\n");
  const features = new Map();
  const assignment = /([A-Za-z0-9_.-]+)\s*=\s*\[([^\]]*)\]/g;
  for (const match of joined.matchAll(assignment)) {
    const literals = [...match[2].matchAll(/"([^"]*)"/g)].map((m) => m[1]);
    features.set(match[1], literals);
  }
  if (features.size === 0) fail("[features] 段解析结果为 0 条，断言失去基准");
  return features;
}

function depCrateOf(item) {
  if (item.startsWith("dep:")) return item.slice(4);
  const slash = item.indexOf("/");
  if (slash <= 0) return null;
  const crate = item.slice(0, slash);
  return crate.endsWith("?") ? crate.slice(0, -1) : crate;
}

function featureRefOf(item) {
  if (item.startsWith("dep:")) return null;
  if (item.includes("/")) return null;
  return item;
}

export function featureClosure(features, seeds) {
  const seen = new Set();
  const stack = [...seeds];
  while (stack.length > 0) {
    const name = stack.pop();
    if (seen.has(name)) continue;
    seen.add(name);
    const items = features.get(name);
    if (!items) continue;
    for (const item of items) {
      const ref = featureRefOf(item);
      if (ref !== null && !seen.has(ref)) stack.push(ref);
    }
  }
  return seen;
}

function depCratesOf(features, closure) {
  const crates = new Set();
  for (const name of closure) {
    const items = features.get(name);
    if (!items) continue;
    for (const item of items) {
      const crate = depCrateOf(item);
      if (crate !== null) crates.add(crate);
    }
  }
  return crates;
}

export function disabledDepCrates(features, manifestIds, enabledSeeds) {
  const enabledClosure = featureClosure(features, enabledSeeds);
  const enabledCrates = depCratesOf(features, enabledClosure);
  const disabled = manifestIds.filter((id) => !enabledClosure.has(id));
  if (disabled.length === 0) return { disabled, crates: [] };
  const disabledClosure = featureClosure(features, disabled);
  const disabledCrates = depCratesOf(features, disabledClosure);
  const crates = [...disabledCrates].filter(
    (crate) => !enabledCrates.has(crate),
  );
  return { disabled, crates };
}

function normalizeCrateName(name) {
  return name.replace(/_/g, "-").toLowerCase();
}

export function normalEdgeReachable(metadata, rootPackage) {
  const nameById = new Map(metadata.packages.map((pkg) => [pkg.id, pkg.name]));
  const nodes = metadata.resolve?.nodes ?? [];
  if (nodes.length === 0) fail("cargo metadata 的 resolve 图为空");
  const root = nodes.find((node) => nameById.get(node.id) === rootPackage);
  if (!root) fail(`resolve 图中找不到 package ${rootPackage}`);
  const reachable = new Set();
  const stack = [root];
  while (stack.length > 0) {
    const node = stack.pop();
    if (reachable.has(node.id)) continue;
    reachable.add(node.id);
    for (const dep of node.deps ?? []) {
      const kinds = dep.dep_kinds ?? [];
      const isNormal = kinds.some(
        (kind) => kind.kind === null || kind.kind === "normal",
      );
      if (!isNormal) continue;
      const next = nodes.find((item) => item.id === dep.pkg);
      if (next && !reachable.has(next.id)) stack.push(next);
    }
  }
  const names = new Set();
  for (const id of reachable) {
    const name = nameById.get(id);
    if (name) names.add(normalizeCrateName(name));
  }
  if (names.size === 0) fail("normal-edge 可达集合为空，断言失去基准");
  return names;
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const manifestIds = readManifestIds();
  const cargoText = fs.readFileSync(CARGO_MANIFEST, "utf8");
  const features = parseCargoFeatures(cargoText);
  for (const id of options.features) {
    if (!manifestIds.includes(id)) fail(`--features 中的 ${id} 不在特性清单`);
  }
  const { disabled, crates } = disabledDepCrates(
    features,
    manifestIds,
    options.features,
  );

  if (disabled.length === 0) {
    console.log(
      "[assert-feature-shed] 未关闭任何清单特性，依赖断开断言无可检查对象，按失败处理",
    );
    process.exit(2);
  }
  if (crates.length === 0) {
    console.log(
      `[assert-feature-shed] 禁用特性 ${disabled.join(", ")} 未解析出任何独占 dep: crate，按失败处理`,
    );
    process.exit(2);
  }

  const raw = execFileSync(
    "cargo",
    [
      "metadata",
      "--format-version",
      "1",
      "--no-default-features",
      ...(options.features.length > 0
        ? ["--features", options.features.join(",")]
        : []),
    ],
    { cwd: SRC_TAURI, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
  );
  const metadata = JSON.parse(raw);
  const reachable = normalEdgeReachable(metadata, options.package);

  const leaked = crates.filter((crate) =>
    reachable.has(normalizeCrateName(crate)),
  );
  if (leaked.length > 0) {
    fail(
      `禁用特性 ${disabled.join(", ")} 的依赖仍在 normal-edge 图: ${leaked.join(", ")}`,
    );
  }
  console.log(
    `[assert-feature-shed] 已关闭 ${disabled.join(", ")}；${crates.length} 个独占 dep: crate 均不在 normal-edge 图（可达 ${reachable.size} 个包）`,
  );
}

if (path.basename(process.argv[1] ?? "") === "assert-feature-shed.mjs") {
  main();
}
