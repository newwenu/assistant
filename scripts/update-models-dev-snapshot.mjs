#!/usr/bin/env node
/**
 * Update the bundled models.dev snapshot from the upstream API.
 *
 * Usage:
 *   node scripts/update-models-dev-snapshot.mjs            # fetch from https://models.dev/api.json
 *   node scripts/update-models-dev-snapshot.mjs --check     # validate only, no network
 *   node scripts/update-models-dev-snapshot.mjs --source-file <api.json>  # use local file
 *
 * This is a checked-in generator (like fetch-fonts.mjs), not a build step.
 * Re-run when models.dev publishes new models that should be available
 * without a runtime refresh.
 *
 * Adapted from references/OpenBitFun/scripts/update-models-dev-snapshot.mjs.
 * Simplified: no provider overlay裁剪, full snapshot embed (V1).
 */

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  existsSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const ASSET_DIR = join(
  ROOT,
  "src-tauri",
  "crates",
  "assistant-providers",
  "assets",
);
const SNAPSHOT_PATH = join(ASSET_DIR, "models-dev.json");
const PROVENANCE_PATH = join(ASSET_DIR, "models-dev.provenance.json");
const LICENSE_PATH = join(ASSET_DIR, "models-dev.LICENSE.txt");
const API_URL = "https://models.dev/api.json";
const REPOSITORY = "https://github.com/anomalyco/models.dev";
const REPOSITORY_BRANCH = "dev";

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function serializedJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function optionValue(args, name) {
  const inlinePrefix = `${name}=`;
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === name) {
      return args[index + 1];
    }
    if (args[index].startsWith(inlinePrefix)) {
      return args[index].slice(inlinePrefix.length);
    }
  }
  return undefined;
}

function copyDefinedFields(source, fields) {
  return Object.fromEntries(
    fields
      .filter((field) => source[field] !== undefined && source[field] !== null)
      .map((field) => [field, source[field]]),
  );
}

function normalizedOptions(value) {
  if (Array.isArray(value)) return value;
  if (value && typeof value === "object") return [value];
  return [];
}

function copyModel(sourceModel, modelId) {
  const model = {
    id: sourceModel.id || modelId,
    ...copyDefinedFields(sourceModel, [
      "name",
      "description",
      "family",
      "status",
      "release_date",
      "last_updated",
      "knowledge",
      "open_weights",
      "attachment",
      "reasoning",
      "tool_call",
      "structured_output",
    ]),
  };
  if (Object.hasOwn(sourceModel, "reasoning_options")) {
    model.reasoning_options = normalizedOptions(sourceModel.reasoning_options);
  }
  if (sourceModel.modalities && typeof sourceModel.modalities === "object") {
    model.modalities = copyDefinedFields(sourceModel.modalities, [
      "input",
      "output",
    ]);
  }
  if (sourceModel.limit && typeof sourceModel.limit === "object") {
    model.limit = copyDefinedFields(sourceModel.limit, [
      "context",
      "input",
      "output",
    ]);
  }
  if (sourceModel.cost && typeof sourceModel.cost === "object") {
    model.cost = copyDefinedFields(sourceModel.cost, [
      "input",
      "output",
      "cache_read",
      "cache_write",
    ]);
  }
  return model;
}

function buildSnapshot(source) {
  assert(
    source && typeof source === "object" && !Array.isArray(source),
    "models.dev source must be a provider object",
  );

  const snapshot = {};
  const providerIds = Object.keys(source).sort();

  for (const providerId of providerIds) {
    const sourceProvider = source[providerId];
    const sourceModels = sourceProvider.models;
    if (!sourceModels || typeof sourceModels !== "object") continue;

    const modelEntries = Object.entries(sourceModels).sort(([l], [r]) =>
      l < r ? -1 : l > r ? 1 : 0,
    );
    if (modelEntries.length === 0) continue;

    const provider = {
      id: sourceProvider.id || providerId,
      name: sourceProvider.name || providerId,
      ...copyDefinedFields(sourceProvider, ["api", "doc", "env"]),
      models: {},
    };
    for (const [modelId, sourceModel] of modelEntries) {
      if (!sourceModel || typeof sourceModel !== "object") continue;
      provider.models[modelId] = copyModel(sourceModel, modelId);
    }
    snapshot[providerId] = provider;
  }
  return snapshot;
}

