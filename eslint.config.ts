import js from "@eslint/js";
import globals from "globals";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import prettierConfig from "eslint-config-prettier";
import { defineConfig } from "eslint/config";
import type { Linter } from "eslint";

const BARREL_DOMAINS = ["history", "mascot", "vault"];

const MODULE_REFERENCE_SELECTOR =
  ":matches(ImportDeclaration, ExportNamedDeclaration, ExportAllDeclaration)";

const EXPLICIT_INDEX_SELECTOR =
  MODULE_REFERENCE_SELECTOR + "[source.value=/\\/index(\\.tsx?)?$/]";

const selfBarrelSelector = (domain: string) =>
  MODULE_REFERENCE_SELECTOR +
  `[source.value=/^(@\\/${domain}|\\.\\.(\\/\\.\\.)*)$/]`;

const EXPLICIT_INDEX_MESSAGE =
  "禁止显式引用 index 文件：相对路径直连具体模块，跨域用 `<domain>` 目录写法。";

const selfBarrelMessage = (domain: string) =>
  `域内文件禁止经自身 barrel 引用（${domain}/index）：直连具体模块，避免 barrel 回环。`;

const INDEX_FILE_GUARD: Linter.Config = {
  files: ["src/**/index.ts", "src/**/index.tsx"],
  rules: {
    "no-restricted-syntax": [
      "error",
      { selector: EXPLICIT_INDEX_SELECTOR, message: EXPLICIT_INDEX_MESSAGE },
      {
        selector: "ExportAllDeclaration",
        message: "barrel 禁止 export *：逐项列出导出面，避免隐式扩大公开 API。",
      },
    ],
  },
};

export default defineConfig(
  {
    ignores: [
      "dist",
      "src-tauri",
      "src/vault/core/contract",
      "src/types/protocol.contract.ts",
      "src/types/shortcutDefaults.generated.ts",
      "references",
      "temp-backup",
      "for_generate",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommendedTypeChecked,
  prettierConfig,
  {
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    plugins: {
      "react-hooks": { rules: reactHooks.rules },
    },
    rules: {
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "warn",
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_" },
      ],
      "@typescript-eslint/no-explicit-any": "warn",
    },
  },
  {
    files: ["src/**/*.ts", "src/**/*.tsx"],
    ignores: ["**/__tests__/**"],
    rules: {
      "@typescript-eslint/no-deprecated": "warn",
    },
  },
  {
    files: ["scripts/**/*.mjs", "scripts/**/*.cjs"],
    ...tseslint.configs.disableTypeChecked,
    languageOptions: {
      globals: globals.node,
    },
    rules: {
      ...tseslint.configs.disableTypeChecked.rules,
      "react-hooks/rules-of-hooks": "off",
      "react-hooks/exhaustive-deps": "off",
    },
  },
  {
    files: ["scripts/**/*.cjs"],
    rules: {
      "@typescript-eslint/no-require-imports": "off",
    },
  },
  {
    files: ["src/**/*.ts", "src/**/*.tsx"],
    rules: {
      "no-restricted-syntax": [
        "error",
        { selector: EXPLICIT_INDEX_SELECTOR, message: EXPLICIT_INDEX_MESSAGE },
      ],
    },
  },
  ...BARREL_DOMAINS.map((domain): Linter.Config => ({
    files: [`src/${domain}/**/*.ts`, `src/${domain}/**/*.tsx`],
    rules: {
      "no-restricted-syntax": [
        "error",
        { selector: EXPLICIT_INDEX_SELECTOR, message: EXPLICIT_INDEX_MESSAGE },
        {
          selector: selfBarrelSelector(domain),
          message: selfBarrelMessage(domain),
        },
      ],
    },
  })),
  INDEX_FILE_GUARD,
);
