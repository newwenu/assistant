import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ConversationListChangedPayload,
  ChatMessagesChangedPayload,
  ConversationTerminatedPayload,
  ConversationUnlockedPayload,
} from "../chat/core/chatTypes";
import { ChatEvent } from "../chat/core/chatTypes";
import { useHistoryStore } from "../stores/historyStore";
import { useChatStore } from "../stores/chatStore";
import { chatService } from "./chatService";
import { createLogger } from "./logger";

const log = createLogger("historyEvents");

const DEBOUNCE_MS = 50;

function debounce(fn: () => void, ms: number): () => void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  return () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = null;
      fn();
    }, ms);
  };
}

async function refreshConversationList(): Promise<void> {
  try {
    const convs = await chatService.listConversations();
    useChatStore.getState().updateConversations(convs);
  } catch (e) {
    log.error("Failed to refresh conversation list", e);
  }
}

export function subscribeHistoryEvents(): () => void {
  let disposed = false;
  const unlisteners: UnlistenFn[] = [];

  const track = (promise: Promise<UnlistenFn>) => {
    promise
      .then((unlisten) => {
        if (disposed) unlisten();
        else unlisteners.push(unlisten);
      })
      .catch((error) => {
        log.debug("listener unavailable", error);
      });
  };

  const debouncedRefreshList = debounce(() => {
    if (!disposed) void refreshConversationList();
  }, DEBOUNCE_MS);

  track(
    listen<ConversationListChangedPayload>(
      ChatEvent.ConversationListChanged,
      (event) => {
        if (disposed) return;
        const payload = event.payload;
        const selectedId = useHistoryStore.getState().selectedId;

        const isSelectedDeleted =
          (payload.changeKind === "deleted" &&
            payload.conversationId === selectedId) ||
          (payload.changeKind === "batch_deleted" &&
            selectedId != null &&
            payload.affectedIds?.includes(selectedId)) ||
          (payload.changeKind === "purged" && selectedId !== null);

        if (isSelectedDeleted) {
          useHistoryStore.getState().clearSelection();
        }

        debouncedRefreshList();
      },
    ),
  );

  track(
    listen<ChatMessagesChangedPayload>(ChatEvent.MessagesChanged, (event) => {
      if (disposed) return;
      const payload = event.payload;
      const selectedId = useHistoryStore.getState().selectedId;
      if (payload.conversationId === selectedId) {
        void useHistoryStore.getState().refreshMessages();
      }
    }),
  );

  track(
    listen<ConversationTerminatedPayload>(
      ChatEvent.ConversationTerminated,
      () => {
        if (disposed) return;
        debouncedRefreshList();
      },
    ),
  );

  track(
    listen<ConversationUnlockedPayload>(ChatEvent.ConversationUnlocked, () => {
      if (disposed) return;
      debouncedRefreshList();
    }),
  );

  return () => {
    disposed = true;
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
}