function snapshotModelCounts(snapshot) {
  return Object.fromEntries(
    Object.entries(snapshot).map(([providerId, provider]) => [
      providerId,
      Object.keys(provider.models || {}).length,
    ]),
  );
}

function validateProvenance(provenance, snapshotText, licenseText) {
  assert.equal(provenance.schema_version, 1, "unsupported provenance schema");
  assert.equal(
    provenance.artifact?.sha256,
    sha256(snapshotText),
    "bundled snapshot hash does not match provenance",
  );
  assert.equal(
    provenance.artifact?.bytes,
    Buffer.byteLength(snapshotText),
    "bundled snapshot byte count does not match provenance",
  );
  assert.equal(provenance.source?.project, "models.dev");
  assert.equal(provenance.source?.api_url, API_URL);
  assert.equal(provenance.license?.spdx, "MIT");
  assert.equal(
    provenance.license?.sha256,
    sha256(licenseText),
    "license hash does not match provenance",
  );
  assert.match(licenseText, /^MIT License\r?\n/);
  assert.match(licenseText, /Copyright \(c\) 2025 models\.dev/);
}

function checkAssets() {
  for (const path of [SNAPSHOT_PATH, LICENSE_PATH]) {
    assert(
      existsSync(path),
      `required models.dev asset is missing: ${path}`,
    );
  }

  const snapshotText = readFileSync(SNAPSHOT_PATH, "utf8");
  const licenseText = readFileSync(LICENSE_PATH, "utf8");

  if (existsSync(PROVENANCE_PATH)) {
    const provenance = JSON.parse(readFileSync(PROVENANCE_PATH, "utf8"));
    validateProvenance(provenance, snapshotText, licenseText);
  }

  const snapshot = JSON.parse(snapshotText);
  const counts = snapshotModelCounts(snapshot);
  const totalModels = Object.values(counts).reduce((a, b) => a + b, 0);
  const providerCount = Object.keys(counts).length;

  console.log(
    `models.dev snapshot valid: ${providerCount} providers, ${totalModels} models.`,
  );
}

async function fetchBytes(url) {
  let response;
  try {
    response = await fetch(url);
  } catch (error) {
    throw new Error(
      `Failed to fetch ${url}: ${error.message || String(error)}`,
    );
  }
  if (!response.ok) {
    throw new Error(`Failed to fetch ${url}: HTTP ${response.status}`);
  }
  return {
    bytes: Buffer.from(await response.arrayBuffer()),
    etag: response.headers.get("etag"),
  };
}

async function resolveRepositoryRevision(explicitRevision) {
  if (explicitRevision) {
    assert.match(
      explicitRevision,
      /^[a-f0-9]{40}$/,
      "--repository-revision must be a full lowercase commit SHA",
    );
    return explicitRevision;
  }
  const response = await fetch(
    `https://api.github.com/repos/anomalyco/models.dev/commits/${REPOSITORY_BRANCH}`,
    {
      headers: {
        Accept: "application/vnd.github+json",
        "User-Agent": "assistant-models-dev-updater",
      },
    },
  );
  if (!response.ok) {
    throw new Error(
      `Failed to resolve models.dev revision: HTTP ${response.status}`,
    );
  }
  const document = await response.json();
  assert.match(
    document.sha || "",
    /^[a-f0-9]{40}$/,
    "GitHub returned an invalid models.dev revision",
  );
  return document.sha;
}

