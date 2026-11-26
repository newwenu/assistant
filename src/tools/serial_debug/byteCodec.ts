export type LineEnding = "none" | "cr" | "lf" | "crlf";

export type SerialEncoding = "utf8" | "gbk" | "latin1";

export type HexParseResult =
  { ok: true; bytes: Uint8Array } | { ok: false; reason: "odd" | "invalid" };

const LINE_ENDING_BYTES: Record<LineEnding, string> = {
  none: "",
  cr: "\r",
  lf: "\n",
  crlf: "\r\n",
};

export function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let index = 0; index < bytes.length; index += 1) {
    binary += String.fromCharCode(bytes[index] ?? 0);
  }
  return btoa(binary);
}

export function base64ToBytes(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

export function textToBase64(text: string): string {
  return bytesToBase64(new TextEncoder().encode(text));
}

export function formatHex(bytes: Uint8Array, separator = " "): string {
  const parts: string[] = [];
  for (let index = 0; index < bytes.length; index += 1) {
    parts.push((bytes[index] ?? 0).toString(16).padStart(2, "0").toUpperCase());
  }
  return parts.join(separator);
}

export function parseHex(input: string): HexParseResult {
  const stripped = input.replace(/0[xX]/g, "").replace(/[\s,]/g, "");
  if (stripped.length === 0) return { ok: true, bytes: new Uint8Array(0) };
  if (!/^[0-9a-fA-F]+$/.test(stripped)) return { ok: false, reason: "invalid" };
  if (stripped.length % 2 !== 0) return { ok: false, reason: "odd" };
  const bytes = new Uint8Array(stripped.length / 2);
  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = Number.parseInt(
      stripped.slice(index * 2, index * 2 + 2),
      16,
    );
  }
  return { ok: true, bytes };
}

export function appendLineEnding(text: string, lineEnding: LineEnding): string {
  return text + LINE_ENDING_BYTES[lineEnding];
}

export function decodeLatin1(bytes: Uint8Array): string {
  let text = "";
  for (let index = 0; index < bytes.length; index += 1) {
    text += String.fromCharCode(bytes[index] ?? 0);
  }
  return text;
}

const DECODER_LABELS: Record<SerialEncoding, string> = {
  utf8: "utf-8",
  gbk: "gbk",
  latin1: "iso-8859-1",
};

export function decodeText(
  bytes: Uint8Array,
  encoding: SerialEncoding = "utf8",
): string {
  if (encoding === "latin1") return decodeLatin1(bytes);
  try {
    return new TextDecoder(DECODER_LABELS[encoding], { fatal: true }).decode(
      bytes,
    );
  } catch {
    return decodeLatin1(bytes);
  }
}

export function formatTimestamp(atMs: number): string {
  const date = new Date(atMs);
  const pad = (value: number, width = 2) => String(value).padStart(width, "0");
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(
    date.getSeconds(),
  )}.${pad(date.getMilliseconds(), 3)}`;
}
