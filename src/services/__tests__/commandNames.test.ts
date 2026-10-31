import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";

const SRC_ROOT = path.resolve(import.meta.dirname, "../..");
const COMMANDS_JSON = path.resolve(SRC_ROOT, "../src-tauri/commands.json");
const INVOKE_LITERAL =
  /(?:^|[^A-Za-z0-9_$])(?:invoke|tauriInvoke)(?:<[^>]*>)?\(\s*"([^"]+)"/g;

function collectSourceFiles(dir: string, out: string[] = []): string[] {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      collectSourceFiles(full, out);
    } else if (
      /\.(ts|tsx)$/.test(entry.name) &&
      !/\.test\.(ts|tsx)$/.test(entry.name)
    ) {
      out.push(full);
    }
  }
  return out;
}

function literalInvokes(files: string[]): Map<string, string[]> {
  const found = new Map<string, string[]>();
  for (const file of files) {
    const text = fs.readFileSync(file, "utf8");
    for (const match of text.matchAll(INVOKE_LITERAL)) {
      const name = match[1];
      const relative = path.relative(SRC_ROOT, file).replace(/\\/g, "/");
      const hits = found.get(name) ?? [];
      hits.push(relative);
      found.set(name, hits);
    }
  }
  return found;
}

function declaredGroups(): Record<string, string[]> {
  const raw = fs.readFileSync(COMMANDS_JSON, "utf8");
  const parsed = JSON.parse(raw) as { groups: Record<string, string[]> };
  return parsed.groups;
}

function sortedUnique(values: string[]): string[] {
  return Array.from(new Set(values)).sort();
}

function difference(a: string[], b: string[]): string[] {
  const known = new Set(b);
  return sortedUnique(a.filter((name) => !known.has(name)));
}

describe("前后端命令名双向差集", () => {
  const files = collectSourceFiles(SRC_ROOT);
  const invokes = literalInvokes(files);
  const groups = declaredGroups();
  const declared = sortedUnique(Object.values(groups).flat());

  it("前端字面量命令全部在声明清单内", () => {
    const stray = difference(Array.from(invokes.keys()), declared);
    expect(stray, `前端调用了未声明的命令: ${stray.join(", ")}`).toEqual([]);
  });

  it("vault 前端命令与 vault 声明清单双向一致", () => {
    const frontVault = Array.from(invokes.keys()).filter((name) =>
      name.startsWith("vault_"),
    );
    const declaredVault = groups.vault ?? [];
    expect(
      difference(frontVault, declaredVault),
      "vault 前端调用了未声明的命令",
    ).toEqual([]);
    expect(
      difference(declaredVault, frontVault),
      "vault 声明了前端未调用的命令",
    ).toEqual([]);
  });

  it("声明清单内命令不重复", () => {
    const all = Object.values(groups).flat();
    expect(sortedUnique(all).length).toBe(all.length);
  });
});
