import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "node:path";
import { readFileSync } from "node:fs";
import process from "node:process";

const host = process.env.TAURI_DEV_HOST;

interface FeatureSpec {
  default: boolean;
  cargoFeature: string;
  env: string;
}

const featureManifest = JSON.parse(
  readFileSync(resolve(import.meta.dirname, "config/features.json"), "utf8"),
) as Record<string, FeatureSpec>;

function featureDefines(): Record<string, string> {
  const defines: Record<string, string> = {};
  for (const spec of Object.values(featureManifest)) {
    const override = process.env[spec.env];
    const enabled = override === undefined ? spec.default : override !== "0";
    defines[`__${spec.env}__`] = JSON.stringify(enabled);
  }
  return defines;
}

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [react()],
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