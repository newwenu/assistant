import { defineConfig } from "vitest/config";
import { resolve } from "node:path";
import {
  featureDefines,
  loadFeatureManifest,
} from "./scripts/feature-manifest.mjs";

const featureEntries = loadFeatureManifest(
  resolve(import.meta.dirname, "config/features.json"),
);

export default defineConfig({
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "src"),
    },
  },
  define: featureDefines(featureEntries),
  test: {
    include: ["src/**/*.test.{ts,tsx}", "scripts/**/*.test.mjs"],
    setupFiles: ["src/testSetup.ts"],
  },
});
