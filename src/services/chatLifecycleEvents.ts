import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ConversationListChangedPayload,
  ConversationTerminatedPayload,
  ConversationUnlockedPayload,
  Message,
  ToolCallView,
} from "../chat/core/chatTypes";
import type { ChatMessagesChangedPayloadDto } from "../types/protocol.contract";
import { ChatEvent } from "../chat/core/chatTypes";
import { hasPendingToolCalls, toChatMessage } from "../chat/core/toolCallView";
import { useChatStore } from "../stores/chatStore";
import { createLogger } from "@/services/logger";

/** 实时流里仍在进行的调用优先保留前端视图；否则以后端终态为准。 */
function mergeToolCalls(
  frontend: ToolCallView[] | undefined,
  backend: ToolCallView[] | undefined,
): ToolCallView[] | undefined {
  if (!frontend || frontend.length === 0) return backend;
  if (hasPendingToolCalls(frontend)) return frontend;
  if (backend && backend.length > 0) return backend;
  return frontend;
}

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
      toolCalls: mergeToolCalls(frontendMsg.toolCalls, backendMsg.toolCalls),
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
          useChatStore.setState({ isFull: payload.isFull });
        }
        void useChatStore.getState().loadConversations(true);
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
          useChatStore.setState({ isFull: false });
        }
        void useChatStore.getState().loadConversations(true);
      },
    ),
  );

  let lastProcessedSeq = 0;

  track(
    listen<ChatMessagesChangedPayloadDto>(
      ChatEvent.MessagesChanged,
      (event) => {
        if (disposed) return;
        const payload = event.payload;
        const store = useChatStore.getState();
        if (store.currentConversationId === payload.conversationId) {
          if (payload.seq !== null && payload.seq <= lastProcessedSeq) {
            void store.switchConversation(payload.conversationId);
            return;
          }
          if (payload.seq !== null) {
            lastProcessedSeq = payload.seq;
          }
          const backendMessages = payload.messages.map(toChatMessage);
          useChatStore.setState({
            messages: mergeMessagesPreservingFrontendState(
              store.messages,
              backendMessages,
            ),
            activeBranchId: payload.activeBranchId,
          });
          if (payload.changeKind !== "stream_finished") {
            useChatStore.getState().refreshTokenUsage();
          }
        }
      },
    ),
  );

  return () => {
    disposed = true;
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
}
