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
