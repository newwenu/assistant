import { translateKey } from "@/i18n";
import type { StorageEntryDto } from "@/types/protocol.contract";
import { bytesOrDash, countText, timeText, type TagTone } from "./format";
import {
  availabilityText,
  eligibilityText,
  entryIdText,
  entryNameText,
  layerText,
  policyChips,
  reasonText,
  actionText,
} from "./lexicon";

export type { TagTone } from "./format";

export type EntryActionView =
  | {
      kind: "button";
      disabled: boolean;
      strongConfirm: boolean;
      tooltip: string | null;
    }
  | { kind: "text"; text: string }
  | { kind: "none" };

export interface EntryRowView {
  key: string;
  id: string | null;
  name: string;
  nameIsFallback: boolean;
  sizeText: string;
  countText: string | null;
  availability: {
    text: string;
    tone: TagTone;
    reason: string | null;
  };
  eligibility: {
    text: string;
    tone: TagTone;
    secondLine: string | null;
  };
  policyChips: string[];
  action: EntryActionView;
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

const AVAILABILITY_TONES = {
  available: "neutral",
  missing: "neutral",
  locked: "warning",
  unreadable: "error",
  unavailable: "error",
  notImplemented: "info",
  noReportYet: "neutral",
  staleReport: "warning",
} as const satisfies Record<string, TagTone>;

const ELIGIBILITY_TONES = {
  eligible: "success",
  notEligible: "neutral",
  locked: "warning",
  ownerManaged: "info",
  notImplemented: "info",
} as const satisfies Record<string, TagTone>;

function toneOf(
  tones: Readonly<Record<string, TagTone>>,
  value: string,
): TagTone {
  return Object.prototype.hasOwnProperty.call(tones, value)
    ? tones[value]
    : "neutral";
}

function eligibilitySecondLine(entry: StorageEntryDto): string | null {
  const reclaim = entry.reclaim;
  if (reclaim.eligibility === "notImplemented" && reclaim.note !== null) {
    return reclaim.note;
  }
  if (reclaim.reason !== null) return reasonText(reclaim.reason);
  return null;
}

function actionView(entry: StorageEntryDto): EntryActionView {
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
    return { kind: "text", text: eligibilityText("ownerManaged") };
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
  const id = entryIdText(entry.key);
  return {
    key: entry.key,
    id,
    name: entryNameText(entry.key),
    nameIsFallback: id === null,
    sizeText: bytesOrDash(entry.bytes),
    countText: entry.bytes === null ? null : countText(entry.count),
    availability: {
      text: availabilityText(entry.availability),
      tone: toneOf(AVAILABILITY_TONES, entry.availability),
      reason: entry.availabilityReason,
    },
    eligibility: {
      text: eligibilityText(entry.reclaim.eligibility),
      tone: toneOf(ELIGIBILITY_TONES, entry.reclaim.eligibility),
      secondLine: eligibilitySecondLine(entry),
    },
    policyChips: policyChips(entry.reclaim.policy, entry.reclaim.autoActive),
    action: actionView(entry),
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
  if (policy.failClosed) {
    parts.push(translateKey("settings.storage.entries.facts.fail_closed"));
  }
  return parts.join(" · ");
}

export function entryDetailView(entry: StorageEntryDto): EntryDetailView {
  const id = entryIdText(entry.key);
  const items: EntryDetailItem[] = [
    {
      label: null,
      value: id === null ? entry.key : `${id} / ${entry.key}`,
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
