import { describe, expect, it } from "vitest";
import fs from "node:fs";
import {
  getBuiltinConfSlots,
  getBuiltinPages,
} from "@/settings/core/builtinRegistry";
import { commandGroups } from "@/test-utils/rustCommands";
import { readRepoFile, repoFileExists, repoPath } from "@/test-utils/paths";

interface FeatureEntry {
  default: boolean;
  cargoFeature: string;
  env: string;
}

interface SlotDecl {
  group: string;
  invoke: string;
  namespace?: string;
  pageId?: string;
}

function read(relative: string): string {
  return readRepoFile(relative);
}

function exists(relative: string): boolean {
  return repoFileExists(relative);
}

function declaredFeatures(): Record<string, FeatureEntry> {
  return JSON.parse(read("config/features.json")) as Record<
    string,
    FeatureEntry
  >;
}

function bracketBody(text: string, openIndex: number): string {
  let depth = 0;
  for (let index = openIndex; index < text.length; index += 1) {
    const ch = text[index];
    if (ch === "[") depth += 1;
    else if (ch === "]") {
      depth -= 1;
      if (depth === 0) return text.slice(openIndex + 1, index);
    }
  }
  return "";
}

function arrayBodyAfter(text: string, key: string): string {
  const at = text.indexOf(key);
  if (at < 0) return "";
  const open = text.indexOf("[", at);
  if (open < 0) return "";
  return bracketBody(text, open);
}

function extractField(text: string, field: string): string | undefined {
  const match = text.match(new RegExp(`${field}:\\s*"([^"]+)"`));
  return match?.[1];
}

function featureSlotFiles(): { name: string; file: string }[] {
  const featuresDir = repoPath("src/features");
  return fs
    .readdirSync(featuresDir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .filter((name) => exists(`src/features/${name}/conf.ts`))
    .map((name) => ({ name, file: `src/features/${name}/conf.ts` }));
}

function featureSlotDecl(file: string): SlotDecl {
  const text = read(file);
  const marker = text.lastIndexOf("satisfies ConfSlot");
  const start = marker < 0 ? 0 : text.lastIndexOf("export const", marker);
  const body = marker < 0 ? text : text.slice(start, marker);
  const group = extractField(body, "group");
  const invoke = extractField(body, "invoke");
  expect(group, `${file} 未声明 group`).toBeDefined();
  expect(invoke, `${file} 未声明 invoke`).toBeDefined();
  return {
    group: group as string,
    invoke: invoke as string,
    namespace: extractField(body, "namespace"),
    pageId: extractField(body, "pageId"),
  };
}

function featureSlotExportName(file: string): string | undefined {
  return read(file).match(/export const\s+([A-Za-z0-9_]+)/)?.[1];
}

describe("配置槽位契约（槽位声明与命令清单、页面注册表、模块装配三方一致）", () => {
  it("内置槽位的 invoke 在 core 注册表中，pageId 在页面注册表中", () => {
    const problems: string[] = [];
    const slots = getBuiltinConfSlots();
    const core = new Set(commandGroups().core ?? []);
    const pageIds = new Set(getBuiltinPages().map((page) => page.id));

    if (slots.length < 3) {
      problems.push(
        `内置槽位仅 ${slots.length} 个，低于预期 3 个，解析器可能已失效`,
      );
    }

    for (const slot of slots) {
      if (!core.has(slot.invoke)) {
        problems.push(
          `${slot.group}: invoke ${slot.invoke} 未在 core 注册表声明`,
        );
      }
      if (slot.pageId && !pageIds.has(slot.pageId)) {
        problems.push(`${slot.group}: pageId ${slot.pageId} 不在页面注册表中`);
      }
    }

    expect(problems).toEqual([]);
  });

  it("特性槽位的 group 与特性同名、invoke 在命令清单中、且被模块装配", () => {
    const problems: string[] = [];
    const features = declaredFeatures();
    const groups = commandGroups();
    const builtinPageIds = new Set(getBuiltinPages().map((page) => page.id));

    for (const { name, file } of featureSlotFiles()) {
      const decl = featureSlotDecl(file);
      if (!features[name]) {
        problems.push(`${name}: 存在 ${file} 但未在 config/features.json 声明`);
      }
      if (decl.group !== name) {
        problems.push(`${name}: 槽位 group 为 ${decl.group}，必须与特性名同名`);
      }
      if (decl.namespace !== name) {
        problems.push(
          `${name}: 槽位 namespace 为 ${decl.namespace ?? "(未声明)"}，插件命令必须以 plugin:${name}| 前缀调用`,
        );
      }

      const allowed = new Set([
        ...(groups.core ?? []),
        ...(groups[name] ?? []),
      ]);
      if (!allowed.has(decl.invoke)) {
        problems.push(
          `${name}: invoke ${decl.invoke} 未在 core 或 ${name} 插件注册表中声明`,
        );
      }

      const exportName = featureSlotExportName(file);
      if (!exportName) {
        problems.push(`${name}: ${file} 未导出槽位常量`);
      } else {
        const moduleText = read(`src/features/${name}/module.ts`);
        const confsBody = arrayBodyAfter(moduleText, "confs:");
        if (!confsBody.includes(exportName)) {
          problems.push(
            `${name}: module.ts 的 confs 未包含 ${exportName}，槽位声明不会被注册`,
          );
        }
      }

      if (decl.pageId) {
        const modulePages = arrayBodyAfter(
          read(`src/features/${name}/module.ts`),
          "pages:",
        );
        const featurePageIds = [
          ...modulePages.matchAll(/id:\s*"([^"]+)"/g),
        ].map((match) => match[1]);
        if (
          !builtinPageIds.has(decl.pageId) &&
          !featurePageIds.includes(decl.pageId)
        ) {
          problems.push(
            `${name}: pageId ${decl.pageId} 既非内置页面也未在模块 pages 中声明`,
          );
        }
      }
    }

    expect(problems).toEqual([]);
  });

  it("槽位 group 在内置与特性之间全局唯一", () => {
    const problems: string[] = [];
    const seen = new Map<string, string>();

    for (const slot of getBuiltinConfSlots()) {
      seen.set(slot.group, "builtin");
    }
    for (const { name, file } of featureSlotFiles()) {
      const decl = featureSlotDecl(file);
      const owner = seen.get(decl.group);
      if (owner) {
        problems.push(`group ${decl.group} 重复声明：${owner} 与 ${name}`);
      } else {
        seen.set(decl.group, name);
      }
    }

    expect(problems).toEqual([]);
  });
});
