import { isAfter, subWeeks } from "date-fns";
import { lazyWithRecovery } from "@/utils/lazyWithRecovery";
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
  FolderOutlined,
} from "@ant-design/icons";
import type { ToolDefinition, ToolCategoryGroup } from "./types";
import { TOOL_CATEGORIES, classifyKeywords } from "./categories";

type ToolInput = Omit<ToolDefinition, "category" | "isNew"> & {
  category?: never;
  isNew?: boolean;
};

export function defineTool(tool: ToolInput): ToolDefinition {
  const category = classifyKeywords(tool.keywords);
  const isNew = tool.createdAt
    ? isAfter(tool.createdAt, subWeeks(new Date(), 2))
    : false;
  return { ...tool, category, isNew };
}

const hashText = defineTool({
  id: "hash-text",
  name: "tools.hash_text.name",
  description: "tools.hash_text.description",
  icon: EyeInvisibleOutlined,
  keywords: ["hash", "digest", "crypto", "SHA1", "SHA256", "SHA512"],
  component: lazyWithRecovery(() => import("./hash-text")),
  widthMode: "default",
});

const uuidGenerator = defineTool({
  id: "uuid-generator",
  name: "tools.uuid_generator.name",
  description: "tools.uuid_generator.description",
  icon: IdcardOutlined,
  keywords: ["uuid", "v4", "v1", "random", "id", "unique"],
  component: lazyWithRecovery(() => import("./uuid-generator")),
  widthMode: "default",
});

const base64StringConverter = defineTool({
  id: "base64-string-converter",
  name: "tools.base64_string_converter.name",
  description: "tools.base64_string_converter.description",
  icon: SwapOutlined,
  keywords: ["base64", "encode", "decode", "convert"],
  component: lazyWithRecovery(() => import("./base64-string-converter")),
  widthMode: "default",
});

const colorConverter = defineTool({
  id: "color-converter",
  name: "tools.color_converter.name",
  description: "tools.color_converter.description",
  icon: BgColorsOutlined,
  keywords: ["color", "hex", "rgb", "hsl", "rgba", "picker", "converter"],
  component: lazyWithRecovery(() => import("./color-converter")),
  widthMode: "default",
});

const dateTimeConverter = defineTool({
  id: "date-time-converter",
  name: "tools.date_time_converter.name",
  description: "tools.date_time_converter.description",
  icon: ClockCircleOutlined,
  keywords: ["date", "time", "timestamp", "unix", "converter", "epoch"],
  component: lazyWithRecovery(() => import("./date-time-converter")),
  widthMode: "default",
});

const integerBaseConverter = defineTool({
  id: "integer-base-converter",
  name: "tools.integer_base_converter.name",
  description: "tools.integer_base_converter.description",
  icon: NumberOutlined,
  keywords: ["integer", "base", "binary", "octal", "decimal", "hex", "convert"],
  component: lazyWithRecovery(() => import("./integer-base-converter")),
  widthMode: "default",
});

const urlEncoder = defineTool({
  id: "url-encoder",
  name: "tools.url_encoder.name",
  description: "tools.url_encoder.description",
  icon: LinkOutlined,
  keywords: ["url", "encode", "decode", "uri", "component"],
  component: lazyWithRecovery(() => import("./url-encoder")),
  widthMode: "default",
});

const jsonViewer = defineTool({
  id: "json-viewer",
  name: "tools.json_viewer.name",
  description: "tools.json_viewer.description",
  icon: CodeOutlined,
  keywords: ["json", "format", "prettify", "minify", "validate", "viewer"],
  component: lazyWithRecovery(() => import("./json-viewer")),
  widthMode: "wide",
});

const regexTester = defineTool({
  id: "regex-tester",
  name: "tools.regex_tester.name",
  description: "tools.regex_tester.description",
  icon: SearchOutlined,
  keywords: ["regex", "regexp", "test", "match", "capture"],
  component: lazyWithRecovery(() => import("./regex-tester")),
  widthMode: "default",
});

const jwtParser = defineTool({
  id: "jwt-parser",
  name: "tools.jwt_parser.name",
  description: "tools.jwt_parser.description",
  icon: KeyOutlined,
  keywords: ["jwt", "token", "decode", "parse", "header", "payload"],
  component: lazyWithRecovery(() => import("./jwt-parser")),
  widthMode: "default",
});

