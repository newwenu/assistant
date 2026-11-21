import { translateKey } from "@/i18n";
import type { StorageEntryDto } from "@/types/protocol.contract";
import { bytesOrDash, countText, timeText, type TagTone } from "./format";
import {
  availabilityText,
  eligibilityText,
  entryNameText,
  layerText,
  policyChips,
  reasonText,
  actionText,
} from "./lexicon";

export type { TagTone } from "./format";

export type CleanupActionView =
  | {
      kind: "button";
      disabled: boolean;
      strongConfirm: boolean;
      tooltip: string | null;
    }
  | { kind: "text"; text: string }
  | { kind: "none" };

export interface EntryAvailabilityBadge {
  tone: TagTone;
  reason: string;
}

export interface EntryRowView {
  key: string;
  name: string;
  sizeText: string;
  countText: string | null;
  availabilityBadge: EntryAvailabilityBadge | null;
  policyChips: string[];
  cleanup: CleanupActionView;
}

export interface EntryDetailItem {
  label: string | null;
  value: string;
  mono: boolean;
}

export interface EntryDetailView {
  items: EntryDetailItem[];
  chips: string[];
  availabilityReason: string | null;
  note: string | null;
}

const ABNORMAL_AVAILABILITY_TONES = {
  locked: "warning",
  staleReport: "warning",
  unreadable: "error",
  unavailable: "error",
  notImplemented: "neutral",
  noReportYet: "neutral",
} as const satisfies Record<string, TagTone>;

function badgeTone(availability: string): TagTone | null {
  return Object.prototype.hasOwnProperty.call(
    ABNORMAL_AVAILABILITY_TONES,
    availability,
  )
    ? ABNORMAL_AVAILABILITY_TONES[
        availability as keyof typeof ABNORMAL_AVAILABILITY_TONES
      ]
    : null;
}

function availabilityBadge(
  entry: StorageEntryDto,
): EntryAvailabilityBadge | null {
  const tone = badgeTone(entry.availability);
  if (tone === null) return null;
  return {
    tone,
    reason: entry.availabilityReason ?? availabilityText(entry.availability),
  };
}

function cleanupView(entry: StorageEntryDto): CleanupActionView {
  const reclaim = entry.reclaim;

  if (reclaim.action === "none") {
    if (reclaim.eligibility === "eligible") {
      console.warn(
        `[storage] entry "${entry.key}" is eligible but action is "none"`,
      );
    }
    return { kind: "none" };
  }

  if (reclaim.eligibility === "eligible") {
    return {
      kind: "button",
      disabled: false,
      strongConfirm: reclaim.confirmation === "strongConfirm",
      tooltip: null,
    };
  }

  if (reclaim.eligibility === "ownerManaged") {
    return {
      kind: "text",
      text: translateKey("settings.storage.entries.eligibility.auto_managed"),
    };
  }

  const reason =
    reclaim.eligibility === "notImplemented" && reclaim.note !== null
      ? reclaim.note
      : reclaim.reason !== null
        ? reasonText(reclaim.reason)
        : eligibilityText(reclaim.eligibility);

  return {
    kind: "button",
    disabled: true,
    strongConfirm: false,
    tooltip: `${actionText(reclaim.action)} · ${reason}`,
  };
}

export function entryRowView(entry: StorageEntryDto): EntryRowView {
  const name = entryNameText(entry.key);
  return {
    key: entry.key,
    name,
    sizeText: bytesOrDash(entry.bytes),
    countText: entry.bytes === null ? null : countText(entry.count),
    availabilityBadge: availabilityBadge(entry),
    policyChips: policyChips(entry.reclaim.policy, entry.reclaim.autoActive),
    cleanup: cleanupView(entry),
  };
}

function reclaimableText(entry: StorageEntryDto): string {
  const { reclaimableCount, reclaimableBytes } = entry.reclaim;
  if (reclaimableCount === null || reclaimableBytes === null) {
    return bytesOrDash(null);
  }
  return translateKey("settings.storage.entries.facts.reclaimable", {
    count: reclaimableCount,
    size: bytesOrDash(reclaimableBytes),
  });
}

function timeRangeText(entry: StorageEntryDto): string {
  const parts: string[] = [];
  if (entry.oldestAt !== null) {
    parts.push(
      translateKey("settings.storage.entries.facts.oldest", {
        time: timeText(entry.oldestAt),
      }),
    );
  }
  if (entry.newestAt !== null) {
    parts.push(
      translateKey("settings.storage.entries.facts.newest", {
        time: timeText(entry.newestAt),
      }),
    );
  }
  return parts.length === 0 ? bytesOrDash(null) : parts.join(" · ");
}

function policyText(entry: StorageEntryDto): string {
  const policy = entry.reclaim.policy;
  const parts: string[] = [policyChips(policy, false)[0]];
  if (policy.effectiveSince !== null) {
    parts.push(
      translateKey("settings.storage.entries.facts.effective_since", {
        time: timeText(policy.effectiveSince),
      }),
    );
  }
  if (policy.mutable) {
    parts.push(translateKey("settings.storage.entries.facts.mutable"));
  }
  return parts.join(" · ");
}

export function entryDetailView(entry: StorageEntryDto): EntryDetailView {
  const items: EntryDetailItem[] = [
    {
      label: null,
      value: entry.key,
      mono: true,
    },
    {
      label: translateKey("settings.storage.entries.facts.layer"),
      value: layerText(entry.layer),
      mono: false,
    },
    {
      label: null,
      value: translateKey("settings.storage.entries.provided_by", {
        owner: entry.owner,
      }),
      mono: false,
    },
    {
      label: null,
      value: timeRangeText(entry),
      mono: false,
    },
    {
      label: null,
      value: reclaimableText(entry),
      mono: false,
    },
    {
      label: null,
      value: policyText(entry),
      mono: false,
    },
  ];

  const chips: string[] = [];
  if (entry.includesDerived) {
    chips.push(translateKey("settings.storage.entries.facts.includes_derived"));
  }
  if (entry.countsSelfLogs) {
    chips.push(translateKey("settings.storage.entries.facts.counts_self_logs"));
  }

  return {
    items,
    chips,
    availabilityReason: entry.availabilityReason,
    note: entry.reclaim.note,
  };
}
