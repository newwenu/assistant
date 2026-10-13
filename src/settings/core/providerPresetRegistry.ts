export interface ProviderPresetDef {
  slug: string;
  label: string;
  format: string;
  defaultBaseUrl: string;
}

const presetMap = new Map<string, ProviderPresetDef>();

export function defineProviderPreset(
  def: ProviderPresetDef,
): ProviderPresetDef {
  presetMap.set(def.slug, def);
  return def;
}

export function getProviderPreset(slug: string): ProviderPresetDef | undefined {
  return presetMap.get(slug);
}

export function getAllProviderPresets(): ProviderPresetDef[] {
  return Array.from(presetMap.values());
}

export function getPresetsByFormat(format: string): ProviderPresetDef[] {
  return Array.from(presetMap.values()).filter((p) => p.format === format);
}

export function getApiFormatByType(type: string): string {
  const preset = presetMap.get(type);
  if (preset) return preset.format;
  return "openai";
}