const textDiff = defineTool({
  id: "text-diff",
  name: "tools.text_diff.name",
  description: "tools.text_diff.description",
  icon: DiffOutlined,
  keywords: ["diff", "compare", "text", "difference"],
  component: lazyWithRecovery(() => import("./text-diff")),
  widthMode: "wide",
  createdAt: new Date("2025-09-01"),
});

const textStatistics = defineTool({
  id: "text-statistics",
  name: "tools.text_statistics.name",
  description: "tools.text_statistics.description",
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
  component: lazyWithRecovery(() => import("./text-statistics")),
  widthMode: "default",
});

const passwordGenerator = defineTool({
  id: "password-generator",
  name: "tools.password_generator.name",
  description: "tools.password_generator.description",
  icon: SafetyCertificateOutlined,
  keywords: ["password", "generator", "strength", "entropy", "crack", "secure"],
  component: lazyWithRecovery(() => import("./password-generator")),
  widthMode: "default",
  createdAt: new Date("2025-09-15"),
});

const qrcodeGenerator = defineTool({
  id: "qrcode-generator",
  name: "tools.qrcode_generator.name",
  description: "tools.qrcode_generator.description",
  icon: QrcodeOutlined,
  keywords: ["qrcode", "qr", "code", "generate", "url", "text"],
  component: lazyWithRecovery(() => import("./qrcode-generator")),
  widthMode: "default",
  createdAt: new Date("2025-09-15"),
});

const sqlFormatter = defineTool({
  id: "sql-formatter",
  name: "tools.sql_formatter.name",
  description: "tools.sql_formatter.description",
  icon: DatabaseOutlined,
  keywords: ["sql", "format", "prettify", "minify", "dialect", "query"],
  component: lazyWithRecovery(() => import("./sql-formatter")),
  widthMode: "wide",
});

const cronParser = defineTool({
  id: "cron-parser",
  name: "tools.cron_parser.name",
  description: "tools.cron_parser.description",
  icon: FieldTimeOutlined,
  keywords: ["cron", "crontab", "schedule", "parse", "timer"],
  component: lazyWithRecovery(() => import("./cron-parser")),
  widthMode: "default",
});

const calculator = defineTool({
  id: "calculator",
  name: "tools.calculator.name",
  description: "tools.calculator.description",
  icon: CalculatorOutlined,
  keywords: ["calculator", "math", "evaluate", "expression", "compute"],
  component: lazyWithRecovery(() => import("./calculator")),
  widthMode: "default",
});

const unitConverter = defineTool({
  id: "unit-converter",
  name: "tools.unit_converter.name",
  description: "tools.unit_converter.description",
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
  component: lazyWithRecovery(() => import("./unit-converter")),
  widthMode: "default",
});

const ipInfo = defineTool({
  id: "ip-info",
  name: "tools.ip_info.name",
  description: "tools.ip_info.description",
  icon: GlobalOutlined,
  keywords: ["ip", "address", "ipv4", "ipv6", "lookup", "geo"],
  component: lazyWithRecovery(() => import("./ip-info")),
  widthMode: "default",
});

const imageCompressor = defineTool({
  id: "image-compressor",
  name: "tools.image_compressor.name",
  description: "tools.image_compressor.description",
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
  component: lazyWithRecovery(() => import("./image-compressor")),
  widthMode: "full",
  capabilities: { fileDrop: { accept: "image/*" } },
});

const svgPreview = defineTool({
  id: "svg-preview",
  name: "tools.svg_preview.name",
  description: "tools.svg_preview.description",
  icon: EditOutlined,
  keywords: ["svg", "preview", "edit", "optimize", "vector", "download"],
  component: lazyWithRecovery(() => import("./svg-preview")),
  widthMode: "full",
  capabilities: { fileDrop: { accept: ".svg,image/svg+xml" } },
});

const fsLink = defineTool({
  id: "fs-link",
  name: "tools.fs_link.name",
  description: "tools.fs_link.description",
  icon: FolderOutlined,
  keywords: [
    "link",
    "symlink",
    "hardlink",
    "junction",
    "filesystem",
    "path",
    "shortcut",
  ],
  component: lazyWithRecovery(() => import("./fs-link")),
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
  fsLink,
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
