#!/usr/bin/env node
// 文案门禁：校验各 locale 的键集与插值占位符完全对称。
// 用法: node scripts/check-i18n-parity.mjs
//
// 为什么需要：`I18nPath` 由 zh-CN 编译期推导，英文缺键不会报类型错，
// 只会在运行时回退成 key 字面量，人工同步极易漏项。

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const LOCALE_DIR = path.join(ROOT, "src/i18n/locales");
const BASE_LOCALE = "zh-CN";

function flatten(value, prefix, out) {
  for (const [key, child] of Object.entries(value)) {
    const full = prefix ? `${prefix}.${key}` : key;
    if (child && typeof child === "object" && !Array.isArray(child)) {
      flatten(child, full, out);
      continue;
    }
    out.set(full, String(child));
  }
  return out;
}

function placeholders(text) {
  return new Set(
    [...text.matchAll(/\{\{\s*([A-Za-z0-9_]+)\s*\}\}/g)].map((m) => m[1]),
  );
}

function loadLocale(file) {
  const abs = path.join(LOCALE_DIR, file);
  const parsed = JSON.parse(fs.readFileSync(abs, "utf8"));
  return flatten(parsed, "", new Map());
}

function main() {
  if (!fs.existsSync(LOCALE_DIR)) {
    console.error(`[check-i18n-parity] 目录不存在: ${LOCALE_DIR}`);
    process.exit(1);
  }
  const files = fs
    .readdirSync(LOCALE_DIR)
    .filter((name) => name.endsWith(".json"));
  const baseName = `${BASE_LOCALE}.json`;
  if (!files.includes(baseName)) {
    console.error(`[check-i18n-parity] 缺少基准语言文件: ${baseName}`);
    process.exit(1);
  }

  const base = loadLocale(baseName);
  const problems = [];

  for (const file of files) {
    if (file === baseName) continue;
    const other = loadLocale(file);

    for (const key of base.keys()) {
      if (!other.has(key)) {
        problems.push(`${file}  缺少键 ${key}`);
        continue;
      }
      const a = placeholders(base.get(key));
      const b = placeholders(other.get(key));
      const missing = [...a].filter((name) => !b.has(name));
      const extra = [...b].filter((name) => !a.has(name));
      if (missing.length > 0) {
        problems.push(
          `${file}  ${key} 缺少占位符 ${missing.map((n) => `{{${n}}}`).join(" ")}`,
        );
      }
      if (extra.length > 0) {
        problems.push(
          `${file}  ${key} 多出占位符 ${extra.map((n) => `{{${n}}}`).join(" ")}`,
        );
      }
    }
    for (const key of other.keys()) {
      if (!base.has(key)) problems.push(`${file}  多出键 ${key}`);
    }
  }

  if (problems.length > 0) {
    for (const line of problems) console.error(line);
    console.error(`[check-i18n-parity] 发现 ${problems.length} 处不一致`);
    process.exit(1);
  }
  console.log(
    `[check-i18n-parity] 通过（${files.length} 个语言包，${base.size} 个键，基准 ${BASE_LOCALE}）`,
  );
}

main();
