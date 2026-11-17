#!/usr/bin/env node
// F7：关闭态产物图断言（Vite generateBundle，只读）
// 判定抽成纯函数 findFeatureModules，插件只报告不修改 bundle。

const FEATURE_ROOT = "src/features/";

function normalizeModuleId(moduleId) {
  return moduleId.replace(/\\/g, "/").replace(/[?#].*$/, "");
}

export function findFeatureModules(moduleIds, featureIds) {
  const violations = [];
  for (const moduleId of moduleIds) {
    const normalized = normalizeModuleId(moduleId);
    for (const id of featureIds) {
      const prefix = `${FEATURE_ROOT}${id}/`;
      if (normalized.includes(`/${prefix}`) || normalized.startsWith(prefix)) {
        violations.push({ feature: id, module: normalized });
      }
    }
  }
  return violations;
}

function collectChunkModules(bundle) {
  const moduleIds = new Set();
  let chunkCount = 0;
  for (const item of Object.values(bundle)) {
    if (item.type !== "chunk") continue;
    chunkCount += 1;
    for (const moduleId of item.moduleIds ?? []) moduleIds.add(moduleId);
    for (const moduleId of Object.keys(item.modules ?? {})) {
      moduleIds.add(moduleId);
    }
  }
  return { chunkCount, moduleIds: [...moduleIds] };
}

export function featureBundleGuard(featureIds) {
  return {
    name: "feature-bundle-guard",
    apply: "build",
    generateBundle(_options, bundle) {
      const { chunkCount, moduleIds } = collectChunkModules(bundle);
      if (chunkCount === 0) {
        throw new Error(
          "[feature-bundle-guard] 产物中没有 chunk，断言失去基准",
        );
      }
      if (moduleIds.length === 0) {
        throw new Error(
          "[feature-bundle-guard] 未解析到任何模块 id，断言失去基准",
        );
      }
      if (featureIds.length === 0) return;
      const violations = findFeatureModules(moduleIds, featureIds);
      if (violations.length > 0) {
        const detail = violations
          .map((item) => `${item.feature} <- ${item.module}`)
          .join("; ");
        throw new Error(
          `[feature-bundle-guard] 关闭态产物仍含特性模块: ${detail}`,
        );
      }
    },
  };
}
