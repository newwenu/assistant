import type {
  ReclaimPolicyDto,
  ReclaimViewDto,
  StorageEntryDto,
  StorageInventoryDto,
  StorageInventoryItemDto,
  StorageInventoryNodeDto,
  StorageReclaimEntryResultDto,
  StorageReclaimPlanEntryDto,
  StorageReclaimResultDto,
} from "@/types/protocol.contract";

export function makePolicy(
  overrides: Partial<ReclaimPolicyDto> = {},
): ReclaimPolicyDto {
  return {
    source: "none",
    retentionDays: null,
    keepCount: null,
    maxBytes: null,
    effectiveSince: null,
    mutable: false,
    failClosed: false,
    ...overrides,
  };
}

export function makeReclaim(
  overrides: Partial<ReclaimViewDto> = {},
): ReclaimViewDto {
  return {
    action: "none",
    eligibility: "notEligible",
    reason: null,
    policy: makePolicy(),
    reclaimableBytes: null,
    reclaimableCount: null,
    confirmation: "none",
    autoActive: false,
    note: null,
    ...overrides,
  };
}

export function makeEntry(
  overrides: Partial<StorageEntryDto> = {},
): StorageEntryDto {
  return {
    key: "attachments",
    layer: "data",
    owner: "AttachmentStore",
    bytes: 1024,
    count: 2,
    oldestAt: 100,
    newestAt: 200,
    availability: "available",
    availabilityReason: null,
    includesDerived: false,
    countsSelfLogs: false,
    reclaim: makeReclaim(),
    ...overrides,
  };
}

export function makeNode(
  overrides: Partial<StorageInventoryNodeDto> = {},
): StorageInventoryNodeDto {
  return {
    name: "stray",
    class: "unregistered",
    scope: "data",
    entry: null,
    scanBytes: 512,
    scanCount: 2,
    declaredBytes: null,
    declaredCount: null,
    residualBytes: 512,
    residualCount: 2,
    overRegistered: false,
    children: [],
    ...overrides,
  };
}

export function makeChild(
  overrides: Partial<StorageInventoryItemDto> = {},
): StorageInventoryItemDto {
  return { name: "child", bytes: 128, count: 1, ...overrides };
}

export function makeInventory(
  overrides: Partial<StorageInventoryDto> = {},
): StorageInventoryDto {
  return {
    status: "fresh",
    scannedAt: 1000,
    failedAt: null,
    nodes: [],
    unregisteredBytes: 512,
    unregisteredCount: 2,
    partial: false,
    truncatedNodes: [],
    ...overrides,
  };
}

export function makePlanEntry(
  overrides: Partial<StorageReclaimPlanEntryDto> = {},
): StorageReclaimPlanEntryDto {
  return {
    key: "attachments",
    action: "deleteOrphans",
    eligibility: "eligible",
    reason: null,
    expectedCount: 3,
    expectedBytes: 4096,
    policy: makePolicy(),
    confirmation: "confirm",
    ...overrides,
  };
}

export function makeReclaimEntryResult(
  overrides: Partial<StorageReclaimEntryResultDto> = {},
): StorageReclaimEntryResultDto {
  return {
    key: "attachments",
    action: "deleteOrphans",
    status: "done",
    removedCount: 3,
    removedBytes: 4096,
    failedCount: 0,
    skippedCount: 0,
    errors: [],
    ...overrides,
  };
}

export function makeReclaimResult(
  entries: StorageReclaimEntryResultDto[],
): StorageReclaimResultDto {
  return { planId: "plan-1", executedAt: 2000, entries };
}
