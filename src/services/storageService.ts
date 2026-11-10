import type {
  AssetDirsDto,
  AttachmentDeleteResultDto,
  AttachmentImportDto,
  StorageBreakdownDto,
} from "@/types/protocol.contract";
import { tauriInvoke as invoke } from "./tauriInvoke";

export const storageService = {
  getStorageBreakdown: (): Promise<StorageBreakdownDto> =>
    invoke<StorageBreakdownDto>("get_storage_breakdown"),

  getAssetDirs: (): Promise<AssetDirsDto> =>
    invoke<AssetDirsDto>("get_asset_dirs"),

  importAttachment: (path: string): Promise<AttachmentImportDto> =>
    invoke<AttachmentImportDto>("import_attachment", { path }),

  deleteAttachments: (
    attachmentIds: string[],
  ): Promise<AttachmentDeleteResultDto> =>
    invoke<AttachmentDeleteResultDto>("delete_attachments", { attachmentIds }),

  openAttachment: (attachmentId: string): Promise<boolean> =>
    invoke<boolean>("open_attachment", { attachmentId }),
};
