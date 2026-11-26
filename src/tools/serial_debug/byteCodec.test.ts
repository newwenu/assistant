import { describe, expect, it } from "vitest";
import {
  appendLineEnding,
  base64ToBytes,
  bytesToBase64,
  decodeText,
  formatHex,
  formatTimestamp,
  parseHex,
  textToBase64,
} from "./byteCodec";

describe("base64 round trip", () => {
  it("encodes and decodes bytes without loss", () => {
    const bytes = new Uint8Array([0x00, 0x41, 0x7f, 0x80, 0xff]);
    const base64 = bytesToBase64(bytes);
    expect(base64ToBytes(base64)).toEqual(bytes);
  });

  it("encodes text as UTF-8", () => {
    expect(base64ToBytes(textToBase64("串口"))).toEqual(
      new TextEncoder().encode("串口"),
    );
  });
});

describe("formatHex", () => {
  it("formats each byte as uppercase two-digit hex", () => {
    expect(formatHex(new Uint8Array([0x01, 0xab, 0xff]))).toBe("01 AB FF");
  });

  it("supports a custom separator", () => {
    expect(formatHex(new Uint8Array([0x01, 0x02]), "-")).toBe("01-02");
  });

  it("returns an empty string for empty input", () => {
    expect(formatHex(new Uint8Array([]))).toBe("");
  });
});

describe("parseHex", () => {
  it("accepts spaces, commas and 0x prefixes", () => {
    const result = parseHex("0x41, 42 ff");
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect([...result.bytes]).toEqual([0x41, 0x42, 0xff]);
    }
  });

  it("reports odd length", () => {
    expect(parseHex("ABC")).toEqual({ ok: false, reason: "odd" });
  });

  it("reports illegal characters", () => {
    expect(parseHex("41ZZ")).toEqual({ ok: false, reason: "invalid" });
  });

  it("parses an empty string as zero bytes", () => {
    const result = parseHex("   ");
    expect(result.ok).toBe(true);
    if (result.ok) expect(result.bytes.length).toBe(0);
  });
});

describe("appendLineEnding", () => {
  it("appends the requested ending", () => {
    expect(appendLineEnding("AT", "none")).toBe("AT");
    expect(appendLineEnding("AT", "cr")).toBe("AT\r");
    expect(appendLineEnding("AT", "lf")).toBe("AT\n");
    expect(appendLineEnding("AT", "crlf")).toBe("AT\r\n");
  });
});

describe("decodeText", () => {
  it("decodes UTF-8 text", () => {
    expect(decodeText(new TextEncoder().encode("你好"), "utf8")).toBe("你好");
  });

  it("falls back to Latin-1 for invalid UTF-8", () => {
    expect(decodeText(new Uint8Array([0xff, 0xfe]), "utf8")).toBe(
      "\u00ff\u00fe",
    );
  });

  it("decodes GBK text", () => {
    expect(decodeText(new Uint8Array([0xc4, 0xe3, 0xba, 0xc3]), "gbk")).toBe(
      "你好",
    );
  });

  it("decodes Latin-1 bytes directly", () => {
    expect(decodeText(new Uint8Array([0x41, 0xe9]), "latin1")).toBe("A\u00e9");
  });
});

describe("formatTimestamp", () => {
  it("formats with millisecond precision", () => {
    const atMs = new Date(2026, 0, 2, 3, 4, 5, 6).getTime();
    expect(formatTimestamp(atMs)).toBe("03:04:05.006");
  });
});
