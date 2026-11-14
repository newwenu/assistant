#!/usr/bin/env node
// 契约生成物同步校验：取当前工作区生成物哈希，跑一次 `contract:gen` 后比对
// 用法: node scripts/contract-check.mjs
//
// 判据与 git 状态无关：源码未变 → 生成结果与现有生成物一致 → 通过；
// 源码变了但没重新生成 → 哈希不同 → 失败。

import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

const TARGETS = [
  "src/types/protocol.contract.ts",
  "src/types/shortcutDefaults.generated.ts",
  "src/vault/core/contract",
];

function fail(message) {
  console.error(`[contract-check] ${message}`);
  process.exit(1);
}

function collectFiles(target) {
  const absolute = path.join(REPO_ROOT, target);
  if (!fs.existsSync(absolute)) return [target];
  if (!fs.statSync(absolute).isDirectory()) return [target];

  const out = [];
  for (const entry of fs.readdirSync(absolute, { withFileTypes: true })) {
    const child = path.posix.join(target, entry.name);
    if (entry.isDirectory()) out.push(...collectFiles(child));
    else if (entry.isFile()) out.push(child);
  }
  return out.sort();
}

function snapshot() {
  const hashes = new Map();
  for (const file of TARGETS.flatMap((target) => collectFiles(target))) {
    const absolute = path.join(REPO_ROOT, file);
    if (!fs.existsSync(absolute)) {
      hashes.set(file, "missing");
      continue;
    }
    const digest = crypto
      .createHash("sha256")
      .update(fs.readFileSync(absolute))
      .digest("hex");
    hashes.set(file, digest);
  }
  return hashes;
}

function runGen() {
  const result = spawnSync("npm", ["run", "contract:gen"], {
    cwd: REPO_ROOT,
    stdio: "inherit",
    shell: true,
  });
  if (result.status !== 0) {
    fail(`contract:gen 失败（exit ${result.status ?? "unknown"}）`);
  }
}

const baseline = snapshot();
const missing = [...baseline].filter(([, hash]) => hash === "missing");
if (missing.length > 0) {
  for (const [file] of missing) console.error(`  ${file}: 不存在`);
  fail(`生成物缺失，先运行 npm run contract:gen 并提交生成物`);
}

runGen();
const after = snapshot();

const files = new Set([...baseline.keys(), ...after.keys()]);
const mismatched = [...files].filter(
  (file) => baseline.get(file) !== after.get(file),
);

if (mismatched.length > 0) {
  for (const file of mismatched) {
    console.error(`  ${file}: ${baseline.get(file)} -> ${after.get(file)}`);
  }
  fail(
    `生成物与源码不同步：${mismatched.length} 个文件在 contract:gen 后发生变化`,
  );
}

console.log(
  `[contract-check] ${files.size} 个生成物与源码一致（无需重新生成）`,
);
