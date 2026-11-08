import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";

const MODULE_ROOT = path.resolve(import.meta.dirname, "..");
const SOURCES_DIR = path.join(MODULE_ROOT, "sources");
const INTAKE_FILE = path.join(MODULE_ROOT, "intake.ts");
const FACTORY_FILE = path.join(MODULE_ROOT, "factory.ts");

const CREATED_SOURCE_FILE_NAMES = ["paste", "filePicker", "dragDrop"];
const FACTORY_EXPORT_NAMES = [
  "createImageAttachmentFromFile",
  "createImageAttachmentFromPath",
  "createFileAttachmentFromFile",
  "createFileAttachmentFromPath",
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
  it("exposes the full 2x2 create matrix", () => {
    const factory = read(FACTORY_FILE);
    for (const name of FACTORY_EXPORT_NAMES) {
      expect(factory).toContain(`export function ${name}(`);
    }
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
