import type { Message, ToolCallView } from "../core/chatTypes";
import { summarizeArguments } from "../tool/toolCallDisplay";
import { PROCESS_COLLAPSE_THRESHOLD } from "../../config/constants";

export type GroupPhase = "collecting" | "settled";

/** 一个连续步段（run）的统计；`id` 由首步键（`callId` 或 `reasoning:<messageId>`）生成，跨渲染稳定。 */
export interface NodeRunStats {
  id: string;
  steps: number;
  /** 与工具步同一 run、同一展开开关，但组头单独外显。 */
  reasoning: number;
  failed: number;
  durationMs: number | null;
}

export type NodeView =
  | {
      kind: "message";
      message: Message;
      showIdentity: boolean;
      ownsAssistantChrome: boolean;
      /** `true`：气泡无正文可渲染，只承载身份行 / 尾锚点 / 错误，不画空壳（I46 §6.1）。 */
      chromeOnly: boolean;
    }
  | {
      kind: "tool";
      callId: string;
      view: ToolCallView;
      /** 所属折叠 run；`null` 表示 run 外的步（待处理 / 无摘要 / 流式中），恒以卡片显示。 */
      run: NodeRunStats | null;
    }
  | {
      kind: "reasoning";
      messageId: string;
      text: string;
      /** 所属折叠 run；`null` 表示 run 外的思考（流式中 / 纯思考段），走 `ReasoningBlock` 自折叠。 */
      run: NodeRunStats | null;
    }
  /**
   * 身份行节点：同一消息的思考上移为节点时，身份行从气泡拆出、排在最前，
   * 否则思考节点会渲染在头像之前（I46 §5.5 挂载顺序）。
   */
  | { kind: "identity"; message: Message };

export interface NodeGroupStats {
  steps: number;
  failed: number;
  durationMs: number | null;
}

export interface NodeGroupView {
  turnIndex: number | null;
  nodes: NodeView[];
  phase: GroupPhase;
  isLastGroupInTurn: boolean;
  needsAttention: boolean;
  /** 只读快照，禁止参与决策（门禁 16）。 */
  link: "chat" | "agent";
  stats: NodeGroupStats;
  collapsible: boolean;
  /** turn 尾锚点；`null` 时本组不渲染 `TurnTail`。 */
  tailAnchor: Message | null;
}

export interface GroupNodeRowsInput {
  /** 已过滤 `role === "tool"` 的可见消息，顺序沿用 I46 §2.7 键序。 */
  messages: Message[];
  /** 只读快照，写入视图供断言与调试；函数体内不得据此分支。 */
  link: "chat" | "agent";
  phase: GroupPhase;
  /** 当前流式 turn 序号，唯一来源是 `chat-stream-start`（I46 §5.3）。 */
  currentTurnIndex: number | null;
  /** 流式占位行 id：`currentTurnIndex` 尚未下发时用它兜住 collecting 判定。 */
  streamingMessageId?: string | null;
}

function isAssistant(message: Message): boolean {
  return message.role === "assistant";
}

function hasContent(message: Message): boolean {
  return message.content.trim().length > 0;
}

/** 思考行的稳定键：思考没有独立 id，按所属消息派生（`reasoning:<messageId>`）。 */
function reasoningKeyOf(message: Message): string | null {
  if (!isAssistant(message)) return null;
  const text = message.reasoning;
  if (typeof text !== "string" || text.trim().length === 0) return null;
  return `reasoning:${message.id}`;
}

function collectToolViews(message: Message): ToolCallView[] {
  return isAssistant(message) ? (message.toolCalls ?? []) : [];
}

function computeStats(toolViews: ToolCallView[]): NodeGroupStats {
  let failed = 0;
  let minStart: number | null = null;
  let maxFinish: number | null = null;
  for (const view of toolViews) {
    if (view.status === "error" || view.status === "cancelled") failed++;
    if (view.startedAt != null) {
      minStart =
        minStart === null ? view.startedAt : Math.min(minStart, view.startedAt);
    }
    if (view.finishedAt != null) {
      maxFinish =
        maxFinish === null
          ? view.finishedAt
          : Math.max(maxFinish, view.finishedAt);
    }
  }
  const durationMs =
    minStart !== null && maxFinish !== null && maxFinish >= minStart
      ? maxFinish - minStart
      : null;
  return { steps: toolViews.length, failed, durationMs };
}

function needsAttention(
  toolViews: ToolCallView[],
  messages: Message[],
): boolean {
  if (
    toolViews.some((v) => v.status === "error" || v.status === "awaiting_user")
  ) {
    return true;
  }
  return messages.some((m) => m.error != null);
}

function hasBoundaryNode(messages: Message[]): boolean {
  return messages.some((m) => m.role === "compact_summary");
}

