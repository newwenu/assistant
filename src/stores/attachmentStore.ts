import { create } from "zustand";
import type {
  AdmitError,
  Attachment,
  VisionSupport,
} from "../chat/attachment/types";
import { admitAttachment } from "../chat/attachment/admit";
import { resolveVisionSupport } from "../chat/attachment/vision";
import { useChatStore } from "./chatStore";
import { useProviderStore } from "./providerStore";

interface AttachmentState {
  attachments: Attachment[];
  addAttachment: (att: Attachment) => AdmitError | null;
  removeAttachment: (id: string) => void;
  clearAttachments: () => void;
}

/**
 * 视觉能力是 provider / model 的派生事实：每次按当前选中模型实时推导，
 * 不再复制成一份 store 状态再靠视图 effect 回写，避免同一事实出现两处。
 */
export function getVisionSupport(): VisionSupport {
  const providers = useProviderStore.getState().providers;
  const { selectedProviderId, selectedModelId } = useChatStore.getState();
  return resolveVisionSupport(providers, selectedProviderId, selectedModelId);
}

export const useAttachmentStore = create<AttachmentState>((set, get) => ({
  attachments: [],

  addAttachment: (att: Attachment): AdmitError | null => {
    const error = admitAttachment(att, {
      attachments: get().attachments,
      visionSupport: getVisionSupport(),
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
