import type { Message } from "../../chat/core/chatTypes";
import type { AgentToolCall, Conversation } from "../../chat/core/chatTypes";
import {
  applyAgentToolCall,
  cancelPendingToolCalls,
  findToolCallOwner,
} from "../../chat/core/toolCallView";
import { createOptimisticMessage } from "../../chat/core/optimisticMessage";
import { useChatStreamStore } from "../chatStreamStore";

interface SegmentsWritable {
  messages: Message[];
}

interface SegmentsReadable {
  messages: Message[];
  currentConversationId: string | null;
  /** 会话列表：仅用于取本轮身份字段（`modeId` / `providerId` / `modelId`）。 */
  conversations?: Conversation[];
}

/**
 * agent 私有段表：`iteration → 段消息 id`。
 *
 * 气泡边界 = 迭代（I38d §3.1.1）：每次迭代一条独立消息、一个独立气泡，
 * 工具卡片按事件 `iteration` 归到发起它的那条消息。这份状态只服务 agent 链路，
 * 因此不放进 `chatStreamStore`，也不放进通用状态端口（§3.7）。
 */
export interface AgentSegments {
  /** 新一轮开始：把首个占位消息绑定到迭代 1。 */
  reset: (assistantMsgId: string) => void;
  /** 切回运行中的会话：真实行 id 未知，首个增量必须开新段。 */
  markUnknownSegment: () => void;
  /** 补拉快照接管：已知「当前段就是历史里的哪一行」，把段表直接绑定到该行。 */
  adoptSegment: (iteration: number, messageId: string) => void;
  /**
   * 从已加载消息重建映射（I46 §3.3）：坐标是事实源，段表只是缓存。
   * 刷新 / 重开会话后 `currentIteration` 由消息行还原，避免整轮失忆。
   */
  rebuildFromMessages: (messages: Message[], turnIndex: number | null) => void;
  /** 真实行 id 就绪：把占位 id 在段表里换成真实 id。 */
  renameSegment: (previousId: string, realMessageId: string) => void;
  appendDelta: (iteration: number, delta: string) => void;
  appendReasoningDelta: (iteration: number, delta: string) => void;
  applyToolCall: (iteration: number, call: AgentToolCall) => void;
  /** 提交当前段正文，但不结束流式态（迭代结束 ≠ turn 结束）。 */
  commitSegment: () => void;
  /** turn 收尾：把仍挂起的工具调用收敛到终态，避免历史留下 `running`。 */
  settleToolCalls: () => void;
}

