import type { ComponentType } from "react";
import {
  SafetyCertificateOutlined,
  SwapOutlined,
  CodeOutlined,
  ClockCircleOutlined,
  CalculatorOutlined,
  PictureOutlined,
  FontSizeOutlined,
  RocketOutlined,
  GlobalOutlined,
  FolderOpenOutlined,
} from "@ant-design/icons";

export const CATEGORY_KEYS = [
  "security",
  "codec",
  "dev",
  "time",
  "math",
  "media",
  "text",
  "generate",
  "network",
  "filesystem",
] as const;

export type ToolCategory = (typeof CATEGORY_KEYS)[number];

export interface ToolCategoryMeta {
  key: ToolCategory;
  label: string;
  icon: ComponentType;
  order: number;
}

const CATEGORY_SIGNATURES: Record<ToolCategory, readonly string[]> = {
  security: [
    "hash",
    "digest",
    "crypto",
    "password",
    "entropy",
    "crack",
    "secure",
    "strength",
    "SHA1",
    "SHA256",
    "SHA512",
  ],
  codec: [
    "encode",
    "decode",
    "convert",
    "converter",
    "base64",
    "hex",
    "rgb",
    "hsl",
    "color",
    "integer",
    "binary",
    "octal",
    "decimal",
    "url",
    "uri",
    "jwt",
    "token",
    "picker",
    "component",
  ],
  dev: [
    "json",
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
  ],
  time: [
    "date",
    "time",
    "timestamp",
    "epoch",
    "cron",
    "crontab",
    "schedule",
    "timer",
    "unix",
  ],
  math: [
    "math",
    "calculator",
    "compute",
    "evaluate",
    "expression",
    "unit",
    "length",
    "weight",
    "temperature",
    "area",
    "volume",
  ],
  media: [
    "image",
    "svg",
    "compress",
    "resize",
    "optimize",
    "preview",
    "edit",
    "vector",
    "download",
    "jpg",
    "png",
    "quality",
  ],
  text: [
    "text",
    "diff",
    "compare",
    "difference",
    "statistics",
    "count",
    "character",
    "word",
    "line",
    "byte",
  ],
  generate: [
    "generate",
    "generator",
    "uuid",
    "random",
    "create",
    "qrcode",
    "qr",
    "code",
    "id",
    "unique",
    "v4",
    "v1",
  ],
  network: ["ip", "address", "ipv4", "ipv6", "lookup", "geo", "network"],
  filesystem: [
    "file",
    "link",
    "symlink",
    "hardlink",
    "junction",
    "filesystem",
    "path",
    "directory",
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
  {
    key: "security",
    label: "tools.category.security",
    icon: SafetyCertificateOutlined,
    order: 0,
  },
  { key: "codec", label: "tools.category.codec", icon: SwapOutlined, order: 1 },
  { key: "dev", label: "tools.category.dev", icon: CodeOutlined, order: 2 },
  {
    key: "time",
    label: "tools.category.time",
    icon: ClockCircleOutlined,
    order: 3,
  },
  {
    key: "math",
    label: "tools.category.math",
    icon: CalculatorOutlined,
    order: 4,
  },
  {
    key: "media",
    label: "tools.category.media",
    icon: PictureOutlined,
    order: 5,
  },
  {
    key: "text",
    label: "tools.category.text",
    icon: FontSizeOutlined,
    order: 6,
  },
  {
    key: "generate",
    label: "tools.category.generate",
    icon: RocketOutlined,
    order: 7,
  },
  {
    key: "network",
    label: "tools.category.network",
    icon: GlobalOutlined,
    order: 8,
  },
  {
    key: "filesystem",
    label: "tools.category.filesystem",
    icon: FolderOpenOutlined,
    order: 9,
  },
];

export const CATEGORY_MAP = Object.fromEntries(
  TOOL_CATEGORIES.map((c) => [c.key, c]),
) as Record<string, ToolCategoryMeta>;
