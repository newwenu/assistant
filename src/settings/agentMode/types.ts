export interface AgentMode {
  id: string;
  name: string;
  description: string;
  kind: "default" | "quick" | "deep" | "custom";
  prompt: string;
  model: string | null;
  temperature: number | null;
  max_tokens: number | null;
  reasoning_enabled: boolean | null;
  reasoning_effort: string | null;
  tools: string[];
  presentation_rank: number;
  is_builtin: boolean;
  created_at: number;
  updated_at: number;
}

export interface AgentModeInput {
  id: string;
  name: string;
  description: string;
  kind: AgentMode["kind"];
  prompt: string;
  model: string | null;
  temperature: number | null;
  max_tokens: number | null;
  reasoning_enabled: boolean | null;
  reasoning_effort: string | null;
  tools: string[];
  presentation_rank: number;
  is_builtin: boolean;
  created_at: number;
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
    max_tokens: mode.max_tokens ?? null,
    reasoning_enabled: mode.reasoning_enabled ?? null,
    reasoning_effort: mode.reasoning_effort ?? null,
    tools: mode.tools || [],
    presentation_rank: mode.presentation_rank ?? 99,
    is_builtin: mode.is_builtin ?? false,
    created_at: mode.created_at ?? Date.now(),
  };
}
