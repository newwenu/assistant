import { colord, extend } from "colord";
import cmykPlugin from "colord/plugins/cmyk";
import hwbPlugin from "colord/plugins/hwb";
import lchPlugin from "colord/plugins/lch";
import namesPlugin from "colord/plugins/names";

extend([cmykPlugin, hwbPlugin, lchPlugin, namesPlugin]);

export type ColordInstance = ReturnType<typeof colord>;

export interface FormatDef {
  label: string;
  format: (c: ColordInstance) => string;
  placeholder: string;
}

export const FORMATS: FormatDef[] = [
  { label: "HEX", format: (c) => c.toHex(), placeholder: "#ff0000" },
  {
    label: "RGB",
    format: (c) => c.toRgbString(),
    placeholder: "rgb(255, 0, 0)",
  },
  {
    label: "HSL",
    format: (c) => c.toHslString(),
    placeholder: "hsl(0, 100%, 50%)",
  },
  {
    label: "HWB",
    format: (c) => c.toHwbString(),
    placeholder: "hwb(0, 0%, 0%)",
  },
  {
    label: "LCH",
    format: (c) => c.toLchString(),
    placeholder: "lch(53.24, 104.55, 40.85)",
  },
  {
    label: "CMYK",
    format: (c) => c.toCmykString(),
    placeholder: "cmyk(0, 100%, 100%, 0)",
  },
  {
    label: "Name",
    format: (c) => c.toName({ closest: true }) ?? "Unknown",
    placeholder: "red",
  },
];

export function computeColorValues(
  c: ColordInstance,
  alpha: number,
  omitLabel?: string,
): Record<string, string> {
  const newValues: Record<string, string> = {};
  for (const f of FORMATS) {
    if (f.label !== omitLabel) {
      newValues[f.label] = f.format(c);
    }
  }
  if (omitLabel !== "RGBA") {
    const rgb = c.toRgb();
    newValues["RGBA"] = `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, ${alpha})`;
  }
  if (omitLabel !== "HEX") {
    newValues["HEX"] = c.toHex();
  }
  return newValues;
}

export { colord };
