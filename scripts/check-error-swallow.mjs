#!/usr/bin/env node
// 吞没门禁：扫描前端空 catch 与后端 `let _ =` 丢弃 IO / 持久化结果的落点。
// 用法: node scripts/check-error-swallow.mjs
//
// 只做检测，不做自动修复：改法涉及日志级别与回滚语义，必须人工判断。

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const TS_PATTERNS = [
  {
    // `.catch(() => {})`（含跨行空块）
    re: /\.catch\(\s*\(\s*\)\s*=>\s*\{\s*\}\s*\)/g,
    label: "空 catch 回调",
  },
  {
    // `.catch(() => null)`
    re: /\.catch\(\s*\(\s*\)\s*=>\s*null\s*(?:,?\s*\))/g,
    label: "catch 返回 null",
  },
];

// 只兜 IO / 持久化一类丢弃点：`write!` 宏（String 拼装不会失败）、
// `clear_sync`（FFI 清理）都不匹配，避免误报。
const RS_PATTERN =
  /\blet\s*_\s*=\s*[^;]*(\bfs::write|\bwrite\(|\bwrite_all\(|\.clear\(|with_conn)/;

// 目录级排除：测试与示例代码不纳入门禁。
const SKIP_DIRS = new Set(["tests", "examples", "target", "node_modules"]);

// 逐条白名单：命中仍允许，理由写在条目上。
// 注意：本脚本的正则只兜 IO 类形态；`conf.save()` / `delete_key()` / `set_position`
// 等由人工清单（S4-e）逐条收口，不在此登记。
const WHITELIST = [
  {
    file: "src-tauri/crates/assistant-tools/src/mcp/stdio/mod.rs",
    line: 540,
    reason: "内联 #[cfg(test)] 用例：故意注入非法字节，写入结果不是被测目标",
  },
  {
    file: "src-tauri/crates/assistant-tools/src/mcp/stdio/mod.rs",
    line: 541,
    reason: "内联 #[cfg(test)] 用例：故意注入非法字节，写入结果不是被测目标",
  },
];

const BEST_EFFORT = "best-effort:";

function rel(abs) {
  return path.relative(ROOT, abs).split(path.sep).join("/");
}

function walk(dir, extensions, out) {
  if (!fs.existsSync(dir)) return;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      walk(path.join(dir, entry.name), extensions, out);
      continue;
    }
    if (!entry.isFile()) continue;
    if (extensions.some((ext) => entry.name.endsWith(ext))) {
      out.push(path.join(dir, entry.name));
    }
  }
}

function whitelisted(file, line) {
  return WHITELIST.some((item) => item.file === file && item.line === line);
}

function exemptByComment(lines, lineNo) {
  const current = lines[lineNo - 1] ?? "";
  const previous = lines[lineNo - 2] ?? "";
  return current.includes(BEST_EFFORT) || previous.includes(BEST_EFFORT);
}

function scanTs(abs, hits) {
  const text = fs.readFileSync(abs, "utf8");
  const lines = text.split(/\r?\n/);
  const file = rel(abs);
  for (const pattern of TS_PATTERNS) {
    pattern.re.lastIndex = 0;
    let match;
    while ((match = pattern.re.exec(text)) !== null) {
      const lineNo = text.slice(0, match.index).split(/\r?\n/).length;
      if (exemptByComment(lines, lineNo)) continue;
      if (whitelisted(file, lineNo)) continue;
      hits.push({
        file,
        line: lineNo,
        snippet: lines[lineNo - 1].trim(),
        label: pattern.label,
      });
    }
  }
}

function scanRs(abs, hits) {
  const lines = fs.readFileSync(abs, "utf8").split(/\r?\n/);
  const file = rel(abs);
  lines.forEach((line, index) => {
    if (!RS_PATTERN.test(line)) return;
    const lineNo = index + 1;
    if (exemptByComment(lines, lineNo)) return;
    if (whitelisted(file, lineNo)) return;
    hits.push({
      file,
      line: lineNo,
      snippet: line.trim(),
      label: "丢弃 IO/持久化结果",
    });
  });
}

function main() {
  const files = [];
  walk(path.join(ROOT, "src"), [".ts", ".tsx"], files);
  walk(path.join(ROOT, "src-tauri"), [".rs"], files);

  const hits = [];
  for (const abs of files) {
    if (abs.endsWith(".rs")) scanRs(abs, hits);
    else scanTs(abs, hits);
  }

  for (const hit of hits) {
    console.error(`${hit.file}:${hit.line}  ${hit.snippet}  [${hit.label}]`);
  }
  if (hits.length > 0) {
    console.error(`[check-error-swallow] 发现 ${hits.length} 处吞没`);
    process.exit(1);
  }
  console.log(
    `[check-error-swallow] 通过（扫描 ${files.length} 个文件，白名单 ${WHITELIST.length} 条）`,
  );
}

main();
