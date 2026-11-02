import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ConversationListChangedPayload,
  ConversationTerminatedPayload,
  ConversationCompactFailedPayload,
  ConversationUnlockedPayload,
  ChatMessagesChangedPayload,
  Message,
} from "../chat/core/chatTypes";
import { ChatEvent } from "../chat/core/chatTypes";
import { toChatError } from "../chat/core/chatErrors";
import { useChatStore } from "../stores/chatStore";
import { createLogger } from "@/services/logger";

function mergeMessagesPreservingFrontendState(
  frontendMessages: Message[],
  backendMessages: Message[],
): Message[] {
  const frontendMap = new Map<string, Message>();
  for (const msg of frontendMessages) {
    frontendMap.set(msg.id, msg);
  }

  return backendMessages.map((backendMsg) => {
    const frontendMsg = frontendMap.get(backendMsg.id);
    if (!frontendMsg) return backendMsg;

    return {
      ...backendMsg,
      error: frontendMsg.error ?? backendMsg.error,
      reasoning: backendMsg.reasoning ?? frontendMsg.reasoning,
    };
  });
}

const log = createLogger("chatLifecycleEvents");

export function subscribeChatLifecycleEvents(): () => void {
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

  track(
    listen<ConversationListChangedPayload>(
      ChatEvent.ConversationListChanged,
      (event) => {
        if (disposed) return;
        const payload = event.payload;
        const store = useChatStore.getState();
        const currentId = store.currentConversationId;

        const isSelectedDeleted =
          (payload.changeKind === "deleted" &&
            payload.conversationId === currentId) ||
          (payload.changeKind === "batch_deleted" &&
            currentId != null &&
            payload.affectedIds?.includes(currentId)) ||
          (payload.changeKind === "purged" && currentId !== null);

        if (isSelectedDeleted) {
          useChatStore.setState({
            currentConversationId: null,
            messages: [],
          });
        }

        void useChatStore.getState().loadConversations(true);
      },
    ),
  );

  track(
    listen<ConversationTerminatedPayload>(
      ChatEvent.ConversationTerminated,
      (event) => {
        if (disposed) return;
        const payload = event.payload;
        const store = useChatStore.getState();
        if (store.currentConversationId === payload.conversationId) {
          useChatStore.setState({
            isFull: payload.isFull,
            compactSummary: payload.summary,
            compactError: null,
          });
        }
        void useChatStore.getState().loadConversations(true);
      },
    ),
  );

  track(
    listen<ConversationCompactFailedPayload>(
      ChatEvent.ConversationCompactFailed,
      (event) => {
        if (disposed) return;
        const payload = event.payload;
        const store = useChatStore.getState();
        if (store.currentConversationId === payload.conversationId) {
          useChatStore.setState({ compactError: toChatError(payload.error) });
        }
      },
    ),
  );

  track(
    listen<ConversationUnlockedPayload>(
      ChatEvent.ConversationUnlocked,
      (event) => {
        if (disposed) return;
        const payload = event.payload;
        const store = useChatStore.getState();
        if (store.currentConversationId === payload.conversationId) {
          useChatStore.setState({
            isFull: false,
            compactSummary: null,
            compactError: null,
          });
        }
        void useChatStore.getState().loadConversations(true);
      },
    ),
  );

  let lastProcessedSeq = 0;

  track(
    listen<ChatMessagesChangedPayload>(ChatEvent.MessagesChanged, (event) => {
      if (disposed) return;
      const payload = event.payload;
      const store = useChatStore.getState();
      if (store.currentConversationId === payload.conversationId) {
        if (payload._seq !== undefined && payload._seq <= lastProcessedSeq) {
          void store.switchConversation(payload.conversationId);
          return;
        }
        if (payload._seq !== undefined) {
          lastProcessedSeq = payload._seq;
        }
        useChatStore.setState({
          messages: mergeMessagesPreservingFrontendState(
            store.messages,
            payload.messages,
          ),
          activeBranchId: payload.activeBranchId,
        });
        if (payload.changeKind !== "stream_finished") {
          useChatStore.getState().refreshTokenUsage();
        }
      }
    }),
  );

  return () => {
    disposed = true;
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
}
