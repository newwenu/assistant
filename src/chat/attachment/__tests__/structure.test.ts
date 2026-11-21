import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";

const MODULE_ROOT = path.resolve(import.meta.dirname, "..");
const SOURCES_DIR = path.join(MODULE_ROOT, "sources");
const INTAKE_FILE = path.join(MODULE_ROOT, "intake.ts");
const FACTORY_FILE = path.join(MODULE_ROOT, "factory.ts");
const PATHS_FILE = path.join(MODULE_ROOT, "paths.ts");
const ASSET_DIRS_FILE = path.resolve(
  MODULE_ROOT,
  "..",
  "..",
  "services",
  "desktop",
  "assetDirs.ts",
);

const CREATED_SOURCE_FILE_NAMES = ["paste", "filePicker", "dragDrop"];
const FACTORY_EXPORT_NAMES = [
  "createImageAttachmentFromFile",
  "createPathAttachment",
];

function read(file: string): string {
  return fs.readFileSync(file, "utf8");
}

describe("attachment sources wiring", () => {
  it("keeps the source set discoverable in one directory", () => {
    const files = fs
      .readdirSync(SOURCES_DIR, { withFileTypes: true })
      .filter((e) => e.isFile() && e.name.endsWith(".ts"))
      .map((e) => e.name.replace(/\.ts$/, ""))
      .sort();
    expect(files).toEqual([...CREATED_SOURCE_FILE_NAMES].sort());
  });

  it("wires every source into intake", () => {
    const intake = read(INTAKE_FILE);
    for (const name of CREATED_SOURCE_FILE_NAMES) {
      expect(intake).toContain(`./sources/${name}`);
    }
  });

  it("keeps sources free of store dependencies", () => {
    for (const name of CREATED_SOURCE_FILE_NAMES) {
      const content = read(path.join(SOURCES_DIR, `${name}.ts`));
      expect(content).not.toContain("stores/");
    }
  });
});

describe("attachment factory matrix", () => {
  it("exposes the file factories plus the single path factory", () => {
    const factory = read(FACTORY_FILE);
    for (const name of FACTORY_EXPORT_NAMES) {
      expect(factory).toMatch(
        new RegExp(`export (async )?function ${name}\\(`),
      );
    }
    expect(factory).not.toContain("createImageAttachmentFromPath");
    expect(factory).not.toContain("createFileAttachmentFromPath");
  });

  it("no longer references the removed core attachment modules", () => {
    const files = fs
      .readdirSync(MODULE_ROOT, { withFileTypes: true })
      .filter((e) => e.isFile() && e.name.endsWith(".ts"))
      .map((e) => path.join(MODULE_ROOT, e.name));
    for (const file of files) {
      expect(read(file)).not.toContain("core/attachment");
    }
  });
});

describe("attachment paths single join point", () => {
  it("exports only the URL helpers", () => {
    const paths = read(PATHS_FILE);
    expect(paths).toContain("export async function attachmentUrl(");
    expect(paths).toContain("export async function openAttachmentFile(");
    expect(paths).not.toContain("mascotAssetUrl");
    expect(paths).not.toContain("isLocalAssetUrl");
    expect(paths).not.toContain("getAttachmentsDir");
  });

  it("keeps exactly one convertFileSrc call", () => {
    const assetDirs = read(ASSET_DIRS_FILE);
    const hits = assetDirs.match(/convertFileSrc\(/g) ?? [];
    expect(hits.length).toBe(1);
  });

  it("takes directory names from the backend instead of literals", () => {
    const assetDirs = read(ASSET_DIRS_FILE);
    expect(assetDirs).toContain("getAssetDirs");
    expect(assetDirs).not.toContain('"attachments"');
  });
});

describe("attachment limits single source", () => {
  const LIMITS_FILE = path.join(MODULE_ROOT, "limits.ts");

  function moduleFiles(): string[] {
    return fs
      .readdirSync(MODULE_ROOT, { withFileTypes: true })
      .filter((e) => e.isFile() && e.name.endsWith(".ts"))
      .map((e) => path.join(MODULE_ROOT, e.name))
      .filter((file) => file !== LIMITS_FILE);
  }

  it("reads the limits from the backend DTO", () => {
    const limits = read(LIMITS_FILE);
    expect(limits).toContain("getAttachmentLimits");
    expect(limits).toContain("export function attachmentLimits(");
  });

  it("holds no numeric limit literal outside the limits module", () => {
    for (const file of moduleFiles()) {
      expect(read(file)).not.toMatch(/\b(8|16)\s*\*\s*1024\s*\*\s*1024/);
      expect(read(file)).not.toMatch(/ATTACHMENT_MAX_/);
    }
  });
});
