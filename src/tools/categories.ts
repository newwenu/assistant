import {
  LockOutlined,
  SwapOutlined,
  GlobalOutlined,
  CodeOutlined,
  FontSizeOutlined,
  CalculatorOutlined,
  SmileOutlined,
  PictureOutlined,
} from "@ant-design/icons";
import type { ToolCategoryMeta } from "./types";

export const TOOL_CATEGORIES: ToolCategoryMeta[] = [
  { key: "crypto", label: "加密", icon: LockOutlined, order: 0 },
  { key: "converter", label: "转换器", icon: SwapOutlined, order: 1 },
  { key: "web", label: "网络", icon: GlobalOutlined, order: 2 },
  { key: "dev", label: "开发", icon: CodeOutlined, order: 3 },
  { key: "text", label: "文本", icon: FontSizeOutlined, order: 4 },
  { key: "math", label: "数学", icon: CalculatorOutlined, order: 5 },
  { key: "daily", label: "日常", icon: SmileOutlined, order: 6 },
  { key: "media", label: "多媒体", icon: PictureOutlined, order: 7 },
];

export const CATEGORY_MAP = Object.fromEntries(
  TOOL_CATEGORIES.map((c) => [c.key, c]),
) as Record<string, ToolCategoryMeta>;