/**
 * 摘要优先 `compact.head`，旧数据退回 `summarizeArguments`；两者皆无则**不折叠**
 * （I46 compact §2.12 规则 5 / 门禁 14，宁可高不可空）。
 */
function hasStepSummary(view: ToolCallView): boolean {
  const head = view.compact?.head;
  if (typeof head === "string" && head.trim().length > 0) return true;
  const summary = summarizeArguments(view.arguments);
  return typeof summary === "string" && summary.trim().length > 0;
}

/**
 * 成功与失败的步只要能取到摘要就压成单行：失败原因已由 `compact.head` 承载，
 * 单行同时显示状态与原因；`running` / `awaiting_user` / `missing` / `cancelled` 仍整卡显示并**切断** run（D8）。
 */
function isFoldableStep(view: ToolCallView): boolean {
  return (
    (view.status === "success" || view.status === "error") &&
    hasStepSummary(view)
  );
}

interface RunEntry {
  key: string;
  view: ToolCallView | null;
}

function runStats(id: string, entries: RunEntry[]): NodeRunStats {
  const views = entries.flatMap((entry) => (entry.view ? [entry.view] : []));
  const { failed, durationMs } = computeStats(views);
  return {
    id,
    steps: views.length,
    reasoning: entries.length - views.length,
    failed,
    durationMs,
  };
}

/**
 * 按「连续可折叠步」切 run（I46 §5.4）：run 是折叠的最小单位，各自计数、各自展开态。
 * 待处理 / 无摘要 / 带消息级错误 / 边界行，以及任何带正文的行都切断 run——
 * 否则组头会被气泡打断重复渲染，且每处都显示整段总步数，与实际行数不符。
 * 思考行（`reasoningAsNode`）与工具步同权：非空即可折叠，不切断 run。
 */
function planRuns(
  slice: Message[],
  foldAllowed: boolean,
  reasoningAsNode: boolean,
): { runs: NodeRunStats[]; runByKey: Map<string, NodeRunStats> } {
  const runs: NodeRunStats[] = [];
  const runByKey = new Map<string, NodeRunStats>();
  const seenCallIds = new Set<string>();
  let pending: RunEntry[] = [];
  let cutNext = false;

  const flush = () => {
    if (pending.length === 0) return;
    // 纯思考段不成组（否则出现「执行过程 · 1 思考」这种重复组头），由 `ReasoningBlock` 自折叠。
    const hasToolStep = pending.some((entry) => entry.view !== null);
    if (
      foldAllowed &&
      hasToolStep &&
      pending.length >= PROCESS_COLLAPSE_THRESHOLD
    ) {
      const stats = runStats(`run:${pending[0].key}`, pending);
      runs.push(stats);
      for (const entry of pending) runByKey.set(entry.key, stats);
    }
    pending = [];
  };

  const push = (entry: RunEntry) => {
    if (cutNext) {
      flush();
      cutNext = false;
    }
    pending.push(entry);
  };

  for (const message of slice) {
    const blocked = message.role === "compact_summary" || message.error != null;
    const reasoningKey = reasoningAsNode ? reasoningKeyOf(message) : null;
    // 思考行在节点序列里排在正文之前，因此先入 run、再按正文切段。
    if (reasoningKey !== null) push({ key: reasoningKey, view: null });
    // 正文行在节点序列里自成一段，必须切断 run 以保证「组头 + 步」连续。
    if (blocked || hasContent(message)) flush();
    for (const view of collectToolViews(message)) {
      // 同一 turn 内重复下发的 `callId` 只计一次：否则组头步数虚高，且重复 React key 让实际行数少于计数。
      if (seenCallIds.has(view.callId)) continue;
      if (blocked || !isFoldableStep(view)) {
        seenCallIds.add(view.callId);
        flush();
        cutNext = true;
        continue;
      }
      seenCallIds.add(view.callId);
      push({ key: view.callId, view });
    }
    if (blocked) cutNext = true;
  }
  flush();
  return { runs, runByKey };
}

/** ADR D3 三级优先级：有正文且 settled 的 assistant 行 → 末个 assistant 行 → null。 */
function pickTailAnchor(messages: Message[]): Message | null {
  let fallback: Message | null = null;
  for (let i = messages.length - 1; i >= 0; i--) {
    const candidate = messages[i];
    if (!isAssistant(candidate)) continue;
    if (fallback === null) fallback = candidate;
    if (
      candidate.content.trim().length > 0 &&
      candidate.status !== "streaming"
    ) {
      return candidate;
    }
  }
  return fallback;
}

/**
 * 分组投影（I46 §5）：纯函数，无 I/O、无时间、无随机、不读 store。
 * 分组只看 `turnIndex`；`link` 仅写入视图，不参与任何分支（门禁 16）。
 */
