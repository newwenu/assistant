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
  textDiff,
  textStatistics,
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
