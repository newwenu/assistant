export interface ProviderFormatDef {
  id: string;
  label: string;
  description: string;
  needsApiKey: boolean;
  keyPlaceholder?: string;
  color?: string;
}

const formatMap = new Map<string, ProviderFormatDef>();

export function defineProviderFormat(
  def: ProviderFormatDef,
): ProviderFormatDef {
  formatMap.set(def.id, def);
  return def;
}

export function getProviderFormat(id: string): ProviderFormatDef | undefined {
  return formatMap.get(id);
}

export function getAllProviderFormats(): ProviderFormatDef[] {
  return Array.from(formatMap.values());
}

export function getProviderFormatLabel(id: string): string {
  return formatMap.get(id)?.label ?? id;
}

export function getProviderFormatColor(id: string): string {
  return formatMap.get(id)?.color ?? "default";
}

export function providerFormatNeedsApiKey(id: string): boolean {
  return formatMap.get(id)?.needsApiKey ?? true;
}
