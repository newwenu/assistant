import React from "react";
import { isAfter, subWeeks } from "date-fns";
import {
  EyeInvisibleOutlined,
  IdcardOutlined,
  SwapOutlined,
  BgColorsOutlined,
  ClockCircleOutlined,
  NumberOutlined,
  LinkOutlined,
  CodeOutlined,
  SearchOutlined,
  KeyOutlined,
  DiffOutlined,
  FileTextOutlined,
  SafetyCertificateOutlined,
  QrcodeOutlined,
  DatabaseOutlined,
  FieldTimeOutlined,
  CalculatorOutlined,
  SwapRightOutlined,
  GlobalOutlined,
  PictureOutlined,
  EditOutlined,
} from "@ant-design/icons";
import type { ToolDefinition, ToolCategoryGroup } from "./types";
import { TOOL_CATEGORIES } from "./categories";

type ToolInput = Omit<ToolDefinition, "isNew"> & { isNew?: boolean };

export function defineTool(tool: ToolInput): ToolDefinition {
  const isNew = tool.createdAt
    ? isAfter(tool.createdAt, subWeeks(new Date(), 2))
    : false;
  return { ...tool, isNew };
}

const hashText = defineTool({
  id: "hash-text",
  name: "Hash 计算",
  description: "计算文本的 SHA1/SHA256/SHA512 哈希值",
  category: "crypto",
  icon: EyeInvisibleOutlined,
  keywords: ["hash", "digest", "crypto", "SHA1", "SHA256", "SHA512"],
  component: React.lazy(() => import("./hash-text")),
  widthMode: "default",
});

const uuidGenerator = defineTool({
  id: "uuid-generator",
  name: "UUID 生成",
  description: "生成 v1/v4 UUID，支持批量生成",
  category: "crypto",
  icon: IdcardOutlined,
  keywords: ["uuid", "v4", "v1", "random", "id", "unique"],
  component: React.lazy(() => import("./uuid-generator")),
  widthMode: "default",
});

const base64StringConverter = defineTool({
  id: "base64-string-converter",
  name: "Base64 编解码",
  description: "字符串与 Base64 互转",
  category: "crypto",
  icon: SwapOutlined,
  keywords: ["base64", "encode", "decode", "convert"],
  component: React.lazy(() => import("./base64-string-converter")),
  widthMode: "default",
});

const colorConverter = defineTool({
  id: "color-converter",
  name: "颜色转换",
  description: "HEX/RGB/HSL/RGBA 颜色格式互转",
  category: "converter",
  icon: BgColorsOutlined,
  keywords: ["color", "hex", "rgb", "hsl", "rgba", "picker", "converter"],
  component: React.lazy(() => import("./color-converter")),
  widthMode: "default",
});

const dateTimeConverter = defineTool({
  id: "date-time-converter",
  name: "时间戳转换",
  description: "Unix 时间戳与可读日期互转",
  category: "converter",
  icon: ClockCircleOutlined,
  keywords: ["date", "time", "timestamp", "unix", "converter", "epoch"],
  component: React.lazy(() => import("./date-time-converter")),
  widthMode: "default",
});

const integerBaseConverter = defineTool({
  id: "integer-base-converter",
  name: "进制转换",
  description: "二进制/八进制/十进制/十六进制互转",
  category: "converter",
  icon: NumberOutlined,
  keywords: ["integer", "base", "binary", "octal", "decimal", "hex", "convert"],
  component: React.lazy(() => import("./integer-base-converter")),
  widthMode: "default",
});

const urlEncoder = defineTool({
  id: "url-encoder",
  name: "URL 编解码",
  description: "URL encode/decode 与组件解析",
  category: "converter",
  icon: LinkOutlined,
  keywords: ["url", "encode", "decode", "uri", "component"],
  component: React.lazy(() => import("./url-encoder")),
  widthMode: "default",
});

const jsonViewer = defineTool({
  id: "json-viewer",
  name: "JSON 格式化",
  description: "JSON 格式化/压缩/验证与语法高亮",
  category: "dev",
  icon: CodeOutlined,
  keywords: ["json", "format", "prettify", "minify", "validate", "viewer"],
  component: React.lazy(() => import("./json-viewer")),
  widthMode: "wide",
});

const regexTester = defineTool({
  id: "regex-tester",
  name: "正则测试器",
  description: "测试正则表达式，匹配高亮与捕获组",
  category: "dev",
  icon: SearchOutlined,
  keywords: ["regex", "regexp", "test", "match", "capture"],
  component: React.lazy(() => import("./regex-tester")),
  widthMode: "default",
});

const jwtParser = defineTool({
  id: "jwt-parser",
  name: "JWT 解码",
  description: "解码 JWT Token，展示 Header 与 Payload",
  category: "dev",
  icon: KeyOutlined,
  keywords: ["jwt", "token", "decode", "parse", "header", "payload"],
  component: React.lazy(() => import("./jwt-parser")),
  widthMode: "default",
});

