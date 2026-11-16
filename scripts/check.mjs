#!/usr/bin/env node
import { spawn, spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const BIN = path.join(ROOT, "node_modules", ".bin");
const IS_WIN = process.platform === "win32";
const full = process.argv.includes("--full");
const bin = (name) => path.join(BIN, IS_WIN ? `${name}.cmd` : name);

const CONTRACT_INPUTS = [
  "src-tauri/",
  "src/types/protocol.contract.ts",
  "src/types/shortcutDefaults.generated.ts",
  "src/vault/core/contract/",
];

function exec(cmdline) {
  return new Promise((resolve) => {
    const started = Date.now();
    const child = spawn(cmdline, { cwd: ROOT, shell: true });
    let out = "";
    child.stdout.on("data", (chunk) => (out += chunk));
    child.stderr.on("data", (chunk) => (out += chunk));
    child.on("close", (code) => {
      resolve({ ok: code === 0, code, out, ms: Date.now() - started });
    });
  });
}

async function runChain(commands) {
  let out = "";
  let ms = 0;
  for (const cmdline of commands) {
    const result = await exec(cmdline);
    out += result.out;
    ms += result.ms;
    if (!result.ok) return { ok: false, code: result.code, out, ms };
  }
  return { ok: true, out, ms };
}

function changedPaths() {
  const result = spawnSync("git status --porcelain", {
    cwd: ROOT,
    encoding: "utf8",
    shell: true,
  });
  if (result.status !== 0) return null;
  return result.stdout
    .split("\n")
    .map((line) => line.slice(3).trim())
    .filter(Boolean);
}

function contractNeeded() {
  if (full) return true;
  const paths = changedPaths();
  if (paths === null) return true;
  return paths.some((file) =>
    CONTRACT_INPUTS.some((prefix) => file.startsWith(prefix)),
  );
}

const PHASE1 = [
  {
    id: "format",
    label: "格式",
    commands: [
      `"${bin("prettier")}" --check src scripts vite.config.ts vitest.config.ts eslint.config.ts`,
    ],
  },
  {
    id: "types",
    label: "类型",
    commands: [
      `"${bin("tsc")}" -p tsconfig.json --noEmit`,
      `"${bin("tsc")}" -b tsconfig.node.json`,
    ],
  },
  {
    id: "lint",
    label: "规则",
    commands: [
      `"${bin("eslint")}" . --max-warnings 0 --cache --cache-location node_modules/.cache/eslint`,
    ],
  },
];

const PHASE2 = [
  {
    id: "contract",
    label: "契约",
    commands: [`"${process.execPath}" scripts/contract-check.mjs`],
  },
];

function report(results) {
  for (const { step, result } of results) {
    const status = result.ok ? "ok" : "FAIL";
    console.log(
      `[check] ${step.label} ${status} ${(result.ms / 1000).toFixed(1)}s`,
    );
  }
  const failed = results.filter(({ result }) => !result.ok);
  for (const { step, result } of failed) {
    console.log(`\n----- ${step.label} 输出 -----`);
    const trimmed = result.out.trim();
    console.log(trimmed.length > 0 ? trimmed : `(exit ${result.code})`);
  }
  return failed.length === 0;
}

async function runGroup(steps) {
  return Promise.all(
    steps.map(async (step) => ({
      step,
      result: await runChain(step.commands),
    })),
  );
}

const phase1 = await runGroup(PHASE1);
if (!report(phase1)) {
  console.log("\n[check] 前置检查未通过，跳过契约校验");
  process.exit(1);
}

if (!contractNeeded()) {
  console.log("[check] 契约：Rust 侧与生成物未改动，跳过（--full 可强制）");
  process.exit(0);
}

process.exit(report(await runGroup(PHASE2)) ? 0 : 1);
