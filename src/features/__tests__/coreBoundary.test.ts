import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { commandGroups } from "@/test-utils/rustCommands";
import { readRepoFile, repoFileExists, repoPath } from "@/test-utils/paths";

const TAURI_ROOT = repoPath("src-tauri");

const ASSEMBLY_POINTS = [
  "config/features.json",
  "src/vite-env.d.ts",
  "src/features/registry.ts",
  "src-tauri/Cargo.toml",
  "src-tauri/src/feature_mounts.rs",
  "src-tauri/src/lib.rs",
];

const TRAVERSAL_FILES = [
  "src-tauri/build.rs",
  "src-tauri/capabilities/default.json",
  "src-tauri/src/plugins.rs",
  "src-tauri/src/lifecycle.rs",
  "src-tauri/src/setup.rs",
  "src-tauri/src/ipc.rs",
  "src-tauri/src/window.rs",
  "src-tauri/src/tray.rs",
  "src-tauri/src/cmd/config.rs",
  "src-tauri/tauri.conf.json",
  "src/App.tsx",
  "src/layouts/AppShell.tsx",
];

const CORE_SCAN_ROOTS = ["src/settings", "src/features", "src-tauri/src"];
const CORE_EXTENSIONS = /\.(ts|tsx|rs|json|toml)$/;

const RESERVED_COMMAND_GROUPS = ["core"];

const FEATURE_FOOTPRINT_ALLOWLIST: Record<string, string[]> = {
  vault: ["src-tauri/src/cmd/mod.rs", "src-tauri/src/protocol/events.rs"],
};

const FEATURE_ID_PATTERN = /^[a-z][a-z0-9_]*$/;
const FEATURE_KEYS = ["default", "cargoFeature", "env"];

interface FeatureEntry {
  default: boolean;
  cargoFeature?: string;
  env?: string;
}

function declaredFeatures(): Record<string, FeatureEntry> {
  const raw = readRepoFile("config/features.json");
  return JSON.parse(raw) as Record<string, FeatureEntry>;
}

function featureEnv(name: string): string {
  return `${name.toUpperCase()}_ENABLED`;
}

function featureCargo(name: string, entry: FeatureEntry): string {
  return entry.cargoFeature ?? name;
}

function read(relative: string): string {
  return readRepoFile(relative);
}

function exists(relative: string): boolean {
  return repoFileExists(relative);
}

function collect(relativeDir: string, out: string[] = []): string[] {
  const absolute = repoPath(relativeDir);
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
    `src-tauri/capabilities/${feature}.json`,
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
  return commandGroups();
}

