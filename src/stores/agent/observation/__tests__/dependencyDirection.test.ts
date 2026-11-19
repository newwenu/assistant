import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { readRepoFile, repoPath } from "@/test-utils/paths";

/**
 * 依赖方向静态断言（I55 §6.5 / G7）。
 *
 * 控制路径（`send` / `cancel`）对观测模块零依赖；观测模块对控制路径零依赖。
 * 两侧只允许在组合层（`stores/chat/chatStorePorts.ts`）汇合。
 */

const CONTROL_PATH_FILES = [
  "src/services/stream/turnOrchestrator.ts",
  "src/services/stream/turnStrategy.ts",
  "src/services/stream/strategies/agentTurn.ts",
  "src/services/stream/strategies/chatTurn.ts",
  "src/services/stream/phase.ts",
  "src/services/chat/chatStreamEvents.ts",
  "src/stores/chat/chatStore.ts",
  "src/stores/chat/chatMessageSlice.ts",
  "src/stores/chat/chatConversationSlice.ts",
  "src/stores/chat/chatCompactSlice.ts",
  "src/stores/chat/chatStreamStore.ts",
];

const OBSERVATION_DIR = "src/stores/agent/observation";

const FORBIDDEN_IN_OBSERVATION = [
  "stores/chat",
  "services/stream",
  "services/chat",
  "@/features",
  "react",
  "antd",
  "i18n",
];

function importsOf(source: string): string[] {
  return [...source.matchAll(/from\s+["']([^"']+)["']/g)].map((m) => m[1]);
}

function walk(relativeDir: string): string[] {
  const absolute = repoPath(relativeDir);
  const entries = fs.readdirSync(absolute, { withFileTypes: true });
  const files: string[] = [];
  for (const entry of entries) {
    const childRelative = `${relativeDir}/${entry.name}`;
    if (entry.isDirectory()) {
      if (entry.name === "__tests__") continue;
      files.push(...walk(childRelative));
      continue;
    }
    if (entry.name.endsWith(".ts")) files.push(childRelative);
  }
  return files;
}

describe("依赖方向 — 控制路径不依赖观测（G7）", () => {
  it("控制路径文件不 import 观测模块", () => {
    const offenders: string[] = [];
    for (const file of CONTROL_PATH_FILES) {
      const hits = importsOf(readRepoFile(file)).filter((target) =>
        target.includes("agent/observation"),
      );
      if (hits.length > 0) offenders.push(`${file} → ${hits.join(", ")}`);
    }
    expect(offenders).toEqual([]);
  });

  it("编排骨架只用结构类型声明的观测端口（不 import 观测实现）", () => {
    const source = readRepoFile("src/services/stream/turnOrchestrator.ts");
    expect(source).toContain("export interface ObservationSinkPort");
    expect(importsOf(source).some((t) => t.includes("observation"))).toBe(
      false,
    );
  });

  it("组合层是唯一接线点：只写不读，不 import 投影与调度层", () => {
    const source = readRepoFile("src/stores/chat/chatStorePorts.ts");
    const imports = importsOf(source);
    expect(imports.some((t) => t.includes("agent/observation/store"))).toBe(
      true,
    );
    expect(imports.some((t) => t.includes("projections"))).toBe(false);
    expect(imports.some((t) => t.includes("observation/schedule"))).toBe(false);
  });
});

describe("依赖方向 — 观测模块不依赖控制与渲染（O6）", () => {
  const files = walk(OBSERVATION_DIR);

  it("观测模块不 import 控制路径 / React / 渲染层", () => {
    const offenders: string[] = [];
    for (const file of files) {
      const hits = importsOf(readRepoFile(file)).filter((target) =>
        FORBIDDEN_IN_OBSERVATION.some((forbidden) =>
          target.includes(forbidden),
        ),
      );
      if (hits.length > 0) offenders.push(`${file} → ${hits.join(", ")}`);
    }
    expect(offenders).toEqual([]);
  });

  it("三投影是纯函数：不读 store、不互相引用", () => {
    const projections = [
      "src/stores/agent/observation/projections/graph.ts",
      "src/stores/agent/observation/projections/timeline.ts",
      "src/stores/agent/observation/projections/summary.ts",
    ];
    for (const file of projections) {
      const imports = importsOf(readRepoFile(file));
      expect(imports.some((t) => t.includes("store"))).toBe(false);
      expect(imports.some((t) => t.includes("schedule"))).toBe(false);
      const siblings = projections.filter(
        (other) =>
          other !== file && imports.includes(`./${path.basename(other)}`),
      );
      expect(siblings).toEqual([]);
    }
  });

  it("调度层是唯一投影订阅点（页面不直连传输层事件）", () => {
    const pageFiles = walk("src/features/observation");
    const offenders: string[] = [];
    for (const file of pageFiles) {
      const imports = importsOf(readRepoFile(file));
      const hits = imports.filter(
        (target) =>
          target.includes("@tauri-apps/api/event") ||
          target.includes("services/stream/transport"),
      );
      if (hits.length > 0) offenders.push(`${file} → ${hits.join(", ")}`);
    }
    expect(offenders).toEqual([]);
  });
});
