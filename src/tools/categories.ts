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
  { key: "security", label: "安全", icon: SafetyCertificateOutlined, order: 0 },
  { key: "codec", label: "编解码", icon: SwapOutlined, order: 1 },
  { key: "dev", label: "开发", icon: CodeOutlined, order: 2 },
  { key: "time", label: "时间", icon: ClockCircleOutlined, order: 3 },
  { key: "math", label: "数学", icon: CalculatorOutlined, order: 4 },
  { key: "media", label: "媒体", icon: PictureOutlined, order: 5 },
  { key: "text", label: "文本", icon: FontSizeOutlined, order: 6 },
  { key: "generate", label: "生成", icon: RocketOutlined, order: 7 },
  { key: "network", label: "网络", icon: GlobalOutlined, order: 8 },
];

export const CATEGORY_MAP = Object.fromEntries(
  TOOL_CATEGORIES.map((c) => [c.key, c]),
) as Record<string, ToolCategoryMeta>;
