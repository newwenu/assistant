import type { ComponentType } from "react";
import {
  SafetyCertificateOutlined,
  SwapOutlined,
  CodeOutlined,
  PictureOutlined,
} from "@ant-design/icons";

export const CATEGORY_KEYS = ["dev", "converter", "security", "media"] as const;

export type ToolCategory = (typeof CATEGORY_KEYS)[number];

export interface ToolCategoryMeta {
  key: ToolCategory;
  label: string;
  icon: ComponentType;
  order: number;
}

const CATEGORY_SIGNATURES: Record<ToolCategory, readonly string[]> = {
  dev: [
    "uuid",
    "guid",
    "v4",
    "v1",
    "unique-id",
    "random-id",
    "identifier",
    "rfc4122",
    "id-gen",
    "dev",
    "json",
    "code-format",
    "sql",
    "regex",
    "regexp",
    "format",
    "prettify",
    "minify",
    "validate",
    "query",
    "dialect",
    "viewer",
    "cron",
    "crontab",
    "schedule",
    "timer",
    "ip",
    "address",
    "ipv4",
    "ipv6",
    "lookup",
    "geo",
    "network",
    "link",
    "symlink",
    "hardlink",
    "junction",
    "filesystem",
    "path",
    "directory",
    "diff",
    "compare",
    "difference",
    "statistics",
    "count",
    "calculator",
    "math",
    "evaluate",
    "compute",
    "arithmetic",
    "expression",
  ],
  converter: [
    "base64",
    "encode",
    "decode",
    "binary-to-text",
    "rfc4648",
    "charset",
    "padding",
    "ascii",
    "transform",
    "converter",
    "color",
    "colorspace",
    "rgb",
    "hsl",
    "rgba",
    "hex",
    "picker",
    "palette",
    "css-color",
    "date",
    "time",
    "timestamp",
    "unix",
    "epoch",
    "datetime",
    "calendar",
    "iso8601",
    "timezone",
    "integer",
    "base",
    "binary",
    "octal",
    "decimal",
    "radix",
    "numeral",
    "bit",
    "digit",
    "url",
    "uri",
    "percent-encoding",
    "urlencode",
    "unit",
    "length",
    "weight",
    "temperature",
    "area",
    "volume",
    "measurement",
    "metric",
    "imperial",
  ],
  security: [
    "hash",
    "digest",
    "crypto",
    "sha1",
    "sha256",
    "sha512",
    "checksum",
    "integrity",
    "fingerprint",
    "one-way",
    "verify",
    "audit",
    "jwt",
    "token",
    "claims",
    "issuer",
    "payload",
    "signature",
    "base64url",
    "jws",
    "authentication",
    "password",
    "strength",
    "entropy",
    "crack",
    "secure",
    "brute-force",
    "policy",
  ],
  media: [
    "qrcode",
    "qr",
    "barcode",
    "matrix",
    "error-correction",
    "visual",
    "render",
    "download",
    "media",
    "graphic",
    "image",
    "compress",
    "resize",
    "quality",
    "optimize",
    "jpg",
    "png",
    "webp",
    "thumbnail",
    "svg",
    "preview",
    "edit",
    "vector",
    "graphics",
    "path",
    "xml",
  ],
};

export function classifyKeywords(keywords: readonly string[]): ToolCategory {
  const kwSet = new Set(keywords.map((k) => k.toLowerCase()));
  let bestKey: ToolCategory = CATEGORY_KEYS[0];
  let bestScore = -1;

  for (const key of CATEGORY_KEYS) {
    const sig = CATEGORY_SIGNATURES[key];
    let score = 0;
    for (const s of sig) {
      if (kwSet.has(s.toLowerCase())) score++;
    }
    if (score > bestScore) {
      bestScore = score;
      bestKey = key;
    }
  }
  return bestKey;
}

export const TOOL_CATEGORIES: ToolCategoryMeta[] = [
  { key: "dev", label: "tools.category.dev", icon: CodeOutlined, order: 0 },
  {
    key: "converter",
    label: "tools.category.converter",
    icon: SwapOutlined,
    order: 1,
  },
  {
    key: "security",
    label: "tools.category.security",
    icon: SafetyCertificateOutlined,
    order: 2,
  },
  {
    key: "media",
    label: "tools.category.media",
    icon: PictureOutlined,
    order: 3,
  },
];

export const CATEGORY_MAP = Object.fromEntries(
  TOOL_CATEGORIES.map((c) => [c.key, c]),
) as Record<string, ToolCategoryMeta>;
