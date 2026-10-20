export interface UnitDef {
  name: string;
  toBase: (v: number) => number;
  fromBase: (v: number) => number;
}

export interface CategoryDef {
  name: string;
  units: Record<string, UnitDef>;
}

export const CATEGORIES: Record<string, CategoryDef> = {
  length: {
    name: "长度",
    units: {
      mm: {
        name: "毫米 (mm)",
        toBase: (v) => v / 1000,
        fromBase: (v) => v * 1000,
      },
      cm: {
        name: "厘米 (cm)",
        toBase: (v) => v / 100,
        fromBase: (v) => v * 100,
      },
      m: { name: "米 (m)", toBase: (v) => v, fromBase: (v) => v },
      km: {
        name: "千米 (km)",
        toBase: (v) => v * 1000,
        fromBase: (v) => v / 1000,
      },
      in: {
        name: "英寸 (in)",
        toBase: (v) => v * 0.0254,
        fromBase: (v) => v / 0.0254,
      },
      ft: {
        name: "英尺 (ft)",
        toBase: (v) => v * 0.3048,
        fromBase: (v) => v / 0.3048,
      },
      yd: {
        name: "码 (yd)",
        toBase: (v) => v * 0.9144,
        fromBase: (v) => v / 0.9144,
      },
      mi: {
        name: "英里 (mi)",
        toBase: (v) => v * 1609.344,
        fromBase: (v) => v / 1609.344,
      },
      nm: {
        name: "海里 (nmi)",
        toBase: (v) => v * 1852,
        fromBase: (v) => v / 1852,
      },
    },
  },
  weight: {
    name: "重量",
    units: {
      mg: {
        name: "毫克 (mg)",
        toBase: (v) => v / 1e6,
        fromBase: (v) => v * 1e6,
      },
      g: { name: "克 (g)", toBase: (v) => v / 1000, fromBase: (v) => v * 1000 },
      kg: { name: "千克 (kg)", toBase: (v) => v, fromBase: (v) => v },
      t: { name: "吨 (t)", toBase: (v) => v * 1000, fromBase: (v) => v / 1000 },
      oz: {
        name: "盎司 (oz)",
        toBase: (v) => v * 0.0283495,
        fromBase: (v) => v / 0.0283495,
      },
      lb: {
        name: "磅 (lb)",
        toBase: (v) => v * 0.453592,
        fromBase: (v) => v / 0.453592,
      },
      jin: { name: "斤", toBase: (v) => v * 0.5, fromBase: (v) => v / 0.5 },
    },
  },
  temperature: {
    name: "温度",
    units: {
      C: {
        name: "摄氏度 (°C)",
        toBase: (v) => v,
        fromBase: (v) => v,
      },
      F: {
        name: "华氏度 (°F)",
        toBase: (v) => ((v - 32) * 5) / 9,
        fromBase: (v) => (v * 9) / 5 + 32,
      },
      K: {
        name: "开尔文 (K)",
        toBase: (v) => v - 273.15,
        fromBase: (v) => v + 273.15,
      },
      R: {
        name: "兰氏度 (°R)",
        toBase: (v) => ((v - 491.67) * 5) / 9,
        fromBase: (v) => (v + 273.15) * (9 / 5),
      },
      De: {
        name: "德利尔度 (°De)",
        toBase: (v) => 100 - (v * 2) / 3,
        fromBase: (v) => ((100 - v) * 3) / 2,
      },
      N: {
        name: "牛顿度 (°N)",
        toBase: (v) => (v * 100) / 33,
        fromBase: (v) => (v * 33) / 100,
      },
      Re: {
        name: "列氏度 (°Ré)",
        toBase: (v) => (v * 5) / 4,
        fromBase: (v) => (v * 4) / 5,
      },
      Ro: {
        name: "罗默度 (°Rø)",
        toBase: (v) => ((v - 7.5) * 40) / 21,
        fromBase: (v) => (v * 21) / 40 + 7.5,
      },
    },
  },
  area: {
    name: "面积",
    units: {
      mm2: {
        name: "平方毫米",
        toBase: (v) => v / 1e6,
        fromBase: (v) => v * 1e6,
      },
      cm2: {
        name: "平方厘米",
        toBase: (v) => v / 1e4,
        fromBase: (v) => v * 1e4,
      },
      m2: { name: "平方米", toBase: (v) => v, fromBase: (v) => v },
      km2: {
        name: "平方千米",
        toBase: (v) => v * 1e6,
        fromBase: (v) => v / 1e6,
      },
      ha: { name: "公顷", toBase: (v) => v * 1e4, fromBase: (v) => v / 1e4 },
      acre: {
        name: "英亩",
        toBase: (v) => v * 4046.8564224,
        fromBase: (v) => v / 4046.8564224,
      },
      mu: {
        name: "亩",
        toBase: (v) => v * 666.667,
        fromBase: (v) => v / 666.667,
      },
    },
  },
  volume: {
    name: "体积",
    units: {
      ml: {
        name: "毫升 (mL)",
        toBase: (v) => v / 1e6,
        fromBase: (v) => v * 1e6,
      },
      L: { name: "升 (L)", toBase: (v) => v / 1000, fromBase: (v) => v * 1000 },
      m3: { name: "立方米 (m³)", toBase: (v) => v, fromBase: (v) => v },
      gal: {
        name: "美制加仑",
        toBase: (v) => v * 0.00378541,
        fromBase: (v) => v / 0.00378541,
      },
      qt: {
        name: "美制夸脱",
        toBase: (v) => v * 0.000946353,
        fromBase: (v) => v / 0.000946353,
      },
      cup: {
        name: "美制杯",
        toBase: (v) => v * 0.000236588,
        fromBase: (v) => v / 0.000236588,
      },
      floz: {
        name: "美制液量盎司",
        toBase: (v) => v * 2.95735e-5,
        fromBase: (v) => v / 2.95735e-5,
      },
    },
  },
  speed: {
    name: "速度",
    units: {
      mps: { name: "米/秒 (m/s)", toBase: (v) => v, fromBase: (v) => v },
      kmh: {
        name: "千米/时 (km/h)",
        toBase: (v) => v / 3.6,
        fromBase: (v) => v * 3.6,
      },
      mph: {
        name: "英里/时 (mph)",
        toBase: (v) => v * 0.44704,
        fromBase: (v) => v / 0.44704,
      },
      kn: {
        name: "节 (kn)",
        toBase: (v) => v * 0.514444,
        fromBase: (v) => v / 0.514444,
      },
      c: {
        name: "光速 (c)",
        toBase: (v) => v * 299792458,
        fromBase: (v) => v / 299792458,
      },
      mach: {
        name: "马赫 (Ma)",
        toBase: (v) => v * 340.29,
        fromBase: (v) => v / 340.29,
      },
    },
  },
  data: {
    name: "数据",
    units: {
      B: { name: "字节 (B)", toBase: (v) => v, fromBase: (v) => v },
      KB: { name: "KB", toBase: (v) => v * 1024, fromBase: (v) => v / 1024 },
      MB: {
        name: "MB",
        toBase: (v) => v * 1048576,
        fromBase: (v) => v / 1048576,
      },
      GB: {
        name: "GB",
        toBase: (v) => v * 1073741824,
        fromBase: (v) => v / 1073741824,
      },
      TB: {
        name: "TB",
        toBase: (v) => v * 1099511627776,
        fromBase: (v) => v / 1099511627776,
      },
      PB: {
        name: "PB",
        toBase: (v) => v * 1125899906842624,
        fromBase: (v) => v / 1125899906842624,
      },
      Kbit: { name: "Kbit", toBase: (v) => v * 128, fromBase: (v) => v / 128 },
      Mbit: {
        name: "Mbit",
        toBase: (v) => v * 131072,
        fromBase: (v) => v / 131072,
      },
      Gbit: {
        name: "Gbit",
        toBase: (v) => v * 134217728,
        fromBase: (v) => v / 134217728,
      },
    },
  },
};

export const CATEGORY_OPTIONS = Object.entries(CATEGORIES).map(
  ([key, cat]) => ({
    label: cat.name,
    value: key,
  }),
);

export function computeConversions(
  category: string,
  fromUnit: string,
  value: number,
): { key: string; name: string; value: number }[] {
  const cat = CATEGORIES[category];
  if (!cat) return [];
  const srcUnit = cat.units[fromUnit];
  if (!srcUnit) return [];
  const baseValue = srcUnit.toBase(value);
  return Object.entries(cat.units)
    .filter(([key]) => key !== fromUnit)
    .map(([key, u]) => ({
      key,
      name: u.name,
      value: u.fromBase(baseValue),
    }));
}
