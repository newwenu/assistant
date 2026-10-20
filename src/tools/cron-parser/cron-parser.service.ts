export const HELPERS = [
  { symbol: "*", meaning: "任意值", example: "* * * *", equivalent: "每分钟" },
  {
    symbol: "-",
    meaning: "范围",
    example: "1-10 * * *",
    equivalent: "第 1 到 10 分钟",
  },
  {
    symbol: ",",
    meaning: "列表",
    example: "1,10 * * *",
    equivalent: "第 1 和第 10 分钟",
  },
  {
    symbol: "/",
    meaning: "步长",
    example: "*/10 * * *",
    equivalent: "每 10 分钟",
  },
  {
    symbol: "@yearly",
    meaning: "每年一次",
    example: "@yearly",
    equivalent: "0 0 1 1 *",
  },
  {
    symbol: "@annually",
    meaning: "同 @yearly",
    example: "@annually",
    equivalent: "0 0 1 1 *",
  },
  {
    symbol: "@monthly",
    meaning: "每月一次",
    example: "@monthly",
    equivalent: "0 0 1 * *",
  },
  {
    symbol: "@weekly",
    meaning: "每周一次",
    example: "@weekly",
    equivalent: "0 0 * * 0",
  },
  {
    symbol: "@daily",
    meaning: "每天一次",
    example: "@daily",
    equivalent: "0 0 * * *",
  },
  {
    symbol: "@midnight",
    meaning: "同 @daily",
    example: "@midnight",
    equivalent: "0 0 * * *",
  },
  {
    symbol: "@hourly",
    meaning: "每小时一次",
    example: "@hourly",
    equivalent: "0 * * * *",
  },
  {
    symbol: "@reboot",
    meaning: "启动时运行",
    example: "@reboot",
    equivalent: "—",
  },
];

export const COLUMNS = [
  { title: "符号", dataIndex: "symbol", key: "symbol", width: 100 },
  { title: "含义", dataIndex: "meaning", key: "meaning" },
  { title: "示例", dataIndex: "example", key: "example" },
  { title: "等价", dataIndex: "equivalent", key: "equivalent" },
];

function parseField(field: string, min: number, max: number): number[] {
  if (field === "*")
    return Array.from({ length: max - min + 1 }, (_, i) => min + i);
  const values = new Set<number>();
  for (const part of field.split(",")) {
    if (part.includes("/")) {
      const [range, step] = part.split("/");
      const s = Number(step);
      let start = min;
      let end = max;
      if (range !== "*") {
        [start, end] = range.split("-").map(Number);
      }
      for (let i = start; i <= end; i += s) values.add(i);
    } else if (part.includes("-")) {
      const [start, end] = part.split("-").map(Number);
      for (let i = start; i <= end; i++) values.add(i);
    } else {
      values.add(Number(part));
    }
  }
  return Array.from(values).sort((a, b) => a - b);
}

export function getNextExecutions(cronExpr: string, count: number): Date[] {
  const results: Date[] = [];
  try {
    const parts = cronExpr.trim().split(/\s+/);
    if (parts.length < 5) return results;

    const now = new Date();
    let current = new Date(now.getTime() + 60000);
    current.setSeconds(0, 0);

    const minuteField = parts[0];
    const hourField = parts[1];
    const domField = parts[2];
    const monthField = parts[3];

    const minutes = parseField(minuteField, 0, 59);
    const hours = parseField(hourField, 0, 23);
    const doms = parseField(domField, 1, 31);
    const months = parseField(monthField, 1, 12);

    const maxIter = 525600;
    for (let i = 0; i < maxIter && results.length < count; i++) {
      if (
        minutes.includes(current.getMinutes()) &&
        hours.includes(current.getHours()) &&
        doms.includes(current.getDate()) &&
        months.includes(current.getMonth() + 1)
      ) {
        results.push(new Date(current.getTime()));
      }
      current = new Date(current.getTime() + 60000);
    }
  } catch {
    // ignore
  }
  return results;
}
