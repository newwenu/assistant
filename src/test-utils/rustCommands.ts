import fs from "node:fs";
import path from "node:path";
import { repoPath } from "./paths";

const TAURI_SRC = repoPath("src-tauri/src");
const HANDLER_OPEN = "generate_handler![";
const PLUGIN_SUFFIX = "_plugin.rs";

function stripRustComments(text: string): string {
  let out = "";
  let index = 0;
  while (index < text.length) {
    const ch = text[index];
    if (ch === "/" && text[index + 1] === "/") {
      const newline = text.indexOf("\n", index);
      if (newline < 0) break;
      out += "\n";
      index = newline + 1;
      continue;
    }
    if (ch === "/" && text[index + 1] === "*") {
      const close = text.indexOf("*/", index + 2);
      out += " ";
      index = close < 0 ? text.length : close + 2;
      continue;
    }
    out += ch;
    index += 1;
  }
  return out;
}

function handlerBody(text: string, source: string): string {
  const cleaned = stripRustComments(text);
  const open = cleaned.indexOf(HANDLER_OPEN);
  if (open < 0) {
    throw new Error(`${source} 缺少 ${HANDLER_OPEN} 注册块`);
  }
  let depth = 1;
  const start = open + HANDLER_OPEN.length;
  for (let index = start; index < cleaned.length; index += 1) {
    const ch = cleaned[index];
    if (ch === "[") depth += 1;
    else if (ch === "]") {
      depth -= 1;
      if (depth === 0) return cleaned.slice(start, index);
    }
  }
  throw new Error(`${source} 的 ${HANDLER_OPEN} 注册块未闭合`);
}

function registeredNames(relative: string): string[] {
  const absolute = path.join(TAURI_SRC, relative);
  const text = fs.readFileSync(absolute, "utf8");
  const body = handlerBody(text, relative);
  const names: string[] = [];
  for (const entry of body.split(",")) {
    const trimmed = entry.trim();
    if (!trimmed) continue;
    if (!/^[A-Za-z0-9_:]+$/.test(trimmed)) {
      throw new Error(`${relative} 出现非标识符注册条目: ${trimmed}`);
    }
    names.push(trimmed.split("::").pop() as string);
  }
  return names;
}

export function coreCommands(): string[] {
  return registeredNames("ipc.rs");
}

export function pluginFeatures(): string[] {
  return fs
    .readdirSync(TAURI_SRC)
    .filter((file) => file.endsWith(PLUGIN_SUFFIX))
    .map((file) => file.slice(0, -PLUGIN_SUFFIX.length))
    .sort();
}

export function pluginCommands(feature: string): string[] {
  return registeredNames(`${feature}${PLUGIN_SUFFIX}`);
}

export function commandGroups(): Record<string, string[]> {
  const groups: Record<string, string[]> = { core: coreCommands() };
  for (const feature of pluginFeatures()) {
    groups[feature] = pluginCommands(feature);
  }
  return groups;
}
