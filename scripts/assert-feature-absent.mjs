#!/usr/bin/env node
// 关闭态产物探针：断言被关闭的特性没有 chunk 泄漏到 dist/assets
// 用法: node scripts/assert-feature-absent.mjs --feature vault --dist dist [--prefix settings]
// 探针源根为候选集：src/features/<id>/ 与 src/tools/<id>/（只扫存在的目录并合并）。

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

function fail(message) {
  console.error(`[assert-feature-absent] ${message}`);
  process.exit(1);
}

function parseArgs(argv) {
  const options = { feature: null, dist: "dist", prefix: "settings" };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2);
    const value = argv[index + 1];
    if (!value || value.startsWith("--")) fail(`${arg} 缺少取值`);
    if (!Object.hasOwn(options, key)) fail(`未知参数 ${arg}`);
    options[key] = value;
    index += 1;
  }
  if (!options.feature) fail("必须提供 --feature <id>");
  return options;
}

function collectSources(dir) {
  if (!fs.existsSync(dir))
    fail(`特性源码目录不存在: ${path.relative(REPO_ROOT, dir)}`);
  const out = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const child = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === "__tests__") continue;
      out.push(...collectSources(child));
    } else if (/\.(ts|tsx)$/.test(entry.name)) {
      out.push(child);
    }
  }
  return out;
}

function probeLiterals(options) {
  const candidateRoots = [
    path.join(REPO_ROOT, "src", "features", options.feature),
    path.join(REPO_ROOT, "src", "tools", options.feature),
  ].filter((root) => fs.existsSync(root));
  const needle = `${options.prefix}.${options.feature}.`;
  const pattern = new RegExp(
    `["'\`](${needle.replace(/\./g, "\\.")}[A-Za-z0-9_.\\-]+)["'\`]`,
    "g",
  );

  const probes = new Set();
  for (const root of candidateRoots) {
    for (const file of collectSources(root)) {
      const text = fs.readFileSync(file, "utf8");
      for (const match of text.matchAll(pattern)) {
        probes.add(match[1]);
      }
    }
  }
  const relative = (root) =>
    path.relative(REPO_ROOT, root).split(path.sep).join("/");
  return { probes, roots: candidateRoots.map(relative) };
}

function assetFiles(distDir) {
  const assets = path.join(distDir, "assets");
  if (!fs.existsSync(assets)) {
    fail(
      `产物目录不存在: ${path.relative(REPO_ROOT, assets)}，请先执行关闭态构建`,
    );
  }
  return fs
    .readdirSync(assets)
    .filter((name) => name.endsWith(".js"))
    .map((name) => path.join(assets, name));
}

const options = parseArgs(process.argv.slice(2));
const { probes, roots } = probeLiterals(options);
if (roots.length === 0) {
  console.error(
    `[assert-feature-absent] 特性源码目录不存在: src/features/${options.feature}/ 与 src/tools/${options.feature}/`,
  );
  process.exit(2);
}
if (probes.size === 0) {
  console.error(
    `[assert-feature-absent] 未在 ${roots.join(" + ")} 提取到任何 ${options.prefix}.${options.feature}. 探针字面量，` +
      "空扫描不能视为通过（特性文案可能被搬到 src/locales/）",
  );
  process.exit(2);
}

const assets = assetFiles(path.resolve(REPO_ROOT, options.dist));
if (assets.length === 0) fail("dist/assets 内没有任何 .js 产物");

const leaks = [];
for (const asset of assets) {
  const text = fs.readFileSync(asset, "utf8");
  const hit = Array.from(probes).filter((probe) => text.includes(probe));
  if (hit.length > 0) {
    leaks.push(`${path.relative(REPO_ROOT, asset)} -> ${hit.join(", ")}`);
  }
}

if (leaks.length > 0) {
  fail(
    `特性 ${options.feature} 已关闭但产物仍含专有字符串:\n${leaks.join("\n")}`,
  );
}

console.log(
  `[assert-feature-absent] ${options.feature}: ${assets.length} 个产物、${probes.size} 条探针（源根 ${roots.join(" + ")}），无泄漏`,
);
