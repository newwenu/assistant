import type { SessionState, TurnPhase } from "../../chat/core/chatTypes";

export type ChatPhase =
  "idle" | "sending" | "streaming" | "tool_executing" | "error";

/** 后端 `TurnPhaseDto` → UI `ChatPhase`（I38d §2.3：phase 由后端驱动，前端不再自行推导）。 */
export function phaseFromTurnPhase(turnPhase: TurnPhase): ChatPhase {
  switch (turnPhase) {
    case "starting":
      return "sending";
    case "toolCalling":
    case "awaitingApproval":
      return "tool_executing";
    case "compacting":
    case "thinking":
    case "streaming":
    case "finalizing":
      return "streaming";
  }
}

/** 后端 `SessionStateDto` → UI `ChatPhase`；`error` 是终态（等下一次 `session-state-changed` 复位）。 */
export function stateToPhase(state: SessionState): ChatPhase {
  if (state === "idle") return "idle";
  if ("processing" in state) return phaseFromTurnPhase(state.processing.phase);
  return "error";
}
