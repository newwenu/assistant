import { describe, expect, it } from "vitest";
import { readRepoFile } from "@/test-utils/paths";
import {
  DETAIL_CLOSED_TAGS,
  DETAIL_EXPORT_FORMATS,
  parseAuditDetail,
} from "../detail";

const ENTRY_ID = "3f2a9c1e-0b6d-4f8a-9d3e-5c7b1a2e4f60";
const OTHER_ID = "8c4d1e2f-3a5b-4c6d-8e9f-0a1b2c3d4e5f";
const DIGEST =
  "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

function closedTagsFromBackend(): string[] {
  const source = readRepoFile(
    "src-tauri/crates/assistant-vault/src/audit/mod.rs",
  );
  const match = /const DETAIL_CLOSED_TAGS: \[&str; \d+\] = \[([^\]]+)\];/.exec(
    source,
  );
  expect(match).not.toBeNull();
  return Array.from(
    (match as RegExpExecArray)[1].matchAll(/"([^"]+)"/g),
    (m) => m[1],
  );
}

function exportFormatsFromBackend(): string[] {
  const source = readRepoFile(
    "src-tauri/crates/assistant-vault/src/audit/mod.rs",
  );
  const match =
    /const DETAIL_EXPORT_FORMATS: \[&str; \d+\] = \[([^\]]+)\];/.exec(source);
  expect(match).not.toBeNull();
  return Array.from(
    (match as RegExpExecArray)[1].matchAll(/"([^"]+)"/g),
    (m) => m[1],
  );
}

describe("detail 文法解析（§2.4）", () => {
  it("封闭标签集合与后端 DETAIL_CLOSED_TAGS 对齐", () => {
    expect(closedTagsFromBackend()).toEqual([...DETAIL_CLOSED_TAGS]);
    expect(exportFormatsFromBackend()).toEqual([...DETAIL_EXPORT_FORMATS]);
  });

  it("空串与 null 归为空详情", () => {
    expect(parseAuditDetail(null)).toEqual({ kind: "empty" });
    expect(parseAuditDetail(undefined)).toEqual({ kind: "empty" });
    expect(parseAuditDetail("")).toEqual({ kind: "empty" });
  });

  it("纯计数", () => {
    expect(parseAuditDetail("12")).toEqual({ kind: "count", count: 12 });
  });

  it("单条动作 {entryId}", () => {
    expect(parseAuditDetail(ENTRY_ID)).toEqual({
      kind: "entryId",
      entryId: ENTRY_ID,
    });
  });

  it("{entryId}@{suffix} 按首个 @ 切分，suffix 可再含 @", () => {
    expect(parseAuditDetail(`${ENTRY_ID}@remote`)).toEqual({
      kind: "entryWithSuffix",
      entryId: ENTRY_ID,
      suffix: "remote",
    });
    expect(parseAuditDetail(`${ENTRY_ID}@HEAD@{1}`)).toEqual({
      kind: "entryWithSuffix",
      entryId: ENTRY_ID,
      suffix: "HEAD@{1}",
    });
  });

  it("{tag}:{entryId} 仅对封闭标签 totp 生效", () => {
    expect(parseAuditDetail(`totp:${ENTRY_ID}`)).toEqual({
      kind: "taggedEntry",
      tag: "totp",
      entryId: ENTRY_ID,
    });
  });

  it("{entryId}:{count} 用于 hibp_leak_found，不被解析成 tag:entryId", () => {
    expect(parseAuditDetail(`${ENTRY_ID}:3`)).toEqual({
      kind: "entryCount",
      entryId: ENTRY_ID,
      count: 3,
    });
  });

  it("conflict 固定 5 段", () => {
    expect(
      parseAuditDetail(`conflict:${ENTRY_ID}:remote:password:${DIGEST}`),
    ).toEqual({
      kind: "conflict",
      entryId: ENTRY_ID,
      side: "remote",
      field: "password",
      digest: DIGEST,
    });
  });

  it("conflict 段数不为 5 时原样兜底，不退化成 entryId:count", () => {
    const raw = `conflict:${ENTRY_ID}:remote:password`;
    expect(parseAuditDetail(raw)).toEqual({ kind: "raw", raw });
  });

  it("导出三元组与失败四元组", () => {
    expect(parseAuditDetail(`json:5:${DIGEST}`)).toEqual({
      kind: "export",
      format: "json",
      count: 5,
      digest: DIGEST,
      failed: false,
    });
    expect(parseAuditDetail(`csv:5:${DIGEST}:failed`)).toEqual({
      kind: "export",
      format: "csv",
      count: 5,
      digest: DIGEST,
      failed: true,
    });
  });

  it("导出格式首段但段数或尾段不合法时原样兜底", () => {
    expect(parseAuditDetail(`json:${ENTRY_ID}:${DIGEST}`)).toEqual({
      kind: "raw",
      raw: `json:${ENTRY_ID}:${DIGEST}`,
    });
    expect(parseAuditDetail(`json:5:${DIGEST}:ok`)).toEqual({
      kind: "raw",
      raw: `json:5:${DIGEST}:ok`,
    });
  });

  it("dek_rotated 三段计数", () => {
    expect(parseAuditDetail("1:2:37")).toEqual({
      kind: "dekRotated",
      oldVersion: 1,
      newVersion: 2,
      count: 37,
    });
  });

  it("SSH 指纹 SHA256:{digest}", () => {
    expect(parseAuditDetail("SHA256:abc+/=")).toEqual({
      kind: "fingerprint",
      keyType: "SHA256",
      digest: "abc+/=",
    });
    expect(parseAuditDetail("SHA256:bad digest")).toEqual({
      kind: "raw",
      raw: "SHA256:bad digest",
    });
  });

  it("封闭标签原样作为标签展示", () => {
    expect(parseAuditDetail("wrong_password")).toEqual({
      kind: "label",
      label: "wrong_password",
    });
  });

  it("unknown 哨兵不产出条目 ID", () => {
    expect(parseAuditDetail("unknown")).toEqual({
      kind: "label",
      label: "unknown",
    });
    expect(parseAuditDetail("unknown@remote")).toEqual({
      kind: "entryWithSuffix",
      entryId: null,
      suffix: "remote",
    });
    expect(parseAuditDetail(`totp:unknown`)).toEqual({
      kind: "taggedEntry",
      tag: "totp",
      entryId: null,
    });
    expect(parseAuditDetail(`unknown:2`)).toEqual({
      kind: "entryCount",
      entryId: null,
      count: 2,
    });
  });

  it("畸形串原样兜底，不抛错也不留空", () => {
    const raw = `自由文本 ${OTHER_ID} 含空格`;
    expect(parseAuditDetail(raw)).toEqual({ kind: "label", label: raw });
    expect(parseAuditDetail("a:b:c:d")).toEqual({
      kind: "raw",
      raw: "a:b:c:d",
    });
  });
});
