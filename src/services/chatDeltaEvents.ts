import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useChatStore } from "../stores/chatStore";
import type { TokenUsage } from "../stores/chatStore";
import {
  createChatDeltaCoalescer,
  createSeqDedupFilter,
  textChunkScheduler,
  type CoalescibleDelta,
} from "../chat/streamCoalescer";
import { toChatError } from "../chat/chatErrors";

const CHAT_DELTA_EVENT = "chat-delta";

export function subscribeChatDeltaEvents(): () => void {
  let disposed = false;
  const unlisteners: UnlistenFn[] = [];

  const track = (promise: Promise<UnlistenFn>) => {
    promise
      .then((unlisten) => {
        if (disposed) unlisten();
        else unlisteners.push(unlisten);
      })
      .catch((error) => {
        console.debug("[chat-delta] listener unavailable:", error);
      });
  };

  const store = useChatStore;

  const seqDedup = createSeqDedupFilter();

  const coalescer = createChatDeltaCoalescer<CoalescibleDelta>(
    (event) => {
      if (event.delta) {
        if (import.meta.env.DEV) {
          console.debug(
            `[coalescer-deliver] seq=${event.seq} len=${event.delta.length} snippet=${JSON.stringify(event.delta.slice(0, 40))}`,
          );
        }
        store.getState().appendDelta(event.delta);
      }
    },
    textChunkScheduler,
    seqDedup.accept,
  );

  track(
    listen(CHAT_DELTA_EVENT, (event) => {
      if (disposed) return;

      const payload = event.payload as {
        conversation_id: string;
        message_id: string;
        seq?: number;
        delta_text?: string;
        delta_reasoning?: string;
        delta_tool_call_id?: string;
        delta_tool_call_name?: string;
        delta_tool_call_arguments?: string;
        finish_reason?: string;
        usage?: TokenUsage;
        error?: unknown;
      };

      if (payload.error) {
        coalescer.flush();
        store.getState().errorStream(toChatError(payload.error));
        return;
      }

      if (payload.delta_text) {
        store.setState({ isReasoningStreaming: false });
        coalescer.push({
          conversation_id: payload.conversation_id,
          message_id: payload.message_id,
          seq: payload.seq,
          delta: payload.delta_text,
        });
      }

      if (payload.delta_reasoning) {
        const state = store.getState();
        if (state.streamingMessageId) {
          store.setState({
            isReasoningStreaming: true,
            messages: state.messages.map((m) =>
              m.id === state.streamingMessageId
                ? {
                    ...m,
                    reasoning: (m.reasoning || "") + payload.delta_reasoning,
                  }
                : m,
            ),
          });
        }
      }

      if (
        payload.delta_tool_call_id ||
        payload.delta_tool_call_name ||
        payload.delta_tool_call_arguments
      ) {
        const state = store.getState();
        if (state.streamingMessageId) {
          store.setState({
            messages: state.messages.map((m) => {
              if (m.id !== state.streamingMessageId) return m;
              const toolCalls = [...(m.toolCalls || [])];
              if (payload.delta_tool_call_id) {
                toolCalls.push({
                  id: payload.delta_tool_call_id,
                  name: payload.delta_tool_call_name || "",
                  arguments: payload.delta_tool_call_arguments || "",
                });
              } else if (toolCalls.length > 0) {
                const last = toolCalls[toolCalls.length - 1];
                if (payload.delta_tool_call_name)
                  last.name += payload.delta_tool_call_name;
                if (payload.delta_tool_call_arguments)
                  last.arguments += payload.delta_tool_call_arguments;
              }
              return { ...m, toolCalls };
            }),
          });
        }
      }

      if (payload.usage) {
        const u = payload.usage;
        const ctx =
          u.context_window_tokens && u.context_window_tokens > 0
            ? u.context_window_tokens
            : 128_000;
        store.setState({
          contextUsage: {
            used_tokens: u.prompt_tokens,
            max_tokens: ctx,
            percentage: u.prompt_tokens / ctx,
          },
        });
      }

      if (payload.finish_reason) {
        coalescer.flush();
        seqDedup.reset();
        store.getState().finishStream(payload.usage);
      }
    }),
  );

  return () => {
    disposed = true;
    coalescer.dispose();
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
}