const textDiff = defineTool({
  id: "text-diff",
  name: "Diff 对比",
  description: "双栏文本对比，差异高亮",
  category: "text",
  icon: DiffOutlined,
  keywords: ["diff", "compare", "text", "difference"],
  component: React.lazy(() => import("./text-diff")),
  widthMode: "wide",
  createdAt: new Date("2025-09-01"),
});

const textStatistics = defineTool({
  id: "text-statistics",
  name: "字数统计",
  description: "统计字符数/单词数/行数/字节数",
  category: "text",
  icon: FileTextOutlined,
  keywords: [
    "text",
    "statistics",
    "count",
    "character",
    "word",
    "line",
    "byte",
  ],
  component: React.lazy(() => import("./text-statistics")),
  widthMode: "default",
});

const passwordGenerator = defineTool({
  id: "password-generator",
  name: "密码生成器",
  description: "生成安全密码并评估强度与破解耗时",
  category: "daily",
  icon: SafetyCertificateOutlined,
  keywords: ["password", "generator", "strength", "entropy", "crack", "secure"],
  component: React.lazy(() => import("./password-generator")),
  widthMode: "default",
  createdAt: new Date("2025-09-15"),
});

const qrcodeGenerator = defineTool({
  id: "qrcode-generator",
  name: "二维码生成",
  description: "文本/URL 生成二维码，支持颜色与容错等级",
  category: "daily",
  icon: QrcodeOutlined,
  keywords: ["qrcode", "qr", "code", "generate", "url", "text"],
  component: React.lazy(() => import("./qrcode-generator")),
  widthMode: "default",
  createdAt: new Date("2025-09-15"),
});

const sqlFormatter = defineTool({
  id: "sql-formatter",
  name: "SQL 格式化",
  description: "SQL 格式化/压缩，支持多种方言",
  category: "dev",
  icon: DatabaseOutlined,
  keywords: ["sql", "format", "prettify", "minify", "dialect", "query"],
  component: React.lazy(() => import("./sql-formatter")),
  widthMode: "wide",
});

const cronParser = defineTool({
  id: "cron-parser",
  name: "Cron 解析",
  description: "Cron 表达式解析与下次执行时间",
  category: "dev",
  icon: FieldTimeOutlined,
  keywords: ["cron", "crontab", "schedule", "parse", "timer"],
  component: React.lazy(() => import("./cron-parser")),
  widthMode: "default",
});

const calculator = defineTool({
  id: "calculator",
  name: "计算器",
  description: "数学表达式计算与历史记录",
  category: "math",
  icon: CalculatorOutlined,
  keywords: ["calculator", "math", "evaluate", "expression", "compute"],
  component: React.lazy(() => import("./calculator")),
  widthMode: "default",
});

const unitConverter = defineTool({
  id: "unit-converter",
  name: "单位换算",
  description: "长度/重量/温度/面积/体积等互转",
  category: "converter",
  icon: SwapRightOutlined,
  keywords: [
    "unit",
    "convert",
    "length",
    "weight",
    "temperature",
    "area",
    "volume",
  ],
  component: React.lazy(() => import("./unit-converter")),
  widthMode: "default",
});

const ipInfo = defineTool({
  id: "ip-info",
  name: "IP 查询",
  description: "本机/指定 IP 地址信息查询与转换",
  category: "web",
  icon: GlobalOutlined,
  keywords: ["ip", "address", "ipv4", "ipv6", "lookup", "geo"],
  component: React.lazy(() => import("./ip-info")),
  widthMode: "default",
});

const imageCompressor = defineTool({
  id: "image-compressor",
  name: "图片压缩",
  description: "图片上传、质量/尺寸调整、压缩对比与下载",
  category: "media",
  icon: PictureOutlined,
  keywords: [
    "image",
    "compress",
    "resize",
    "quality",
    "optimize",
    "jpg",
    "png",
  ],
  component: React.lazy(() => import("./image-compressor")),
  widthMode: "full",
});

const svgPreview = defineTool({
  id: "svg-preview",
  name: "SVG 预览",
  description: "SVG 代码编辑、实时预览、优化与下载",
  category: "media",
  icon: EditOutlined,
  keywords: ["svg", "preview", "edit", "optimize", "vector", "download"],
  component: React.lazy(() => import("./svg-preview")),
  widthMode: "full",
});

export const tools: ToolDefinition[] = [
  hashText,
  uuidGenerator,
  base64StringConverter,
  colorConverter,
  dateTimeConverter,
  integerBaseConverter,
  urlEncoder,
  jsonViewer,
  regexTester,
  jwtParser,
  sqlFormatter,
  cronParser,
  textDiff,
  textStatistics,
  qrcodeGenerator,
  calculator,
  unitConverter,
  passwordGenerator,
  ipInfo,
  imageCompressor,
  svgPreview,
];

export const toolsMap = Object.fromEntries(
  tools.map((t) => [t.id, t]),
) as Record<string, ToolDefinition>;

export const toolsByCategory: ToolCategoryGroup[] = TOOL_CATEGORIES.map(
  (meta) => ({
    category: meta.key,
    meta,
    tools: tools.filter((t) => t.category === meta.key),
  }),
).filter((g) => g.tools.length > 0);