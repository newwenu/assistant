import {
  MD5,
  SHA1,
  SHA256,
  SHA224,
  SHA512,
  SHA384,
  SHA3,
  RIPEMD160,
  enc,
  type lib,
} from "crypto-js";

export const ALGORITHMS = {
  MD5,
  SHA1,
  SHA256,
  SHA224,
  SHA512,
  SHA384,
  SHA3,
  RIPEMD160,
} as const;

export type AlgoName = keyof typeof ALGORITHMS;
export type Encoding = "Hex" | "Base64" | "Base64url" | "Bin";

export const ALGO_NAMES = Object.keys(ALGORITHMS) as AlgoName[];

export const ENCODING_OPTIONS: { value: Encoding; label: string }[] = [
  { value: "Hex", label: "十六进制 (Hex)" },
  { value: "Base64", label: "Base64" },
  { value: "Base64url", label: "Base64url" },
  { value: "Bin", label: "二进制 (Binary)" },
];

export function hexToBin(hex: string): string {
  return hex
    .trim()
    .split("")
    .map((byte) => Number.parseInt(byte, 16).toString(2).padStart(4, "0"))
    .join("");
}

export function formatOutput(words: lib.WordArray, encoding: Encoding): string {
  if (encoding === "Bin") {
    return hexToBin(words.toString(enc.Hex));
  }
  return words.toString(enc[encoding]);
}

export function hashWithAlgo(
  algo: AlgoName,
  text: string,
  encoding: Encoding,
): string {
  return formatOutput(ALGORITHMS[algo](text), encoding);
}
