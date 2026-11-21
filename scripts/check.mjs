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
    fix: [
      `"${bin("prettier")}" --write src scripts vite.config.ts vitest.config.ts eslint.config.ts`,
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
    fix: [
      `"${bin("eslint")}" . --fix --cache --cache-location node_modules/.cache/eslint`,
    ],
  },
  {
    id: "swallow",
    label: "吞没",
    commands: [`"${process.execPath}" scripts/check-error-swallow.mjs`],
  },
  {
    id: "i18n",
    label: "文案",
    commands: [`"${process.execPath}" scripts/check-i18n-parity.mjs`],
  },
];

const PHASE2 = [
  {
    id: "contract",
    label: "契约",
    commands: [`"${process.execPath}" scripts/contract-check.mjs`],
  },
];

function printStatuses(results) {
  for (const { step, result } of results) {
    const status = result.ok ? "ok" : "FAIL";
    console.log(
      `[check] ${step.label} ${status} ${(result.ms / 1000).toFixed(1)}s`,
    );
  }
}

function printFailures(results) {
  for (const { step, result } of results.filter(({ result }) => !result.ok)) {
    console.log(`\n----- ${step.label} 输出 -----`);
    const trimmed = result.out.trim();
    console.log(trimmed.length > 0 ? trimmed : `(exit ${result.code})`);
  }
}

function allOk(results) {
  return results.every(({ result }) => result.ok);
}

async function runGroup(steps) {
  return Promise.all(
    steps.map(async (step) => ({
      step,
      result: await runChain(step.commands),
    })),
  );
}

/**
 * 前置检查失败时先跑一遍自动修复，再复检失败项。
 * 修好就继续往下走（契约照常校验），修不好才真正失败退出。
 */
async function fixThenRetry(results) {
  const failed = results.filter(({ result }) => !result.ok);
  const fixable = failed
    .map(({ step }) => step)
    .filter((step) => step.fix?.length);

  if (fixable.length === 0) {
    console.log("[check] 失败项没有自动修复能力，直接退出");
    printFailures(failed);
    return null;
  }

  console.log(
    `[check] 前置检查未通过，先自动修复：${fixable
      .map((step) => step.label)
      .join("、")}`,
  );
  const fixResults = await runGroup(
    fixable.map((step) => ({ ...step, commands: step.fix })),
  );
  for (const { step, result } of fixResults) {
    console.log(
      `[check] 修复 ${step.label} ${result.ok ? "ok" : "FAIL"} ${(
        result.ms / 1000
      ).toFixed(1)}s`,
    );
    if (!result.ok) {
      console.log(`\n----- ${step.label} 修复输出 -----`);
      console.log(result.out.trim() || `(exit ${result.code})`);
    }
  }

  const retryTargets = failed.map(({ step }) => step);
  const retried = await runGroup(retryTargets);
  console.log("[check] 复检");
  printStatuses(retried);
  if (!allOk(retried)) {
    printFailures(retried);
    return null;
  }
  return results;
}

const phase1 = await runGroup(PHASE1);
printStatuses(phase1);
if (!allOk(phase1)) {
  const recovered = await fixThenRetry(phase1);
  if (recovered === null) {
    console.log("[check] 前置检查仍然未通过，跳过契约校验");
    process.exit(1);
  }
  console.log("[check] 自动修复后通过，继续契约校验");
}

if (!contractNeeded()) {
  console.log("[check] 契约：Rust 侧与生成物未改动，跳过（--full 可强制）");
  process.exit(0);
}

const phase2 = await runGroup(PHASE2);
printStatuses(phase2);
if (!allOk(phase2)) printFailures(phase2);
process.exit(allOk(phase2) ? 0 : 1);
