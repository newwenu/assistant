import type {
  AssetDirsDto,
  AttachmentDeleteResultDto,
  AttachmentImportDto,
  AttachmentLimitsDto,
  StorageBreakdownDto,
  StorageInventorySnapshotStatusDto,
  StorageReclaimEntryRequestDto,
  StorageReclaimPlanDto,
  StorageReclaimResultDto,
} from "@/types/protocol.contract";
import { tauriInvoke as invoke } from "../system/tauriInvoke";

export const storageService = {
  getStorageBreakdown: (): Promise<StorageBreakdownDto> =>
    invoke<StorageBreakdownDto>("get_storage_breakdown"),

  getAssetDirs: (): Promise<AssetDirsDto> =>
    invoke<AssetDirsDto>("get_asset_dirs"),

  getAttachmentLimits: (): Promise<AttachmentLimitsDto> =>
    invoke<AttachmentLimitsDto>("get_attachment_limits"),

  importAttachment: (path: string): Promise<AttachmentImportDto> =>
    invoke<AttachmentImportDto>("import_attachment", { path }),

  deleteAttachments: (
    attachmentIds: string[],
  ): Promise<AttachmentDeleteResultDto> =>
    invoke<AttachmentDeleteResultDto>("delete_attachments", { attachmentIds }),

  openAttachment: (attachmentId: string): Promise<boolean> =>
    invoke<boolean>("open_attachment", { attachmentId }),

  getStorageReclaimPlan: (
    entryKeys: string[],
  ): Promise<StorageReclaimPlanDto> =>
    invoke<StorageReclaimPlanDto>("get_storage_reclaim_plan", {
      request: { entryKeys },
    }),

  reclaimStorage: (
    planId: string,
    entries: StorageReclaimEntryRequestDto[],
    confirmed: boolean,
  ): Promise<StorageReclaimResultDto> =>
    invoke<StorageReclaimResultDto>("reclaim_storage", {
      request: { planId, entries, confirmed },
    }),

  refreshStorageInventory: (): Promise<StorageInventorySnapshotStatusDto> =>
    invoke<StorageInventorySnapshotStatusDto>("refresh_storage_inventory"),
};
