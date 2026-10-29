import { create } from "zustand";
import type { Attachment } from "../chat/core/attachmentTypes";

interface AttachmentState {
  attachments: Attachment[];
  supportVision: boolean;
  setSupportVision: (v: boolean) => void;
  addAttachment: (att: Attachment) => void;
  removeAttachment: (id: string) => void;
  clearAttachments: () => void;
}

export const useAttachmentStore = create<AttachmentState>((set) => ({
  attachments: [],
  supportVision: false,

  setSupportVision: (v: boolean) => set({ supportVision: v }),

  addAttachment: (att: Attachment) =>
    set((s) => ({ attachments: [...s.attachments, att] })),

  removeAttachment: (id: string) =>
    set((s) => {
      const removed = s.attachments.find((a) => a.id === id);
      if (removed) URL.revokeObjectURL(removed.previewUrl);
      return { attachments: s.attachments.filter((a) => a.id !== id) };
    }),

  clearAttachments: () =>
    set((s) => {
      for (const att of s.attachments) URL.revokeObjectURL(att.previewUrl);
      return { attachments: [] };
    }),
}));
