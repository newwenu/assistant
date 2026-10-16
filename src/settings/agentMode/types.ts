export interface AgentMode {
  id: string;
  name: string;
  description: string;
  kind: "default" | "quick" | "deep" | "custom";
  prompt: string;
  model: string | null;
  temperature: number | null;
  maxTokens: number | null;
  reasoningEnabled: boolean | null;
  reasoningEffort: string | null;
  tools: string[];
  presentationRank: number;
  isBuiltin: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface AgentModeInput {
  id: string;
  name: string;
  description: string;
  kind: AgentMode["kind"];
  prompt: string;
  model: string | null;
  temperature: number | null;
  maxTokens: number | null;
  reasoningEnabled: boolean | null;
  reasoningEffort: string | null;
  tools: string[];
  presentationRank: number;
  isBuiltin: boolean;
  createdAt: number;
}

export function toModeInput(
  mode: Partial<AgentMode> & { id?: string },
): AgentModeInput {
  return {
    id: mode.id || "",
    name: mode.name || "",
    description: mode.description || "",
    kind: mode.kind || "custom",
    prompt: mode.prompt || "",
    model: mode.model ?? null,
    temperature: mode.temperature ?? null,
    maxTokens: mode.maxTokens ?? null,
    reasoningEnabled: mode.reasoningEnabled ?? null,
    reasoningEffort: mode.reasoningEffort ?? null,
    tools: mode.tools || [],
    presentationRank: mode.presentationRank ?? 99,
    isBuiltin: mode.isBuiltin ?? false,
    createdAt: mode.createdAt ?? Date.now(),
  };
}
