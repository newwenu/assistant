export interface AgentKindDef {
  id: string;
  label: string;
  color: string;
}

const kindMap = new Map<string, AgentKindDef>();

export function defineAgentKind(def: AgentKindDef): AgentKindDef {
  kindMap.set(def.id, def);
  return def;
}

export function getAgentKind(id: string): AgentKindDef | undefined {
  return kindMap.get(id);
}

export function getAllAgentKinds(): AgentKindDef[] {
  return Array.from(kindMap.values());
}

export function getAgentKindLabelKey(id: string): string {
  return kindMap.get(id)?.label ?? id;
}

export function getAgentKindColor(id: string): string {
  return kindMap.get(id)?.color ?? "default";
}

export interface BuiltinModeDefaults {
  prompt: string;
  temperature: number | null;
  maxTokens: number | null;
  reasoningEnabled: boolean | null;
  reasoningEffort: string | null;
}

const builtinDefaultsMap = new Map<string, BuiltinModeDefaults>();

export function defineBuiltinModeDefaults(
  id: string,
  defaults: BuiltinModeDefaults,
): void {
  builtinDefaultsMap.set(id, defaults);
}

export function getBuiltinModeDefaults(
  id: string,
): BuiltinModeDefaults | undefined {
  return builtinDefaultsMap.get(id);
}
