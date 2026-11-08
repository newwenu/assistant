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
        argsText: "",
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
        finishedAt: now,
        confirmationWaitMs: payload.confirmationWaitMs,
      });
    case "failed":
      return upsert(views, call.callId, call.name, {
        status: "error",
        resultText: payload.error,
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
    case "awaitingAnswer":
      return upsert(views, call.callId, call.name, {
        status: "awaiting_user",
        question: payload.question,
        approval: null,
      });
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
