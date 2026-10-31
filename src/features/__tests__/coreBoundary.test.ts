import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";

const SRC_ROOT = path.resolve(import.meta.dirname, "../..");
const REPO_ROOT = path.resolve(SRC_ROOT, "..");
const TAURI_ROOT = path.join(REPO_ROOT, "src-tauri");

const ASSEMBLY_POINTS = [
  "config/features.json",
  "src/vite-env.d.ts",
  "src/features/registry.ts",
  "src-tauri/Cargo.toml",
  "src-tauri/build.rs",
  "src-tauri/commands.json",
  "src-tauri/capabilities/default.json",
  "src-tauri/src/lib.rs",
  "src-tauri/src/plugins.rs",
];

const CORE_SCAN_ROOTS = ["src/settings", "src/features", "src-tauri/src"];
const CORE_EXTENSIONS = /\.(ts|tsx|rs|json|toml)$/;

const RESERVED_COMMAND_GROUPS = ["core", "hostOnly"];

const FEATURE_FOOTPRINT_ALLOWLIST: Record<string, string[]> = {
  vault: [
    "src-tauri/src/cmd/config.rs",
    "src-tauri/src/cmd/mod.rs",
    "src-tauri/src/ipc.rs",
    "src-tauri/src/lifecycle.rs",
    "src-tauri/src/protocol/dto/config.rs",
    "src-tauri/src/protocol/events.rs",
    "src-tauri/src/setup.rs",
  ],
};

interface FeatureEntry {
  default: boolean;
  cargoFeature: string;
  env: string;
}

function declaredFeatures(): Record<string, FeatureEntry> {
  const raw = fs.readFileSync(
    path.join(REPO_ROOT, "config/features.json"),
    "utf8",
  );
  return JSON.parse(raw) as Record<string, FeatureEntry>;
}

function read(relative: string): string {
  return fs.readFileSync(path.join(REPO_ROOT, relative), "utf8");
}

function exists(relative: string): boolean {
  return fs.existsSync(path.join(REPO_ROOT, relative));
}

function collect(relativeDir: string, out: string[] = []): string[] {
  const absolute = path.join(REPO_ROOT, relativeDir);
  if (!fs.existsSync(absolute)) return out;
  for (const entry of fs.readdirSync(absolute, { withFileTypes: true })) {
    const child = path.join(relativeDir, entry.name).split(path.sep).join("/");
    if (entry.isDirectory()) {
      collect(child, out);
    } else if (CORE_EXTENSIONS.test(entry.name)) {
      out.push(child);
    }
  }
  return out;
}

function isTestArtifact(relative: string): boolean {
  return (
    relative.includes("/__tests__/") ||
    relative.startsWith("src-tauri/tests/") ||
    /\.test\.(ts|tsx)$/.test(relative)
  );
}

function isFeatureBody(relative: string, feature: string): boolean {
  const dirs = [
    `src/features/${feature}/`,
    `src-tauri/src/cmd/${feature}/`,
    `src-tauri/permissions/${feature}/`,
  ];
  const files = [
    `src/features/${feature}.ts`,
    `src-tauri/src/${feature}_plugin.rs`,
  ];
  return (
    dirs.some((dir) => relative.startsWith(dir)) || files.includes(relative)
  );
}

function coreFootprint(feature: string): string[] {
  const needle = feature.toLowerCase();
  const hits = new Set<string>();
  for (const file of CORE_SCAN_ROOTS.flatMap((root) => collect(root))) {
    if (isTestArtifact(file)) continue;
    if (isFeatureBody(file, feature)) continue;
    if (ASSEMBLY_POINTS.includes(file)) continue;
    const text = read(file).toLowerCase();
    if (text.includes(needle)) hits.add(file);
  }
  return Array.from(hits).sort();
}

function cargoFeatureSection(): string {
  const cargo = read("src-tauri/Cargo.toml");
  const start = cargo.indexOf("[features]");
  if (start < 0) return "";
  const rest = cargo.slice(start + "[features]".length);
  const next = rest.indexOf("\n[");
  return next < 0 ? rest : rest.slice(0, next);
}

function declaredGroups(): Record<string, string[]> {
  const raw = read("src-tauri/commands.json");
  return (JSON.parse(raw) as { groups: Record<string, string[]> }).groups;
}

