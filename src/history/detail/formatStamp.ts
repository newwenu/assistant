function pad(value: number): string {
  return value < 10 ? `0${value}` : String(value);
}

/**
 * 历史页唯一时间格式化函数（I51 §3.3 / 门禁 G15）。
 *
 * 短模式 `HH:MM:SS`，完整模式 `YYYY-MM-DD HH:MM:SS`（本地时区、秒级）。
 * 历史页任何时间展示都必须走这里，禁止各组件自行调用 `Date` 的本地化格式化方法。
 */
export function formatStamp(timestamp: number, withDate = false): string {
  const date = new Date(timestamp);
  const time = `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
  if (!withDate) return time;
  const day = `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  return `${day} ${time}`;
}
