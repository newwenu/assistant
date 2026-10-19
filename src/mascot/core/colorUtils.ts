export function hexToArgb(hex: string | null | undefined): number | undefined {
  if (!hex) return undefined;
  const h = hex.replace("#", "");
  return (
    (0xff << 24) |
    (parseInt(h.slice(0, 2), 16) << 16) |
    (parseInt(h.slice(2, 4), 16) << 8) |
    parseInt(h.slice(4, 6), 16)
  );
}

export function shadeHex(hex: string, amount: number): string {
  const h = hex.replace("#", "");
  if (h.length !== 6 || /[^0-9a-fA-F]/.test(h)) return hex;
  const clamp = (v: number) => Math.max(0, Math.min(255, Math.round(v)));
  const mix = (channel: number) =>
    amount >= 0 ? channel + (255 - channel) * amount : channel * (1 + amount);
  const r = clamp(mix(parseInt(h.slice(0, 2), 16)));
  const g = clamp(mix(parseInt(h.slice(2, 4), 16)));
  const b = clamp(mix(parseInt(h.slice(4, 6), 16)));
  return `#${[r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("")}`;
}

export function argbToHex(argb: number | null | undefined): string | null {
  if (argb == null) return null;
  const r = (argb >> 16) & 0xff;
  const g = (argb >> 8) & 0xff;
  const b = argb & 0xff;
  return `#${[r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("")}`;
}
