import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "node:path";
import { readFileSync } from "node:fs";
import process from "node:process";
import { featureBundleGuard } from "./scripts/feature-bundle-guard.mjs";

const host = process.env.TAURI_DEV_HOST;

const FEATURE_ID_PATTERN = /^[a-z][a-z0-9_]*$/;
const FEATURE_KEYS = ["default", "cargoFeature", "env"];

interface FeatureSpec {
  default: boolean;
  cargoFeature?: string;
  env?: string;
}

function featureEnv(id: string): string {
  return `${id.toUpperCase()}_ENABLED`;
}

function loadFeatureManifest(): Array<{ id: string; spec: FeatureSpec }> {
  const manifest = JSON.parse(
    readFileSync(resolve(import.meta.dirname, "config/features.json"), "utf8"),
  ) as Record<string, FeatureSpec>;
  const entries = Object.entries(manifest);
  if (entries.length === 0) {
    throw new Error("config/features.json 未声明任何特性");
  }
  for (const [id, spec] of entries) {
    if (!FEATURE_ID_PATTERN.test(id)) {
      throw new Error(`特性 id '${id}' 必须匹配 ${FEATURE_ID_PATTERN.source}`);
    }
    for (const key of Object.keys(spec)) {
      if (!FEATURE_KEYS.includes(key)) {
        throw new Error(`特性 '${id}' 含未知键 '${key}'`);
      }
    }
    if (typeof spec.default !== "boolean") {
      throw new Error(`特性 '${id}' 的 default 必须是布尔值`);
    }
    if (spec.cargoFeature !== undefined && spec.cargoFeature !== id) {
      throw new Error(`特性 '${id}' 的 cargoFeature 必须等于 id`);
    }
    if (spec.env !== undefined && spec.env !== featureEnv(id)) {
      throw new Error(`特性 '${id}' 的 env 必须等于 ${featureEnv(id)}`);
    }
  }
  return entries.map(([id, spec]) => ({ id, spec }));
}

const featureEntries = loadFeatureManifest();

function featureEnabled(id: string, spec: FeatureSpec): boolean {
  const override = process.env[featureEnv(id)];
  return override === undefined ? spec.default : override !== "0";
}

function featureDefines(): Record<string, string> {
  const defines: Record<string, string> = {};
  for (const { id, spec } of featureEntries) {
    defines[`__${featureEnv(id)}__`] = JSON.stringify(featureEnabled(id, spec));
  }
  return defines;
}

const disabledFeatureIds = featureEntries
  .filter(({ id, spec }) => !featureEnabled(id, spec))
  .map(({ id }) => id);

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [react(), featureBundleGuard(disabledFeatureIds)],
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "src"),
    },
  },
  define: {
    __BUILD_DATE__: JSON.stringify(new Date().toISOString().slice(0, 10)),
    ...featureDefines(),
  },

  optimizeDeps: {
    entries: ["index.html"],
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri` and `references`
      ignored: ["**/src-tauri/**", "**/references/**"],
    },
  },
}));
