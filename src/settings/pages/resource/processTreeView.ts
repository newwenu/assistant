import { translateKey, type I18nPath } from "@/i18n";
import type {
  MemoryBasisDto,
  ProcessEntryDto,
  ProcessGroupDto,
  ProcessRoleDto,
  ProcessSnapshotDto,
} from "@/types/protocol.contract";
import { formatBytes } from "@/utils/formatBytes";

const UNKNOWN_VALUE = "—";

const ROLE_KEYS = {
  host: "settings.resource.role.host",
  webviewBrowser: "settings.resource.role.webviewBrowser",
  webviewRenderer: "settings.resource.role.webviewRenderer",
  webviewGpu: "settings.resource.role.webviewGpu",
  webviewUtility: "settings.resource.role.webviewUtility",
  webviewUnknown: "settings.resource.role.webviewUnknown",
  otherChild: "settings.resource.role.otherChild",
} as const satisfies Record<ProcessRoleDto, I18nPath>;

const MEMORY_BASIS_KEYS = {
  workingSet: "settings.resource.caliber.linuxWorkingSet",
  privateWorkingSet: "settings.resource.caliber.winPrivate",
  physFootprint: "settings.resource.caliber.macFootprint",
} as const satisfies Record<MemoryBasisDto, I18nPath>;

export type ProcessGroupKey = "self" | "subtree";

export interface ProcessRowView {
  key: string;
  name: string;
  pidText: string;
  roleText: string;
  cpuText: string;
  cpuPending: boolean;
  memoryText: string;
}

export interface ProcessGroupView {
  key: ProcessGroupKey;
  label: string;
  pidText: string;
  cpuText: string;
  cpuPending: boolean;
  memoryText: string;
  memoryNote: string | null;
}

export interface ProcessSummaryView {
  pidText: string;
  hostText: string;
  sampledAtText: string | null;
  self: ProcessGroupView;
  subtree: ProcessGroupView;
  relationText: string;
  cpuNote: string;
  memoryCaliberNote: string | null;
}

export function roleText(role: ProcessRoleDto): string {
  return translateKey(ROLE_KEYS[role]);
}

export function memoryColumnText(basis: MemoryBasisDto): string {
  return translateKey(MEMORY_BASIS_KEYS[basis]);
}

export function cpuText(percent: number | null): string {
  return percent === null
    ? translateKey("settings.resource.status.cpuFirstSample")
    : `${percent.toFixed(1)}%`;
}

export function memoryText(bytes: number | null): string {
  return bytes === null ? UNKNOWN_VALUE : formatBytes(bytes);
}

export function entryRowView(entry: ProcessEntryDto): ProcessRowView {
  return {
    key: `${entry.pid}-${entry.startTime}`,
    name: entry.exeName,
    pidText: `pid ${entry.pid}`,
    roleText: roleText(entry.role),
    cpuText: cpuText(entry.cpuPercent),
    cpuPending: entry.cpuPercent === null,
    memoryText: memoryText(entry.memoryBytes),
  };
}

function groupView(
  key: ProcessGroupKey,
  group: ProcessGroupDto,
): ProcessGroupView {
  return {
    key,
    label: translateKey(
      key === "self"
        ? "settings.resource.summary.self"
        : "settings.resource.summary.subtree",
    ),
    pidText: translateKey("settings.resource.summary.pidCount", {
      count: group.pidCount,
    }),
    cpuText: cpuText(group.cpuPercent),
    cpuPending: group.cpuPercent === null,
    memoryText: memoryText(group.memoryBytes),
    memoryNote:
      group.memoryAvailability === "unavailable"
        ? (group.memoryReason ??
          translateKey("settings.resource.status.memoryUnavailable"))
        : null,
  };
}

function sampledAtText(sampledAt: number | null): string | null {
  if (sampledAt === null) return null;
  return translateKey("settings.resource.summary.sampledAt", {
    time: new Date(sampledAt).toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    }),
  });
}

export function summaryView(snapshot: ProcessSnapshotDto): ProcessSummaryView {
  return {
    pidText: `pid ${snapshot.rootPid}`,
    hostText: `${translateKey("settings.resource.summary.hostThreads")} · ${
      snapshot.hostThreadCount ?? UNKNOWN_VALUE
    }`,
    sampledAtText: sampledAtText(snapshot.sampledAt),
    self: groupView("self", snapshot.selfGroup),
    subtree: groupView("subtree", snapshot.subtreeGroup),
    relationText: translateKey("settings.resource.summary.relation"),
    cpuNote: translateKey("settings.resource.summary.cpuNote"),
    memoryCaliberNote: snapshot.memoryAdditive
      ? translateKey(
          snapshot.memoryEqualsFootprint
            ? "settings.resource.summary.memoryApprox"
            : "settings.resource.summary.memoryNotDoubleCounted",
        )
      : null,
  };
}
