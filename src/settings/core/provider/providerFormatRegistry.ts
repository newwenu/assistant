export interface ProviderFormatDef {
  id: string;
  label: string;
  description: string;
  needsApiKey: boolean;
  keyPlaceholder?: string;
  color?: string;
  /** 自定义 Provider 落库时的 type 值，后端 provider_kind_from_str 依此分派 */
  customType: string;
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

/** 反查：自定义 Provider 的 type 值 → 所属 format */
export function getProviderFormatByCustomType(
  customType: string,
): ProviderFormatDef | undefined {
  return Array.from(formatMap.values()).find(
    (f) => f.customType === customType,
  );
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
