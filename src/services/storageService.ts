import { tauriInvoke as invoke } from "./tauriInvoke";

export interface AttachmentSummary {
  attachmentId: string;
  mediaType: string;
  bytes: number;
  createdAt: number;
}

export interface ConversationAttachmentSummary {
  conversationId: string;
  title: string;
  attachments: AttachmentSummary[];
  totalBytes: number;
  updatedAt: number;
}

export interface StorageBreakdownDto {
  totalBytes: number;
  totalCount: number;
  orphans: AttachmentSummary[];
  conversations: ConversationAttachmentSummary[];
}

export interface AttachmentDeleteResultDto {
  removedCount: number;
  removedBytes: number;
}

export const storageService = {
  getStorageBreakdown: (): Promise<StorageBreakdownDto> =>
    invoke<StorageBreakdownDto>("get_storage_breakdown"),

  deleteAttachments: (
    attachmentIds: string[],
  ): Promise<AttachmentDeleteResultDto> =>
    invoke<AttachmentDeleteResultDto>("delete_attachments", { attachmentIds }),
};
