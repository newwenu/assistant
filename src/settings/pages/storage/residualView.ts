import { translateKey } from "@/i18n";
import type {
  StorageInventoryDto,
  StorageInventoryNodeDto,
  StorageInventoryStatusDto,
} from "@/types/protocol.contract";
import { bytesOrDash, countText, timeText, type TagTone } from "./format";
import { inventoryClassText, layerText } from "./lexicon";
import { createLogger } from "@/services/system/logger";

const storageLog = createLogger("storage");

export type ResidualNoticeLevel = "warning" | "error";

export interface ResidualNotice {
  level: ResidualNoticeLevel;
  text: string;
  detail: string | null;
}

export interface ResidualChildView {
  name: string;
  text: string;
}

export interface ResidualRowView {
  name: string;
  classText: string;
  classTone: TagTone;
  isDeclared: boolean;
  scopeText: string;
  scanText: string;
  declaredText: string;
  residualText: string;
  overRegistered: boolean;
  expandable: boolean;
  children: ResidualChildView[];
  childrenMoreText: string | null;
}

export interface ResidualTotalLine {
  key: "total" | "residual" | "unregistered";
  text: string;
}

export interface ResidualViewModel {
  statusText: string;
  statusTone: TagTone;
  notices: ResidualNotice[];
  rows: ResidualRowView[];
  totalsLines: ResidualTotalLine[];
  totalsHint: string;
  unnameableText: string | null;
  empty: boolean;
}

const CLASS_ORDER = [
  "unregistered",
  "optionalNotMounted",
  "companion",
  "ignored",
  "declared",
] as const;

const CLASS_TONES = {
  unregistered: "warning",
  optionalNotMounted: "info",
  companion: "neutral",
  ignored: "neutral",
  declared: "success",
} as const satisfies Record<string, TagTone>;

const CHILD_LIMIT = 20;
const TRUNCATED_NAME_LIMIT = 3;

function classRank(value: string): number {
  const index = CLASS_ORDER.indexOf(value as (typeof CLASS_ORDER)[number]);
  return index === -1 ? CLASS_ORDER.length : index;
}

function toneOfClass(value: string): TagTone {
  const tones: Readonly<Record<string, TagTone>> = CLASS_TONES;
  return Object.prototype.hasOwnProperty.call(tones, value)
    ? tones[value]
    : "neutral";
}

function visible(node: StorageInventoryNodeDto): boolean {
  if (node.class === "declared") {
    return node.residualBytes !== null && node.residualBytes > 0;
  }
  return true;
}

function compareNodes(
  a: StorageInventoryNodeDto,
  b: StorageInventoryNodeDto,
): number {
  const rankDiff = classRank(a.class) - classRank(b.class);
  if (rankDiff !== 0) return rankDiff;

  if (a.residualBytes === null && b.residualBytes !== null) return 1;
  if (a.residualBytes !== null && b.residualBytes === null) return -1;
  if (a.residualBytes !== null && b.residualBytes !== null) {
    const diff = b.residualBytes - a.residualBytes;
    if (diff !== 0) return diff;
  }

  return a.name < b.name ? -1 : a.name > b.name ? 1 : 0;
}

function childText(child: StorageInventoryNodeDto["children"][number]): string {
  const count = countText(child.count);
  const size = bytesOrDash(child.bytes);
  return count === null ? size : `${size} · ${count}`;
}

function rowView(node: StorageInventoryNodeDto): ResidualRowView {
  const children = node.children.slice(0, CHILD_LIMIT);
  const hidden = node.children.length - children.length;
  return {
    name: node.name,
    classText: inventoryClassText(node.class),
    classTone: toneOfClass(node.class),
    isDeclared: node.class === "declared",
    scopeText: layerText(node.scope),
    scanText: bytesOrDash(node.scanBytes),
    declaredText:
      node.class === "declared"
        ? bytesOrDash(node.declaredBytes)
        : bytesOrDash(null),
    residualText: bytesOrDash(node.residualBytes),
    overRegistered: node.overRegistered,
    expandable: children.length > 0,
    children: children.map((child) => ({
      name: child.name,
      text: childText(child),
    })),
    childrenMoreText:
      hidden > 0
        ? translateKey("settings.storage.residual.children.more", {
            count: hidden,
          })
        : null,
  };
}

function statusOf(
  inventory: StorageInventoryDto,
  statusOverride: StorageInventoryStatusDto | undefined,
): {
  statusText: string;
  statusTone: TagTone;
} {
  const status = statusOverride ?? inventory.status;
  switch (status) {
    case "neverRun":
      return {
        statusText: translateKey("settings.storage.residual.status.neverRun"),
        statusTone: "neutral",
      };
    case "running":
      return {
        statusText: translateKey("settings.storage.residual.status.running"),
        statusTone: "neutral",
      };
    case "fresh":
      return {
        statusText: translateKey("settings.storage.residual.status.fresh", {
          time: timeText(inventory.scannedAt),
        }),
        statusTone: "neutral",
      };
    case "stale":
      return {
        statusText: translateKey("settings.storage.residual.status.stale", {
          time: timeText(inventory.scannedAt),
        }),
        statusTone: "warning",
      };
    case "failed":
      return {
        statusText: translateKey("settings.storage.residual.status.failed", {
          time: timeText(inventory.failedAt ?? inventory.scannedAt),
        }),
        statusTone: "error",
      };
    default: {
      const status: string = inventory.status;
      storageLog.warn(`unknown inventory status: "${status}"`);
      return { statusText: status, statusTone: "neutral" };
    }
  }
}