describe("特性扩展边界（新增特性改动 core 文件数 = 0）", () => {
  const features = declaredFeatures();
  const names = Object.keys(features).sort();

  it("清单 schema v2：id 正则 / 闭键 / 派生值一致", () => {
    const problems: string[] = [];
    const raw = JSON.parse(readRepoFile("config/features.json")) as Record<
      string,
      Record<string, unknown>
    >;
    const entries = Object.entries(raw);
    if (entries.length === 0) {
      problems.push("config/features.json 未声明任何特性，schema 断言失去基准");
    }
    for (const [name, spec] of entries) {
      if (!FEATURE_ID_PATTERN.test(name)) {
        problems.push(
          `${name}: id 必须匹配 ${FEATURE_ID_PATTERN.source}（单段蛇形，不含连字符）`,
        );
      }
      for (const key of Object.keys(spec)) {
        if (!FEATURE_KEYS.includes(key)) {
          problems.push(`${name}: 清单条目含未知键 ${key}`);
        }
      }
      if (typeof spec.default !== "boolean") {
        problems.push(`${name}: default 必须是布尔值`);
      }
      if (spec.cargoFeature !== undefined && spec.cargoFeature !== name) {
        problems.push(`${name}: 显式 cargoFeature 必须逐字等于 id`);
      }
      if (spec.env !== undefined && spec.env !== featureEnv(name)) {
        problems.push(`${name}: 显式 env 必须逐字等于 ${featureEnv(name)}`);
      }
    }
    expect(problems).toEqual([]);
  });

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

    if (!buildRs.includes("read_dir")) {
      problems.push("build.rs 未按目录遍历插件真源（缺少 read_dir）");
    }
    if (!buildRs.includes("_plugin.rs")) {
      problems.push("build.rs 未以 _plugin.rs 后缀识别插件文件");
    }
    if (!pluginsRs.includes("feature_mounts::mounts()")) {
      problems.push("plugins.rs 未遍历 feature_mounts::mounts()");
    }

    for (const name of names) {
      const entry = features[name];
      const env = featureEnv(name);
      const cargoFeature = featureCargo(name, entry);
      if (entry.env !== undefined && entry.env !== env) {
        problems.push(`${name}: 显式 env 必须逐字等于 ${env}`);
      }
      if (!viteEnv.includes(`declare const __${env}__`)) {
        problems.push(`${name}: vite-env.d.ts 缺少 __${env}__`);
      }
      if (!registry.includes(`__${env}__`)) {
        problems.push(`${name}: registry.ts 未消费 __${env}__`);
      }
      if (!exists(`src/features/${name}/module.ts`)) {
        problems.push(`${name}: 缺少前端模块 src/features/${name}/module.ts`);
      }
      if (!cargoFeatures.includes(`${cargoFeature} = `)) {
        problems.push(`${name}: Cargo.toml [features] 缺少 ${cargoFeature}`);
      }
      if (
        entry.default &&
        !cargoFeatures.includes(`"${cargoFeature}"`) &&
        !cargoFeatures.includes(`"${name}"`)
      ) {
        problems.push(`${name}: Cargo.toml default 集合未包含该特性`);
      }
      if (!libRs.includes(`mod ${name}_plugin`)) {
        problems.push(`${name}: lib.rs 未声明 mod ${name}_plugin`);
      }
      if (!libRs.includes(`mod feature_mounts`)) {
        problems.push(`${name}: lib.rs 未声明 mod feature_mounts`);
      }
      const mounts = read("src-tauri/src/feature_mounts.rs");
      if (!mounts.includes(`id: "${name}"`)) {
        problems.push(`${name}: feature_mounts.rs 的 MOUNTS 未登记 id ${name}`);
      }
      if ((groups[name] ?? []).length === 0) {
        problems.push(`${name}: 插件 ${name}_plugin.rs 未注册任何命令`);
      }
      const capability = `src-tauri/capabilities/${name}.json`;
      if (!exists(capability)) {
        problems.push(`${name}: 缺少特性能力文件 ${capability}`);
      } else if (!read(capability).includes(`"${name}:default"`)) {
        problems.push(
          `${name}: ${capability} 的 permissions 缺少 ${name}:default`,
        );
      }
      if (capabilities.includes(`"${name}:default"`)) {
        problems.push(
          `${name}: capabilities/default.json 仍授予 ${name}:default，特性权限未拆分`,
        );
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
        `插件注册分组 ${groupNames.join(", ")} ≠ 声明特性 ${names.join(", ")}`,
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

    const permissionsRoot = path.join(TAURI_ROOT, "permissions");
    const permissionDirs = exists("src-tauri/permissions")
      ? fs
          .readdirSync(permissionsRoot)
          .filter((entry) =>
            fs.statSync(path.join(permissionsRoot, entry)).isDirectory(),
          )
          .sort()
      : [];
    for (const dir of permissionDirs) {
      if (!names.includes(dir)) {
        problems.push(`ACL 目录 ${dir} 不对应任何声明特性（残留目录）`);
      } else if (!exists(`src-tauri/permissions/${dir}/default.toml`)) {
        problems.push(`ACL 目录 ${dir} 为空目录且无 default.toml`);
      }
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

  it("registry.ts 只允许动态 import + 编译期守卫", () => {
    const registry = read("src/features/registry.ts");
    const problems: string[] = [];
    for (const name of names) {
      const staticImport = new RegExp(
        `import\\s*\\{[^}]*\\}\\s*from\\s*"(\\.[^"]*|@/features)/${name}/`,
      );
      if (staticImport.test(registry)) {
        problems.push(`${name}: registry.ts 使用了静态 import，chunk 无法裁剪`);
      }
      if (!registry.includes(`await import("./${name}/module")`)) {
        problems.push(
          `${name}: registry.ts 缺少 await import("./${name}/module")`,
        );
      }
    }
    expect(problems).toEqual([]);
  });

  it("每个特性的 env 开关都在 vite-env.d.ts 声明", () => {
    const viteEnv = read("src/vite-env.d.ts");
    const problems: string[] = [];
    for (const name of names) {
      const env = featureEnv(name);
      if (features[name].env !== undefined && features[name].env !== env) {
        problems.push(`${name}: 显式 env 必须逐字等于 ${env}`);
      }
      if (
        !new RegExp(`declare const __${env}__\\s*:\\s*boolean`).test(viteEnv)
      ) {
        problems.push(
          `${name}: vite-env.d.ts 缺少 declare const __${env}__: boolean`,
        );
      }
    }
    expect(problems).toEqual([]);
  });

  it("遍历化后的调用方不再点名特性", () => {
    const problems: string[] = [];
    for (const file of TRAVERSAL_FILES) {
      const text = read(file).toLowerCase();
      for (const name of names) {
        if (text.includes(name.toLowerCase())) {
          problems.push(`${file} 仍出现特性名 ${name}`);
        }
      }
    }
    expect(problems).toEqual([]);
  });

  it("特性自有契约类型不回流主契约", () => {
    const problems: string[] = [];
    for (const name of names) {
      const bodyFiles = collect(`src/features/${name}`).filter(
        (file) => !isTestArtifact(file),
      );
      for (const file of bodyFiles) {
        if (read(file).includes("@/types/protocol.contract")) {
          problems.push(
            `${file}: 特性契约类型应从自身契约域导入，不得依赖 @/types/protocol.contract`,
          );
        }
      }
    }
    expect(problems).toEqual([]);
  });

  it("关闭态产物不含特性专有字符串", (ctx) => {
    const distDir = repoPath("dist");
    const closed = names.filter(
      (name) => process.env[featureEnv(name)] === "0",
    );
    if (closed.length === 0) {
      ctx.skip();
      return;
    }
    expect(
      fs.existsSync(path.join(distDir, "assets")),
      "声明了关闭态（env=0）却没有 dist/assets：先跑 feature-build.ps1 -Task build -Without <id> 再验",
    ).toBe(true);
    for (const name of closed) {
      const result = spawnSync(
        process.execPath,
        [
          repoPath("scripts/assert-feature-absent.mjs"),
          "--feature",
          name,
          "--dist",
          distDir,
        ],
        { encoding: "utf8" },
      );
      expect(result.status, result.stderr ?? "").toBe(0);
    }
  });
});