export function groupNodeRows(input: GroupNodeRowsInput): NodeGroupView[] {
  const { messages, link, phase, currentTurnIndex, streamingMessageId } = input;
  const groups: NodeGroupView[] = [];

  let i = 0;
  while (i < messages.length) {
    const turnIndex = messages[i].turnIndex ?? null;
    let end = i;
    while (
      end + 1 < messages.length &&
      (messages[end + 1].turnIndex ?? null) === turnIndex
    ) {
      end++;
    }
    const slice = messages.slice(i, end + 1);
    const prev = i > 0 ? messages[i - 1] : undefined;

    // 统计口径与渲染口径一致：重复 `callId` 只计一次。
    const toolViews = slice
      .flatMap(collectToolViews)
      .filter(
        (view, index, all) =>
          all.findIndex((other) => other.callId === view.callId) === index,
      );
    const stats = computeStats(toolViews);
    const attention = needsAttention(toolViews, slice);
    // `currentTurnIndex` 未下发时（刷新 / 恢复）用流式占位行兜住 collecting，
    // 避免运行中的 turn 被判成 settled 而提前收起、步数随增量跳动（D8）。
    const carriesStreamingRow =
      streamingMessageId != null &&
      slice.some((message) => message.id === streamingMessageId);
    const groupPhase: GroupPhase =
      phase === "collecting" &&
      turnIndex !== null &&
      (turnIndex === currentTurnIndex || carriesStreamingRow)
        ? "collecting"
        : "settled";
    const foldAllowed =
      groupPhase === "settled" && turnIndex !== null && !hasBoundaryNode(slice);

    // 思考是否上移为节点：只有本 turn 存在工具步（即存在「过程」）时才上移。
    // 零工具 turn（chat 轮、agent 纯问答轮）思考仍留在气泡内，外观与现状一致（门禁 15）。
    const reasoningAsNode = toolViews.length > 0;
    const { runs, runByKey } = planRuns(slice, foldAllowed, reasoningAsNode);

    const tailAnchor = turnIndex === null ? null : pickTailAnchor(slice);
    const nodes: NodeView[] = [];
    // 同一 turn 内重复下发的 `callId` 只渲染一次，与 `planRuns` 的去重口径一致：
    // 否则气泡里会多出一张重复卡片，且 React key 冲突会让实际行数少于计数。
    const emittedCallIds = new Set<string>();
    slice.forEach((message, offset) => {
      const messageToolViews = collectToolViews(message).filter((view) => {
        if (emittedCallIds.has(view.callId)) return false;
        emittedCallIds.add(view.callId);
        return true;
      });
      const prevMessage = offset > 0 ? slice[offset - 1] : prev;
      const hasBody = hasContent(message);
      const reasoningKey = reasoningAsNode ? reasoningKeyOf(message) : null;
      const showIdentity =
        isAssistant(message) &&
        (prevMessage === undefined ||
          prevMessage.role !== "assistant" ||
          (turnIndex !== null &&
            (prevMessage.turnIndex ?? null) !== turnIndex));
      const isTurnTail = tailAnchor !== null && tailAnchor.id === message.id;
      // 身份行必须在所有过程节点之前：思考上移时把身份行拆成独立节点，否则它会渲染在头像前（I46 §5.5）。
      const identityAsNode = reasoningKey !== null && showIdentity;
      if (identityAsNode) {
        nodes.push({ kind: "identity", message });
      }
      if (reasoningKey !== null) {
        nodes.push({
          kind: "reasoning",
          messageId: message.id,
          text: message.reasoning ?? "",
          run: runByKey.get(reasoningKey) ?? null,
        });
      }
      // 气泡只承载正文与 chrome，工具步一律按原序下移为独立节点（I46 §6.1），
      // 否则「步 → 等待卡 → 步」的时间顺序会被气泡挤到最前面。
      const needsBubble =
        hasBody ||
        message.error != null ||
        !isAssistant(message) ||
        showIdentity ||
        isTurnTail;
      if (needsBubble) {
        nodes.push({
          kind: "message",
          message: {
            ...message,
            ...(messageToolViews.length > 0 ? { toolCalls: undefined } : {}),
            ...(reasoningKey !== null ? { reasoning: null } : {}),
          },
          showIdentity: identityAsNode ? false : showIdentity,
          ownsAssistantChrome: turnIndex === null || !isAssistant(message),
          chromeOnly: !hasBody,
        });
      }
      for (const view of messageToolViews) {
        nodes.push({
          kind: "tool",
          callId: view.callId,
          view,
          run: runByKey.get(view.callId) ?? null,
        });
      }
    });

    groups.push({
      turnIndex,
      nodes,
      phase: groupPhase,
      isLastGroupInTurn:
        end + 1 >= messages.length
          ? true
          : (messages[end + 1].turnIndex ?? null) !== turnIndex,
      needsAttention: attention,
      link,
      stats,
      collapsible: runs.length > 0,
      tailAnchor: turnIndex === null ? null : pickTailAnchor(slice),
    });

    i = end + 1;
  }

  return groups;
}
