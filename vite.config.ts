import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "node:path";
import process from "node:process";
import { featureBundleGuard } from "./scripts/feature-bundle-guard.mjs";
import {
  disabledFeatureIds as computeDisabledFeatureIds,
  featureDefines,
  loadFeatureManifest,
} from "./scripts/feature-manifest.mjs";

const host = process.env.TAURI_DEV_HOST;

const featureEntries = loadFeatureManifest(
  resolve(import.meta.dirname, "config/features.json"),
);
const disabledFeatureIds = computeDisabledFeatureIds(featureEntries);

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [
    react(),
    featureBundleGuard(disabledFeatureIds, ["src/features/", "src/tools/"]),
  ],
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "src"),
    },
  },
  define: {
    __BUILD_DATE__: JSON.stringify(new Date().toISOString().slice(0, 10)),
    ...featureDefines(featureEntries),
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
