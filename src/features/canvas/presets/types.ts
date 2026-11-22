/**
 * 画布预设的纯数据类型（I68 §4.3）：只声明区带、卡片类别与列位，
 * 不含坐标、函数与译文（文案只给 i18n 键）。
 */
import type { I18nPath } from "@/i18n";
import type { CardTone, ExitTone } from "../types";

export type CanvasPresetId = "builtin-agent-turn";

/** 区带：世界坐标的行语义分区（I68 §5.2 / T-22）。 */
export type CanvasZoneId = "main-lane" | "branch-lane";

/** 示例卡片类别（展示用闭集，不是 L2 定案；I68 §2.4）。 */
export type CardKindId =
  | "execution-turn"
  | "execution-tool"
  | "execution-conversation"
  | "execution-hitl"
  | "container-conversation";

/** 参数摘要的一行：参数名走 i18n，示例值是原语量。 */
export interface CardParamSpec {
  labelKey: I18nPath;
  value: string;
}

/** 占位端口：类型标签是原语量（如 `turn.in`），不做校验。 */
export interface CardPortSpec {
  side: "left" | "right";
  typeLabel: string;
}

/** 主体附加区：轮次卡片放阶段轨道，其余放内容占位。 */
export type CardBodyKind =
  "stages" | "output" | "messages" | "question" | "subgraph";

/**
 * 卡片类别规格：画布与放置栏共用这一张表（单一来源，A1 / A24）。
 * 全部字段是示例常量，不来自运行、不来自后端。
 */
export interface CardKindSpec {
  id: CardKindId;
  category: "execution" | "container";
  /** 分型短名（徽标右半）；容器卡片为 `null`。 */
  formKey: I18nPath | null;
  nameKey: I18nPath;
  /** 放置栏里的一句话说明。 */
  summaryKey: I18nPath;
  /** 引用槽占位值（原语量，如 `turn` / `shell`）。 */
  reference: string;
  params: readonly CardParamSpec[];
  ports: readonly CardPortSpec[];
  body: CardBodyKind;
  /** 示例色调（示例数据，不来自运行）。 */
  tone: CardTone;
  /** 示例出口：只有轮次卡片有。 */
  exit?: ExitTone;
}

/** 区带：世界坐标的行语义分区。 */
export interface CanvasZone {
  id: CanvasZoneId;
  /** 世界坐标行号（由预设声明，不由顺序推断）。 */
  row: number;
  /** 区带说明文案的 i18n 键；`null` = 整行都是卡片、无标签。 */
  labelKey: I18nPath | null;
  /** 区带内留给说明标签的列号；`null` = 无标签。只用于标签定位。 */
  labelColumns: readonly number[] | null;
}

/** 预设中的一张卡片：声明类别、区带与列位，不声明坐标（A12）。 */
export interface PresetCard {
  kind: CardKindId;
  zone: CanvasZoneId;
  /** 区带内列位（示例摆放的固定列，由预设显式声明；不查模板）。 */
  column: number;
}

/** 示例连线：按类别声明端点（每类恰好一张卡，故类别即端点）。 */
export interface PresetConnector {
  from: CardKindId;
  to: CardKindId;
  route: "horizontal" | "vertical";
}

/** 连线策略：示例连线由预设声明，不从模板边集派生；连线本身可编辑。 */
export interface PresetConnectorPolicy {
  source: "preset-example";
  /** 可编辑：可选中、可拖拽端点重连、可断开、可删除；不保证持久化。 */
  editable: true;
}

export interface CanvasPreset {
  id: CanvasPresetId;
  builtin: true;
  titleKey: I18nPath;
  descriptionKey: I18nPath;
  zones: readonly CanvasZone[];
  cards: readonly PresetCard[];
  connectors: readonly PresetConnector[];
  connectorPolicy: PresetConnectorPolicy;
  /** 阶段轨道来源：12 条阶段条来自 turn 模板（不复制）。 */
  stages: { source: "template-nodes" };
}
