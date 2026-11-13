import { create } from "zustand";
import type {
  AdmitError,
  Attachment,
  VisionSupport,
} from "../chat/attachment/types";
import { admitAttachment } from "../chat/attachment/admit";

interface AttachmentState {
  attachments: Attachment[];
  visionSupport: VisionSupport;
  setVisionSupport: (v: VisionSupport) => void;
  addAttachment: (att: Attachment) => AdmitError | null;
  removeAttachment: (id: string) => void;
  clearAttachments: () => void;
}

export const useAttachmentStore = create<AttachmentState>((set, get) => ({
  attachments: [],
  visionSupport: "unsupported",

  setVisionSupport: (v: VisionSupport) => set({ visionSupport: v }),

  addAttachment: (att: Attachment): AdmitError | null => {
    const error = admitAttachment(att, {
      attachments: get().attachments,
      visionSupport: get().visionSupport,
    });
    if (error) return error;
    set((s) => ({ attachments: [...s.attachments, att] }));
    return null;
  },

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
