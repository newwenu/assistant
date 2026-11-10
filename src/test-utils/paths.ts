import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "../..");

export function repoRoot(): string {
  return ROOT;
}

export function repoPath(relative: string): string {
  return path.join(ROOT, relative);
}

export function readRepoFile(relative: string): string {
  return fs.readFileSync(repoPath(relative), "utf8");
}

export function repoFileExists(relative: string): boolean {
  return fs.existsSync(repoPath(relative));
}
