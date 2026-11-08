import type {
  AttachmentDeleteResultDto,
  StorageBreakdownDto,
} from "@/types/protocol.contract";
import { tauriInvoke as invoke } from "./tauriInvoke";

export const storageService = {
  getStorageBreakdown: (): Promise<StorageBreakdownDto> =>
    invoke<StorageBreakdownDto>("get_storage_breakdown"),

  deleteAttachments: (
    attachmentIds: string[],
  ): Promise<AttachmentDeleteResultDto> =>
    invoke<AttachmentDeleteResultDto>("delete_attachments", { attachmentIds }),

  openAttachment: (attachmentId: string): Promise<boolean> =>
    invoke<boolean>("open_attachment", { attachmentId }),
};