export function createAgentSegments(
  set: (partial: Partial<SegmentsWritable>) => void,
  get: () => SegmentsReadable,
): AgentSegments {
  const segmentMessageIdByIteration = new Map<number, string>();
  let segmentSeq = 0;
  let currentIteration: number | null = null;
  let pendingNewSegment = false;

  /** 已加载消息里最大的 turn 序号：仅在调用方拿不到本轮坐标时作兜底目标。 */
  function lastTurnIndex(messages: Message[]): number | null {
    let last: number | null = null;
    for (const m of messages) {
      const turnIndex = m.turnIndex;
      if (turnIndex === null || turnIndex === undefined) continue;
      if (last === null || turnIndex > last) last = turnIndex;
    }
    return last;
  }

  function makePlaceholder(
    id: string,
    conversationId: string | null,
    parentId: string | null,
    iteration: number,
  ): Message {
    // 身份字段取会话现值，与落库后回读一致（I43 §4.1）；会话不在列表时兜底 `null`。
    const conv = get().conversations?.find((c) => c.id === conversationId);
    return createOptimisticMessage({
      id,
      conversationId: conversationId ?? "",
      role: "assistant",
      content: "",
      timestamp: Date.now(),
      parentId,
      providerId: conv?.providerId,
      modelId: conv?.modelId,
      modeId: conv?.modeId,
      // 坐标来自 wire（I46 §3.2）：turn 序号取本轮事件下发的值，迭代号取事件自带的值。
      turnIndex: useChatStreamStore.getState().currentTurnIndex,
      iteration,
    });
  }

  function commitSegmentContent() {
    const stream = useChatStreamStore.getState();
    const messageId = stream.streamingMessageId;
    if (!messageId || !stream.streamingContent) return;
    const content = stream.streamingContent;
    set({
      messages: get().messages.map((m) =>
        m.id === messageId ? { ...m, content } : m,
      ),
    });
  }

  /** 开新的一段（新气泡）：先提交上一段，再追加占位消息并切换缓冲。 */
  function openSegment(iteration: number): string {
    commitSegmentContent();
    const stream = useChatStreamStore.getState();
    const state = get();
    const id = `temp-assistant-${Date.now()}-${++segmentSeq}`;
    const parentId =
      stream.streamingMessageId ??
      state.messages[state.messages.length - 1]?.id ??
      null;
    segmentMessageIdByIteration.set(iteration, id);
    currentIteration = iteration;
    pendingNewSegment = false;
    set({
      messages: [
        ...state.messages,
        makePlaceholder(id, state.currentConversationId, parentId, iteration),
      ],
    });
    useChatStreamStore.getState().beginSegment(id, iteration);
    return id;
  }

  /** 按坐标在已加载消息里找同迭代的 assistant 行（取最后一条，即最新写入的那行）。 */
  function findByCoords(iteration: number): Message | null {
    const turnIndex = useChatStreamStore.getState().currentTurnIndex;
    if (turnIndex === null) return null;
    let found: Message | null = null;
    for (const m of get().messages) {
      if (m.role !== "assistant") continue;
      if (m.turnIndex !== turnIndex || m.iteration !== iteration) continue;
      found = m;
    }
    return found;
  }

  /** 取该迭代对应的段消息；未建则按需开段，落后于当前段的旧事件丢弃。 */
  function resolveSegment(iteration: number): string | null {
    const existing = segmentMessageIdByIteration.get(iteration);
    if (existing) return existing;
    // 缓存未命中先查已加载消息：坐标是事实源，段表只是缓存（I46 §3.3）。
    const known = findByCoords(iteration);
    if (known) {
      segmentMessageIdByIteration.set(iteration, known.id);
      currentIteration = iteration;
      pendingNewSegment = false;
      useChatStreamStore.getState().setStreamingIteration(iteration);
      return known.id;
    }
    if (currentIteration !== null && iteration <= currentIteration) return null;
    return openSegment(iteration);
  }

  /**
   * 从消息重建段表（I46 §3.3）：坐标是事实源，段表只是缓存。
   * 冲突时以消息行为准；`currentIteration` 取同轮最大迭代号，
   * 使仍在飞的下一迭代（库中尚无行）由首个增量开新段。
   */
  function applyRebuild(messages: Message[], turnIndex: number | null) {
    segmentMessageIdByIteration.clear();
    segmentSeq = 0;
    currentIteration = null;
    pendingNewSegment = true;
    const target = turnIndex ?? lastTurnIndex(messages);
    if (target === null) return;
    let maxIteration = 0;
    for (const m of messages) {
      if (m.role !== "assistant") continue;
      if (m.turnIndex !== target) continue;
      if (m.iteration < 1) continue;
      segmentMessageIdByIteration.set(m.iteration, m.id);
      if (m.iteration > maxIteration) maxIteration = m.iteration;
    }
    if (maxIteration > 0) {
      currentIteration = maxIteration;
      pendingNewSegment = false;
      useChatStreamStore.getState().setStreamingIteration(maxIteration);
    }
  }

  return {
    reset: (assistantMsgId: string) => {
      segmentMessageIdByIteration.clear();
      segmentSeq = 0;
      currentIteration = 1;
      pendingNewSegment = false;
      segmentMessageIdByIteration.set(1, assistantMsgId);
    },

    markUnknownSegment: () => {
      // 刷新 / 重开会话后段表为空，先用已加载消息重建（I46 §3.3）：
      // 已落库的迭代能对上历史行，仍在飞的迭代（无行）由首个增量开新段。
      applyRebuild(
        get().messages,
        useChatStreamStore.getState().currentTurnIndex,
      );
    },

    rebuildFromMessages: applyRebuild,

    adoptSegment: (iteration: number, messageId: string) => {
      segmentMessageIdByIteration.clear();
      segmentMessageIdByIteration.set(iteration, messageId);
      currentIteration = iteration;
      pendingNewSegment = false;
      useChatStreamStore.getState().setStreamingIteration(iteration);
    },

    renameSegment: (previousId: string, realMessageId: string) => {
      for (const [iteration, id] of segmentMessageIdByIteration) {
        if (id === previousId) {
          segmentMessageIdByIteration.set(iteration, realMessageId);
        }
      }
    },

    appendDelta: (iteration: number, delta: string) => {
      const current = currentIteration;
      if (current !== null && iteration < current) return;
      const needsNewSegment =
        current === null ? pendingNewSegment : iteration > current;
      if (needsNewSegment) {
        openSegment(iteration);
      } else if (current === null) {
        const streamingMessageId =
          useChatStreamStore.getState().streamingMessageId;
        if (streamingMessageId) {
          segmentMessageIdByIteration.set(iteration, streamingMessageId);
        }
        currentIteration = iteration;
      }
      useChatStreamStore.getState().appendDelta(delta);
    },

    appendReasoningDelta: (iteration: number, delta: string) => {
      const messageId = resolveSegment(iteration);
      if (!messageId) return;
      const state = get();
      set({
        messages: state.messages.map((m) =>
          m.id === messageId
            ? { ...m, reasoning: (m.reasoning || "") + delta }
            : m,
        ),
      });
    },

    applyToolCall: (iteration: number, call: AgentToolCall) => {
      /**
       * 恢复场景（段表未绑定该迭代）里，若历史已加载发起这条调用的行，
       * 直接回挂到该行：否则 `resolveSegment` 会为同一次迭代再开一个空气泡，
       * 出现「一次迭代两个气泡」的瞬时重复（审批 / 提问恢复同样走这里）。
       */
      if (!segmentMessageIdByIteration.has(iteration)) {
        const owner = findToolCallOwner(get().messages, call.callId);
        if (owner) {
          segmentMessageIdByIteration.set(iteration, owner.id);
          currentIteration = iteration;
          pendingNewSegment = false;
          set({
            messages: get().messages.map((m) =>
              m.id === owner.id
                ? {
                    ...m,
                    toolCalls: applyAgentToolCall(
                      m.toolCalls ?? [],
                      call,
                      Date.now(),
                    ),
                  }
                : m,
            ),
          });
          return;
        }
      }
      const messageId = resolveSegment(iteration);
      if (!messageId) return;
      const state = get();
      set({
        messages: state.messages.map((m) =>
          m.id === messageId
            ? {
                ...m,
                toolCalls: applyAgentToolCall(
                  m.toolCalls ?? [],
                  call,
                  Date.now(),
                ),
              }
            : m,
        ),
      });
    },

    commitSegment: () => {
      commitSegmentContent();
    },

    settleToolCalls: () => {
      const state = get();
      const now = Date.now();
      let changed = false;
      const messages = state.messages.map((m) => {
        if (!m.toolCalls?.length) return m;
        const settled = cancelPendingToolCalls(m.toolCalls, now);
        if (settled === m.toolCalls) return m;
        changed = true;
        return { ...m, toolCalls: settled };
      });
      if (changed) set({ messages });
    },
  };
}
