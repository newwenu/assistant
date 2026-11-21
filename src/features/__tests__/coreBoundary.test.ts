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

// 足迹豁免：只登记「无法用无名词表达」的装配点，其余一律改代码消除
// （新特性不得复制这份历史豁免，见 feature-optional-binary-spec §8.1）。
//
// 豁免粒度是**行**不是文件：`markers` 是「该行必须包含的片段」，未列入的命中行
// 一律判违规。整文件放行会让门禁对该文件的其余部分失效（例如 `use assistant_vault`
// 或新增 `#[cfg(feature = "vault")]` 都会被静默放过）。
interface FootprintAllowance {
  file: string;
  /** 必须非空：豁免理由成为数据，不再是无机制约束的注释。 */
  reason: string;
  /** 允许命中的行（子串匹配，容忍缩进 / 格式化差异）。 */
  markers: string[];
}

const FEATURE_FOOTPRINT_ALLOWLIST: Record<string, FootprintAllowance[]> = {
  vault: [
    {
      file: "src-tauri/src/cmd/mod.rs",
      reason:
        '装配点：`#[cfg(feature = "vault")] mod vault;` 的 cfg 谓词必须点名特性名',
      markers: ['#[cfg(feature = "vault")]', "mod vault;"],
    },
    {
      file: "src-tauri/src/protocol/events.rs",
      reason: "wire 事件名 `vault:*` 是数据不是代码知识，无法用无名词表达",
      markers: [
        '"vault:locked"',
        '"vault:unlocked"',
        '"vault:biometric-enroll-step"',
      ],
    },
    {
      file: "src-tauri/src/app_dirs/registry.rs",
      reason:
        "`E9` 的 key / segments：`security/vault` 是盘上目录名（改名等于改磁盘格式），且按 I61 G19(c) 该行与构建无关恒在，vaultless 下要能归 `optionalNotMounted` 而非 `unregistered`。只覆盖这两行，`owner` 等其余命中一律判红（I61 G17(a)）",
      markers: ['key: "security/vault"', 'segments: &["security", "vault"]'],
    },
    {
      file: "src-tauri/src/app_dirs/registry.rs",
      reason:
        "V22 / V24 的反向断言：显式证明 `owner` 与能力名不等于 feature 名或可选项类型名，feature 名必须出现在断言里才有意义",
      markers: [
        'const FEATURE_NAMES: &[&str] = &["vault"];',
        '!root.owner.contains("assistant_vault")',
        '!root.owner.contains("VaultStorageProbe")',
        'assert_ne!(SECURITY_OBSERVABILITY, "vault", "不得是 feature 名");',
        'assert_ne!(SECURITY_OBSERVABILITY, "VaultProbe", "不得是类型名");',
      ],
    },
    {
      file: "src/settings/pages/storage/lexicon.ts",
      reason:
        "`security/vault` 是盘上目录名（条目 key 为对外契约，改名等于改磁盘格式），前端条目名称表必须逐字登记该 key",
      markers: [
        '"security/vault": "settings.storage.entries.names.security_vault",',
      ],
    },
  ],
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

// 特性的中文产品名与英文 id 等价计入足迹：core 里出现「保险库」和出现 `vault`
// 都是同一个特性泄漏，扫描不能只认 ASCII。新增特性若也有中文名，在此登记。
const FEATURE_ALIASES: Record<string, string[]> = {
  vault: ["保险库"],
};

interface FootprintViolation {
  file: string;
  line: number;
  text: string;
}

function coreFootprintViolations(feature: string): FootprintViolation[] {
  const needles = [feature, ...(FEATURE_ALIASES[feature] ?? [])].map((name) =>
    name.toLowerCase(),
  );
  const allowances = FEATURE_FOOTPRINT_ALLOWLIST[feature] ?? [];
  const violations: FootprintViolation[] = [];
  for (const file of CORE_SCAN_ROOTS.flatMap((root) => collect(root))) {
    if (isTestArtifact(file)) continue;
    if (isFeatureBody(file, feature)) continue;
    if (ASSEMBLY_POINTS.includes(file)) continue;
    const markers = allowances
      .filter((allowance) => allowance.file === file)
      .flatMap((allowance) => allowance.markers);
    read(file)
      .split(/\r?\n/)
      .forEach((text, index) => {
        const lower = text.toLowerCase();
        if (!needles.some((needle) => lower.includes(needle))) return;
        if (markers.some((marker) => text.includes(marker))) return;
        violations.push({ file, line: index + 1, text: text.trim() });
      });
  }
  return violations;
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
      for (const violation of coreFootprintViolations(name)) {
        problems.push(
          `${name}: 未豁免的 core 引用 ${violation.file}:${violation.line} → ${violation.text}`,
        );
      }
      for (const allowance of FEATURE_FOOTPRINT_ALLOWLIST[name] ?? []) {
        if (allowance.reason.trim() === "") {
          problems.push(`${name}: 豁免 ${allowance.file} 缺少 reason`);
        }
        const lines = read(allowance.file).split(/\r?\n/);
        const deadMarkers = allowance.markers.filter(
          (marker) => !lines.some((line) => line.includes(marker)),
        );
        if (deadMarkers.length > 0) {
          problems.push(
            `${name}: 豁免 ${allowance.file} 的 markers 失效 [${deadMarkers.join(", ")}]（死豁免，必须删除条目）`,
          );
        }
      }
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
