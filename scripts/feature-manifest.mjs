#!/usr/bin/env node
// 特性清单派生：`config/features.json` 是唯一真源，
// `vite.config.ts` 与 `vitest.config.ts` 共用本模块，避免出现第二份派生规则。

import fs from "node:fs";
import process from "node:process";

/**
 * @typedef {object} FeatureSpec
 * @property {boolean} default
 * @property {string} [cargoFeature]
 * @property {string} [env]
 */

/**
 * @typedef {object} FeatureEntry
 * @property {string} id
 * @property {FeatureSpec} spec
 */

/** @typedef {Record<string, string | undefined>} FeatureEnv */

const FEATURE_ID_PATTERN = /^[a-z][a-z0-9_]*$/;
const FEATURE_KEYS = ["default", "cargoFeature", "env"];

/**
 * @param {string} id
 * @returns {string}
 */
export function featureEnv(id) {
  return `${id.toUpperCase()}_ENABLED`;
}

/**
 * @param {string} manifestPath
 * @returns {FeatureEntry[]}
 */
export function loadFeatureManifest(manifestPath) {
  /** @type {Record<string, FeatureSpec>} */
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
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

/**
 * @param {string} id
 * @param {FeatureSpec} spec
 * @param {FeatureEnv} [env]
 * @returns {boolean}
 */
export function featureEnabled(id, spec, env = process.env) {
  const override = env[featureEnv(id)];
  return override === undefined ? spec.default : override !== "0";
}

/**
 * @param {FeatureEntry[]} entries
 * @param {FeatureEnv} [env]
 * @returns {Record<string, string>}
 */
export function featureDefines(entries, env = process.env) {
  /** @type {Record<string, string>} */
  const defines = {};
  for (const { id, spec } of entries) {
    defines[`__${featureEnv(id)}__`] = JSON.stringify(
      featureEnabled(id, spec, env),
    );
  }
  return defines;
}

/**
 * @param {FeatureEntry[]} entries
 * @param {FeatureEnv} [env]
 * @returns {string[]}
 */
export function disabledFeatureIds(entries, env = process.env) {
  return entries
    .filter(({ id, spec }) => !featureEnabled(id, spec, env))
    .map(({ id }) => id);
}
