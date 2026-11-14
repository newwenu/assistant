import type {
  AgentToolCall,
  AgentToolCallPayload,
  Message,
  MessageToolCall,
  ToolCallView,
} from "./chatTypes";
import type { MessageDto } from "../../types/protocol.contract";

function asRecord(value: unknown): Record<string, unknown> | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value))
    return undefined;
  return value as Record<string, unknown>;
}

/** 增量 JSON 不完整时返回 `undefined`，由调用方保留上一份可解析快照。 */
function parseArgs(
  text: string | null | undefined,
): Record<string, unknown> | undefined {
  if (!text) return undefined;
  try {
    return asRecord(JSON.parse(text));
  } catch {
    return undefined;
  }
}

function upsert(
  views: ToolCallView[],
  callId: string,
  name: string,
  patch: Partial<ToolCallView>,
): ToolCallView[] {
  const index = views.findIndex((view) => view.callId === callId);
  if (index < 0) {
    return [...views, { callId, name, status: "running", ...patch }];
  }
  const next = [...views];
  next[index] = { ...next[index], ...(name ? { name } : {}), ...patch };
  return next;
}

/** 历史 `tool_calls_json` 的只读视图：只出现 `success` / `error` / `cancelled` / `missing` 终态。 */
export function toolCallsFromHistory(
  calls: MessageToolCall[] | undefined | null,
): ToolCallView[] | undefined {
  if (!calls || calls.length === 0) return undefined;
  return calls.map((call) => ({
    callId: call.callId,
    name: call.name,
    status: call.status,
    arguments: asRecord(call.arguments),
    resultText: call.result,
    compact: call.compact,
  }));
}

/** 线格式消息 → 前端消息：`toolCalls` 从历史 DTO 转为视图。 */
export function toChatMessage(dto: MessageDto): Message {
  const { toolCalls, ...rest } = dto;
  const views = toolCallsFromHistory(toolCalls);
  return views ? { ...rest, toolCalls: views } : rest;
}

/** `AgentToolCallPayloadDto.phase` → `ToolCallView.status`（I38d §3.1）。 */
export function applyAgentToolCall(
  views: ToolCallView[],
  call: AgentToolCall,
  now: number,
): ToolCallView[] {
  const payload: AgentToolCallPayload = call.payload;

  switch (payload.phase) {
    case "started":
      return upsert(views, call.callId, call.name, {
        status: "running",
        startedAt: now,
        finishedAt: null,
        resultText: null,
        compact: null,
        argsText: "",
        argsPreview: payload.argsPreview,
      });
    case "argsDelta": {
      const current = views.find((view) => view.callId === call.callId);
      const argsText = `${current?.argsText ?? ""}${payload.delta}`;
      return upsert(views, call.callId, call.name, {
        status: "running",
        argsText,
        arguments: parseArgs(argsText) ?? current?.arguments,
      });
    }
    case "finished":
      return upsert(views, call.callId, call.name, {
        status: "success",
        resultText: payload.result,
        compact: payload.compact,
        finishedAt: now,
        confirmationWaitMs: payload.confirmationWaitMs,
      });
    case "failed":
      return upsert(views, call.callId, call.name, {
        status: "error",
        resultText: payload.error,
        compact: payload.compact,
        errorKind: payload.errorKind,
        finishedAt: now,
        confirmationWaitMs: payload.confirmationWaitMs,
      });
    case "awaitingApproval": {
      const current = views.find((view) => view.callId === call.callId);
      return upsert(views, call.callId, call.name, {
        status: "awaiting_user",
        approval: payload.approval,
        arguments: asRecord(payload.approval.arguments),
        startedAt: current?.startedAt ?? now,
      });
    }
    case "approvalClosed":
      return upsert(views, call.callId, call.name, {
        status: "cancelled",
        approval: null,
        finishedAt: now,
      });
    case "awaitingAnswer": {
      const current = views.find((view) => view.callId === call.callId);
      return upsert(views, call.callId, call.name, {
        status: "awaiting_user",
        question: payload.question,
        approval: null,
        startedAt: current?.startedAt ?? now,
      });
    }
  }
}

/** turn 终止时把未收尾的调用收敛到终态，避免历史留下 `running`。 */
export function cancelPendingToolCalls(
  views: ToolCallView[],
  now: number,
): ToolCallView[] {
  let changed = false;
  const next = views.map((view) => {
    if (view.status !== "running" && view.status !== "awaiting_user")
      return view;
    changed = true;
    return {
      ...view,
      status: "cancelled" as const,
      approval: null,
      question: null,
      finishedAt: now,
    };
  });
  return changed ? next : views;
}

export function hasPendingToolCalls(views: ToolCallView[]): boolean {
  return views.some(
    (view) => view.status === "running" || view.status === "awaiting_user",
  );
}

/** 找到发起该工具调用的那条消息；补拉快照只带 `callId`，靠历史行里的调用列表定位。 */
export function findToolCallOwner(
  messages: Message[],
  callId: string,
): Message | undefined {
  return messages.find((message) =>
    message.toolCalls?.some((view) => view.callId === callId),
  );
}

/**
 * 补拉快照回挂：把挂起的调用覆盖到发起它的那条历史消息上。
 *
 * 等待期间该调用在库里只有「已发起、无结果行」，历史视图会渲染成 `missing`；
 * 补拉必须显式改写成 `awaiting_user` + `question`，否则切回会话看到的是
 * 「结果缺失」的残卡而不是可作答的提问卡。定位不到的调用（历史未加载到该行）
 * 直接跳过，交给后端超时收口。
 */
export function applyPendingToolCalls(
  messages: Message[],
  calls: AgentToolCall[],
  now: number,
): Message[] {
  if (calls.length === 0) return messages;
  const callsByCallId = new Map<string, AgentToolCall[]>();
  for (const call of calls) {
    const list = callsByCallId.get(call.callId);
    if (list) list.push(call);
    else callsByCallId.set(call.callId, [call]);
  }
  let changed = false;
  const next = messages.map((message) => {
    if (!message.toolCalls?.length) return message;
    const patches = message.toolCalls.flatMap(
      (view) => callsByCallId.get(view.callId) ?? [],
    );
    if (patches.length === 0) return message;
    changed = true;
    return {
      ...message,
      toolCalls: patches.reduce(
        (views, call) => applyAgentToolCall(views, call, now),
        message.toolCalls,
      ),
    };
  });
  return changed ? next : messages;
}
