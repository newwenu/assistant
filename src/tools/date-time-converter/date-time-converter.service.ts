import {
  formatISO,
  formatISO9075,
  formatRFC3339,
  formatRFC7231,
  fromUnixTime,
  getTime,
  getUnixTime,
  isValid,
  parseISO,
} from "date-fns";

export interface DateFormat {
  name: string;
  fromDate: (date: Date) => string;
  toDate: (value: string) => Date;
  match: (value: string) => boolean;
}

export function isUnixTimestamp(value: string): boolean {
  return /^\d{10}$/.test(value.trim());
}

export function isTimestamp(value: string): boolean {
  return /^\d{13}$/.test(value.trim());
}

export function isMongoObjectId(value: string): boolean {
  return /^[0-9a-fA-F]{24}$/.test(value.trim());
}

export function isExcelFormat(value: string): boolean {
  const n = Number(value);
  return !isNaN(n) && n > 0 && n < 2958466;
}

export function dateToExcel(date: Date): string {
  const epoch = new Date(1899, 11, 30);
  return String((date.getTime() - epoch.getTime()) / 86400000);
}

export function excelToDate(value: string): Date {
  const epoch = new Date(1899, 11, 30);
  return new Date(epoch.getTime() + Number(value) * 86400000);
}

export const FORMATS: DateFormat[] = [
  {
    name: "ISO 8601",
    fromDate: formatISO,
    toDate: parseISO,
    match: (v) => /^\d{4}-\d{2}-\d{2}T/.test(v),
  },
  {
    name: "ISO 9075",
    fromDate: formatISO9075,
    toDate: parseISO,
    match: (v) => /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/.test(v),
  },
  {
    name: "RFC 3339",
    fromDate: formatRFC3339,
    toDate: (v) => new Date(v),
    match: (v) => /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/.test(v),
  },
  {
    name: "RFC 7231",
    fromDate: formatRFC7231,
    toDate: (v) => new Date(v),
    match: (v) => /^[A-Z][a-z]{2}, \d{2}/.test(v),
  },
  {
    name: "Unix 时间戳 (秒)",
    fromDate: (d) => String(getUnixTime(d)),
    toDate: (v) => fromUnixTime(Number(v)),
    match: isUnixTimestamp,
  },
  {
    name: "时间戳 (毫秒)",
    fromDate: (d) => String(getTime(d)),
    toDate: (v) => new Date(Number(v)),
    match: isTimestamp,
  },
  {
    name: "UTC 格式",
    fromDate: (d) => d.toUTCString(),
    toDate: (v) => new Date(v),
    match: (v) => /^[A-Z][a-z]{2}, /.test(v),
  },
  {
    name: "Mongo ObjectID",
    fromDate: (d) =>
      `${Math.floor(d.getTime() / 1000).toString(16)}0000000000000000`,
    toDate: (v) => new Date(Number.parseInt(v.substring(0, 8), 16) * 1000),
    match: isMongoObjectId,
  },
  {
    name: "Excel 日期",
    fromDate: dateToExcel,
    toDate: excelToDate,
    match: isExcelFormat,
  },
];

export function formatDateSafe(format: DateFormat, date: Date | null): string {
  if (!date) return "";
  try {
    return format.fromDate(date);
  } catch {
    return "";
  }
}

export { isValid };