function noticesOf(
  inventory: StorageInventoryDto,
  rows: ResidualRowView[],
  status: StorageInventoryStatusDto,
): ResidualNotice[] {
  const notices: ResidualNotice[] = [];
  const hasNumbers =
    rows.length > 0 ||
    inventory.unregisteredBytes !== null ||
    inventory.unregisteredCount !== null;

  if (status === "failed" && hasNumbers) {
    notices.push({
      level: "warning",
      text: translateKey("settings.storage.residual.notice.failed_keep_last"),
      detail: null,
    });
  }

  // `inventory.partial` 有两个来源：真触界（`truncatedNodes` 非空）与 `overRegistered`
  // （登记侧比盘上大，扫描本身是完整的）。后者已由下方 error 提示独立表达，不能复用本条
  // 文案——「只扫描了一部分目录」与「扫描完整、登记偏大」方向正好相反，会把 EBWebView
  // 这类持续写入条目的噪声变成常亮误报。因此只按触界触发（I67 §N8）。
  if (inventory.partial && inventory.truncatedNodes.length > 0) {
    const names = inventory.truncatedNodes.slice(0, TRUNCATED_NAME_LIMIT);
    const hidden = inventory.truncatedNodes.length - names.length;
    const detailParts: string[] = [names.join(", ")];
    if (hidden > 0) {
      detailParts.push(
        translateKey("settings.storage.residual.notice.truncated_more", {
          count: hidden,
        }),
      );
    }
    notices.push({
      level: "warning",
      text: translateKey("settings.storage.residual.notice.partial"),
      detail: detailParts.join(" · "),
    });
  }

  if (rows.some((row) => row.overRegistered)) {
    notices.push({
      level: "error",
      text: translateKey("settings.storage.residual.notice.over_registered"),
      detail: null,
    });
  }

  return notices;
}

// 下界标注（§6）：`totals.partial` 时三个数统一加 `≥` 前缀，不得只标其中一个。
function sizeText(bytes: number | null, lowerBound: boolean): string {
  const text = bytesOrDash(bytes);
  return lowerBound && bytes !== null ? `≥${text}` : text;
}

function countOrDash(count: number | null): string {
  return count === null
    ? bytesOrDash(null)
    : translateKey("settings.storage.common.count_value", { count });
}

// 分 scope 数字求和：任一侧为 `null` 则整体 `null`（不把「不知道」写成 0）。
function scopeSum(a: number | null, b: number | null): number | null {
  return a === null || b === null ? null : a + b;
}

// 合计三段只读 `totals` 与顶层 `unregisteredBytes`，禁止对 `nodes` 求和（G5 / V7）。
function totalsLinesOf(inventory: StorageInventoryDto): ResidualTotalLine[] {
  const lowerBound = inventory.totals.partial;
  return [
    {
      key: "total",
      text: translateKey("settings.storage.residual.totals.total", {
        size: sizeText(inventory.totals.scanBytes, lowerBound),
      }),
    },
    {
      key: "residual",
      text: translateKey("settings.storage.residual.totals.residual", {
        size: sizeText(inventory.totals.residualBytes, lowerBound),
      }),
    },
    {
      key: "unregistered",
      text: translateKey("settings.storage.residual.totals.unregistered", {
        count: countOrDash(inventory.unregisteredCount),
        size: sizeText(inventory.unregisteredBytes, lowerBound),
      }),
    },
  ];
}

// 不可命名部分不进入节点列表，只在合计区追加一行说明（§3.4 / V3）。
function unnameableTextOf(inventory: StorageInventoryDto): string | null {
  const bytes = scopeSum(
    inventory.totals.data.unnameableBytes,
    inventory.totals.local.unnameableBytes,
  );
  if (bytes === null || bytes <= 0) return null;
  const count = scopeSum(
    inventory.totals.data.unnameableCount,
    inventory.totals.local.unnameableCount,
  );
  return translateKey("settings.storage.residual.totals.unnameable", {
    count: countOrDash(count),
  });
}

function totalsHintOf(inventory: StorageInventoryDto): string {
  return translateKey(
    inventory.totals.partial
      ? "settings.storage.residual.totals.lower_bound"
      : "settings.storage.residual.totals.hint",
  );
}

export function residualView(
  inventory: StorageInventoryDto,
  statusOverride?: StorageInventoryStatusDto,
): ResidualViewModel {
  const nodes = inventory.nodes.filter(visible).sort(compareNodes);
  const rows = nodes.map(rowView);
  const status = statusOverride ?? inventory.status;
  const { statusText, statusTone } = statusOf(inventory, statusOverride);

  return {
    statusText,
    statusTone,
    notices: noticesOf(inventory, rows, status),
    rows,
    totalsLines: totalsLinesOf(inventory),
    totalsHint: totalsHintOf(inventory),
    unnameableText: unnameableTextOf(inventory),
    empty: rows.length === 0,
  };
}
