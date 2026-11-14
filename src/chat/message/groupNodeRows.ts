import type { Message, ToolCallView } from "../core/chatTypes";
import { summarizeArguments } from "../tool/toolCallDisplay";
import { PROCESS_COLLAPSE_THRESHOLD } from "../../config/constants";

export type GroupPhase = "collecting" | "settled";

/** 一个连续步段（run）的统计；`id` 由首步 `callId` 生成，跨渲染稳定。 */
export interface NodeRunStats {
  id: string;
  steps: number;
  failed: number;
  durationMs: number | null;
}

export type NodeView =
  | {
      kind: "message";
      message: Message;
      showIdentity: boolean;
      ownsAssistantChrome: boolean;
    }
  | {
      kind: "tool";
      callId: string;
      view: ToolCallView;
      /** 所属折叠 run：步行只在 run 内产生，故恒非空（D8 安全阀已由 `planRuns` 切断）。 */
      run: NodeRunStats;
    };

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
  /** 本 turn 内是否存在可折叠 run；`false` 时全部步留在气泡内。 */
  collapsible: boolean;
  /** turn 尾锚点；`null` 时本组不渲染 `TurnTail`。 */
  tailAnchor: Message | null;
}

export interface GroupNodeRowsInput {
  /** 已过滤 `role === "tool"` 的可见消息，顺序沿用 §2.7 键序。 */
  messages: Message[];
  /** 只读快照，写入视图供断言与调试；函数体内不得据此分支。 */
  link: "chat" | "agent";
  /** 会话级流式相位：`collecting` 表示有 turn 正在运行。 */
  phase: GroupPhase;
  /** 当前流式 turn 序号，唯一来源是 `chat-stream-start`（I46 §5.3）。 */
  currentTurnIndex: number | null;
  /** 流式占位行 id：`currentTurnIndex` 尚未下发时用它兜住 collecting 判定。 */
  streamingMessageId?: string | null;
}

function isAssistant(message: Message): boolean {
  return message.role === "assistant";
}

function hasVisibleBody(message: Message): boolean {
  return message.content.trim().length > 0 || !!message.reasoning;
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
 * 单步折叠行必须能取到摘要（I46 compact §2.12 规则 5 / 门禁 14）：
 * 优先 `compact.head`；旧数据（`compact == null`）退回 `summarizeArguments`。
 * 两者皆无则**不折叠**——宁可高，不可空。
 */
function hasStepSummary(view: ToolCallView): boolean {
  const head = view.compact?.head;
  if (typeof head === "string" && head.trim().length > 0) return true;
  const summary = summarizeArguments(view.arguments);
  return typeof summary === "string" && summary.trim().length > 0;
}

/**
 * 可折叠的步（D8 安全阀）：只有成功且能取到摘要的步允许被压成单行。
 * `running` / `error` / `awaiting_user` / `missing` / `cancelled` 一律完整显示，
 * 并且**切断**所在的 run——失败原因是结论的一部分，不能藏进折叠组。
 */
function isFoldableStep(view: ToolCallView): boolean {
  return view.status === "success" && hasStepSummary(view);
}

function runStats(id: string, entries: ToolCallView[]): NodeRunStats {
  const { failed, durationMs } = computeStats(entries);
  return { id, steps: entries.length, failed, durationMs };
}

/**
 * 按「连续可折叠步」切 run（I46 §5.4）：run 是折叠的最小单位，
 * 每个 run 各自计数、各自判定阈值与展开态，互不污染。
 *
 * 切断条件：失败 / 待处理 / 无摘要的步、带消息级错误的行、边界行，
 * **以及任何带可见正文的行**。正文行会在节点序列里插进一个气泡节点，
 * 若不切断，同一 run 的步会被气泡打断，组头在该段前重复渲染且每处都
 * 显示整段总步数——组头计数与实际行数不符（「所有节点共用一个步数」）。
 */
function planRuns(
  slice: Message[],
  foldAllowed: boolean,
): { runs: NodeRunStats[]; runByCallId: Map<string, NodeRunStats> } {
  const runs: NodeRunStats[] = [];
  const runByCallId = new Map<string, NodeRunStats>();
  const seenCallIds = new Set<string>();
  let pending: ToolCallView[] = [];
  let cutNext = false;

  const flush = () => {
    if (pending.length === 0) return;
    if (foldAllowed && pending.length >= PROCESS_COLLAPSE_THRESHOLD) {
      const stats = runStats(`run:${pending[0].callId}`, pending);
      runs.push(stats);
      for (const view of pending) runByCallId.set(view.callId, stats);
    }
    pending = [];
  };

  for (const message of slice) {
    const blocked = message.role === "compact_summary" || message.error != null;
    // 正文 / 推理行在节点序列里自成一段，必须切断 run 以保证「组头 + 步」连续。
    if (blocked || hasVisibleBody(message)) flush();
    for (const view of collectToolViews(message)) {
      // 同一 turn 内重复下发的 `callId` 只计一次：否则组头步数会虚高，
      // 且重复的 React key 会让实际渲染行数少于计数。
      if (seenCallIds.has(view.callId)) continue;
      if (blocked || !isFoldableStep(view)) {
        seenCallIds.add(view.callId);
        flush();
        cutNext = true;
        continue;
      }
      if (cutNext) {
        flush();
        cutNext = false;
      }
      seenCallIds.add(view.callId);
      pending.push(view);
    }
    if (blocked) cutNext = true;
  }
  flush();
  return { runs, runByCallId };
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

    const { runs, runByCallId } = planRuns(slice, foldAllowed);

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
      // 只有 run 内的步上移为独立步行；run 外的步（失败 / 待处理 / 无摘要）
      // 必须留在气泡里完整显示（D8 安全阀）——不能因为同条消息里还有可折叠步而消失。
      const folded = messageToolViews.filter((view) =>
        runByCallId.has(view.callId),
      );
      const remaining = messageToolViews.filter(
        (view) => !runByCallId.has(view.callId),
      );
      const prevMessage = offset > 0 ? slice[offset - 1] : prev;
      if (
        folded.length === 0 ||
        remaining.length > 0 ||
        hasVisibleBody(message)
      ) {
        const showIdentity =
          isAssistant(message) &&
          (prevMessage === undefined ||
            prevMessage.role !== "assistant" ||
            (turnIndex !== null &&
              (prevMessage.turnIndex ?? null) !== turnIndex));
        nodes.push({
          kind: "message",
          // 折叠态下工具行上移为独立步行，气泡只保留 run 外的步（§6.1）。
          message:
            folded.length === 0
              ? message
              : {
                  ...message,
                  toolCalls: remaining.length > 0 ? remaining : undefined,
                },
          showIdentity,
          ownsAssistantChrome: turnIndex === null || !isAssistant(message),
        });
      }
      for (const view of folded) {
        const run = runByCallId.get(view.callId);
        if (run === undefined) continue;
        nodes.push({ kind: "tool", callId: view.callId, view, run });
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
