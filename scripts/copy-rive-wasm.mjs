#!/usr/bin/env node
/**
 * 把 @rive-app/webgl2 的 wasm 运行时同步到 public/。
 *
 * 为什么不用 vite-plugin-static-copy：
 *   public/ 是 Vite 的 publicDir，dev 阶段直接以 /rive.wasm 提供服务，
 *   build 阶段自动整目录拷进 dist/。插件再从 node_modules 拷一份到 dist/，
 *   只是把同样的两个文件搬运两遍；而且源文件缺失时插件会直接抛错中断构建。
 *
 * 用法（package.json 的 prebuild 已自动调用）：
 *   node scripts/copy-rive-wasm.mjs
 *
 * dev 不跑这个脚本：dev 阶段 Vite 直接从 public/ 提供 /rive.wasm，
 * 仓库里已入库的这两个文件足够用；只有打包时才需要确保它们与
 * 当前安装的 @rive-app/webgl2 版本一致。
 *
 * 行为：
 *   - 源文件不存在 → 报错退出，提示先 npm install
 *   - public/ 已有且 SHA256 相同 → 跳过，不触碰 mtime，避免无意义的文件变动
 *   - 否则覆盖写入 public/
 */
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { copyFile, mkdir, readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const SOURCE_DIR = join(ROOT, "node_modules", "@rive-app", "webgl2");
const DEST_DIR = join(ROOT, "public");
const FILES = ["rive.wasm", "rive_fallback.wasm"];

function digest(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}

function formatBytes(bytes) {
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

async function syncOne(name) {
  const source = join(SOURCE_DIR, name);
  const dest = join(DEST_DIR, name);

  if (!existsSync(source)) {
    throw new Error(
      `缺少 ${source}，请先在仓库根执行 npm install 安装 @rive-app/webgl2。`,
    );
  }

  const sourceBuffer = await readFile(source);

  if (existsSync(dest)) {
    const destBuffer = await readFile(dest);
    if (digest(destBuffer) === digest(sourceBuffer)) {
      console.log(`[rive-wasm] ${name} 已是最新，跳过`);
      return;
    }
  }

  await copyFile(source, dest);
  console.log(
    `[rive-wasm] ${name} 已同步（${formatBytes(sourceBuffer.length)}）`,
  );
}

async function main() {
  await mkdir(DEST_DIR, { recursive: true });
  for (const name of FILES) {
    await syncOne(name);
  }
}

main().catch((error) => {
  console.error(`[rive-wasm] 同步失败：${error.message}`);
  process.exitCode = 1;
});