async function updateAssets(args) {
  const sourceFile = optionValue(args, "--source-file");
  const explicitRevision = optionValue(args, "--repository-revision");
  const revision = await resolveRepositoryRevision(explicitRevision);

  let sourceBytes;
  let etag = null;

  if (sourceFile) {
    sourceBytes = readFileSync(resolve(sourceFile));
  } else {
    const fetched = await fetchBytes(API_URL);
    sourceBytes = fetched.bytes;
    etag = fetched.etag;
  }

  const licenseFile = optionValue(args, "--license-file");
  let upstreamLicenseText;
  if (licenseFile) {
    upstreamLicenseText = readFileSync(resolve(licenseFile), "utf8");
  } else if (existsSync(LICENSE_PATH)) {
    upstreamLicenseText = readFileSync(LICENSE_PATH, "utf8");
    try {
      const fetched = await fetchBytes(
        `https://raw.githubusercontent.com/anomalyco/models.dev/${revision}/LICENSE`,
      );
      const remoteText = fetched.bytes.toString("utf8");
      if (remoteText !== upstreamLicenseText) {
        console.warn(
          "Upstream models.dev license changed. Review and update manually if needed.",
        );
        console.warn(
          "Keeping local license. Use --license-file to override.",
        );
      }
    } catch {
      console.warn(
        "Could not fetch upstream license; keeping local copy.",
      );
    }
  } else {
    const fetched = await fetchBytes(
      `https://raw.githubusercontent.com/anomalyco/models.dev/${revision}/LICENSE`,
    );
    upstreamLicenseText = fetched.bytes.toString("utf8");
    writeFileSync(LICENSE_PATH, upstreamLicenseText, "utf8");
    console.log("Wrote models-dev.LICENSE.txt");
  }

  assert.match(upstreamLicenseText, /^MIT License\r?\n/);
  assert.match(upstreamLicenseText, /Copyright \(c\) 2025 models\.dev/);

  const source = JSON.parse(sourceBytes.toString("utf8"));
  const snapshot = buildSnapshot(source);
  const snapshotText = serializedJson(snapshot);

  const provenance = {
    schema_version: 1,
    artifact: {
      path: "src-tauri/crates/assistant-providers/assets/models-dev.json",
      sha256: sha256(snapshotText),
      bytes: Buffer.byteLength(snapshotText),
    },
    source: {
      project: "models.dev",
      api_url: API_URL,
      api_sha256: sha256(sourceBytes),
      api_bytes: sourceBytes.length,
      api_etag: etag,
      retrieved_at: new Date().toISOString(),
      repository: REPOSITORY,
      repository_branch: REPOSITORY_BRANCH,
      repository_revision: revision,
      revision_role:
        "Upstream dev revision observed when the API document was retrieved; api_sha256 is authoritative for the retrieved source bytes.",
    },
    license: {
      spdx: "MIT",
      copyright: "Copyright (c) 2025 models.dev",
      upstream_path: "LICENSE",
      source_path:
        "src-tauri/crates/assistant-providers/assets/models-dev.LICENSE.txt",
      sha256: sha256(upstreamLicenseText),
    },
    transformation: {
      kind: "full_snapshot_embed",
      description:
        "Full models.dev snapshot with deterministic field selection and pretty-printed JSON. No provider overlay裁剪; all providers included.",
      retained_provider_fields: ["id", "name", "api", "doc", "env"],
      retained_model_fields: [
        "id",
        "name",
        "description",
        "family",
        "status",
        "release_date",
        "last_updated",
        "knowledge",
        "open_weights",
        "attachment",
        "reasoning",
        "tool_call",
        "structured_output",
        "reasoning_options",
        "modalities.input",
        "modalities.output",
        "limit.context",
        "limit.input",
        "limit.output",
        "cost.input",
        "cost.output",
        "cost.cache_read",
        "cost.cache_write",
      ],
      included_provider_ids: Object.keys(snapshot).sort(),
      included_provider_model_counts: snapshotModelCounts(snapshot),
    },
    maintenance: {
      update_command: "npm run models-dev:update",
      check_command: "npm run models-dev:check",
    },
  };

  writeFileSync(SNAPSHOT_PATH, snapshotText, "utf8");
  writeFileSync(PROVENANCE_PATH, serializedJson(provenance), "utf8");
  checkAssets();
  console.log(`Updated bundled models.dev snapshot from revision ${revision}.`);
}

const args = process.argv.slice(2);
if (args.includes("--help")) {
  console.log(`Usage:
  npm run models-dev:update
  npm run models-dev:update -- --source-file <api.json> --license-file <LICENSE> --repository-revision <sha>
  npm run models-dev:check`);
} else if (args.includes("--check")) {
  checkAssets();
} else {
  updateAssets(args).catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}