describe("特性扩展边界（新增特性改动 core 文件数 = 0）", () => {
  const features = declaredFeatures();
  const names = Object.keys(features).sort();

  it("声明特性的装配点齐备", () => {
    const problems: string[] = [];
    const registry = read("src/features/registry.ts");
    const viteEnv = read("src/vite-env.d.ts");
    const buildRs = read("src-tauri/build.rs");
    const libRs = read("src-tauri/src/lib.rs");
    const pluginsRs = read("src-tauri/src/plugins.rs");
    const capabilities = read("src-tauri/capabilities/default.json");
    const cargoFeatures = cargoFeatureSection();
    const groups = declaredGroups();

    for (const name of names) {
      const entry = features[name];
      if (entry.env !== `${name.toUpperCase()}_ENABLED`) {
        problems.push(`${name}: env 应为 ${name.toUpperCase()}_ENABLED`);
      }
      if (!viteEnv.includes(`declare const __${entry.env}__`)) {
        problems.push(`${name}: vite-env.d.ts 缺少 __${entry.env}__`);
      }
      if (!registry.includes(`__${entry.env}__`)) {
        problems.push(`${name}: registry.ts 未消费 __${entry.env}__`);
      }
      if (!exists(`src/features/${name}/module.ts`)) {
        problems.push(`${name}: 缺少前端模块 src/features/${name}/module.ts`);
      }
      if (!cargoFeatures.includes(`${entry.cargoFeature} = `)) {
        problems.push(
          `${name}: Cargo.toml [features] 缺少 ${entry.cargoFeature}`,
        );
      }
      if (
        entry.default &&
        !cargoFeatures.includes(`"${entry.cargoFeature}"`) &&
        !cargoFeatures.includes(`"${name}"`)
      ) {
        problems.push(`${name}: Cargo.toml default 集合未包含该特性`);
      }
      if (!buildRs.includes(`"${name}"`)) {
        problems.push(`${name}: build.rs 未声明内联插件 ${name}`);
      }
      if (!libRs.includes(`mod ${name}_plugin`)) {
        problems.push(`${name}: lib.rs 未声明 mod ${name}_plugin`);
      }
      if (!pluginsRs.includes(`${name}_plugin::init()`)) {
        problems.push(`${name}: plugins.rs 未挂载 ${name}_plugin::init()`);
      }
      if ((groups[name] ?? []).length === 0) {
        problems.push(`${name}: commands.json 缺少 groups.${name} 或为空`);
      }
      if (!capabilities.includes(`"${name}:default"`)) {
        problems.push(
          `${name}: capabilities/default.json 缺少 ${name}:default`,
        );
      }
      if (!exists(`src-tauri/permissions/${name}/default.toml`)) {
        problems.push(`${name}: 缺少 permissions/${name}/default.toml`);
      }
      const plugin = `src-tauri/src/${name}_plugin.rs`;
      if (!exists(plugin)) {
        problems.push(`${name}: 缺少后端插件 ${plugin}`);
      } else if (!read(plugin).includes(`feature = "${name}"`)) {
        problems.push(`${name}: ${plugin} 未用同名特性 cfg 包住自身`);
      }
    }

    expect(problems).toEqual([]);
  });

  it("装配点集合与声明特性双向一致", () => {
    const problems: string[] = [];

    const groupNames = Object.keys(declaredGroups())
      .filter((key) => !RESERVED_COMMAND_GROUPS.includes(key))
      .sort();
    if (groupNames.join("|") !== names.join("|")) {
      problems.push(
        `commands.json 分组 ${groupNames.join(", ")} ≠ 声明特性 ${names.join(", ")}`,
      );
    }

    const pluginStems = fs
      .readdirSync(path.join(TAURI_ROOT, "src"))
      .filter((file) => file.endsWith("_plugin.rs"))
      .map((file) => file.slice(0, -"_plugin.rs".length))
      .sort();
    if (pluginStems.join("|") !== names.join("|")) {
      problems.push(
        `后端插件 ${pluginStems.join(", ")} ≠ 声明特性 ${names.join(", ")}`,
      );
    }

    const permissionDirs = fs
      .readdirSync(path.join(TAURI_ROOT, "permissions"))
      .filter((entry) =>
        fs.statSync(path.join(TAURI_ROOT, "permissions", entry)).isDirectory(),
      )
      .sort();
    if (permissionDirs.join("|") !== names.join("|")) {
      problems.push(
        `ACL 目录 ${permissionDirs.join(", ")} ≠ 声明特性 ${names.join(", ")}`,
      );
    }

    expect(problems).toEqual([]);
  });

  it("特性名在 core 逻辑中的足迹被冻结", () => {
    const problems: string[] = [];
    for (const name of names) {
      const actual = coreFootprint(name);
      const expected = [...(FEATURE_FOOTPRINT_ALLOWLIST[name] ?? [])].sort();
      if (actual.join("|") === expected.join("|")) continue;
      const added = actual.filter((file) => !expected.includes(file));
      const removed = expected.filter((file) => !actual.includes(file));
      problems.push(
        `${name}: 新增 core 引用 [${added.join(", ")}] / 已消失 [${removed.join(", ")}]`,
      );
    }
    expect(problems).toEqual([]);
  });
});
