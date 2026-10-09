import { useState, useMemo } from "react";
import { Input, Typography, Button, Select, message } from "antd";
import { CopyOutlined } from "@ant-design/icons";
import styles from "./date-time-converter.module.css";
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

interface DateFormat {
  name: string;
  fromDate: (date: Date) => string;
  toDate: (value: string) => Date;
  match: (value: string) => boolean;
}

function isUnixTimestamp(value: string): boolean {
  return /^\d{10}$/.test(value.trim());
}

function isTimestamp(value: string): boolean {
  return /^\d{13}$/.test(value.trim());
}

function isMongoObjectId(value: string): boolean {
  return /^[0-9a-fA-F]{24}$/.test(value.trim());
}

function isExcelFormat(value: string): boolean {
  const n = Number(value);
  return !isNaN(n) && n > 0 && n < 2958466;
}

function dateToExcel(date: Date): string {
  const epoch = new Date(1899, 11, 30);
  return String((date.getTime() - epoch.getTime()) / 86400000);
}

function excelToDate(value: string): Date {
  const epoch = new Date(1899, 11, 30);
  return new Date(epoch.getTime() + Number(value) * 86400000);
}

const FORMATS: DateFormat[] = [
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

export default function DateTimeConverter() {
  const [inputDate, setInputDate] = useState("");
  const [formatIndex, setFormatIndex] = useState(4);

  const normalizedDate = useMemo(() => {
    if (!inputDate) return null;
    try {
      const date = FORMATS[formatIndex].toDate(inputDate);
      return isValid(date) ? date : null;
    } catch {
      return null;
    }
  }, [inputDate, formatIndex]);

  const isValidInput = !inputDate || normalizedDate !== null;

  const handleInputChange = (value: string) => {
    setInputDate(value);
    const matchingIndex = FORMATS.findIndex((f) => f.match(value));
    if (matchingIndex !== -1) {
      setFormatIndex(matchingIndex);
    }
  };

  const setNow = () => {
    const now = new Date();
    setInputDate(FORMATS[formatIndex].fromDate(now));
  };

  const copy = (value: string) => {
    navigator.clipboard.writeText(value);
    message.success("已复制");
  };

  const formatDate = (format: DateFormat, date: Date | null) => {
    if (!date) return "";
    try {
      return format.fromDate(date);
    } catch {
      return "";
    }
  };

  return (
    <div>
      <div className={styles.inputRow}>
        <Input
          value={inputDate}
          onChange={(e) => handleInputChange(e.target.value)}
          placeholder="输入日期字符串..."
          status={isValidInput ? undefined : "error"}
          style={{ flex: 1, fontFamily: "monospace" }}
        />
        <Select
          value={formatIndex}
          onChange={setFormatIndex}
          options={FORMATS.map((f, i) => ({ label: f.name, value: i }))}
          style={{ width: 180 }}
        />
        <Button size="small" onClick={setNow}>
          当前
        </Button>
      </div>
      {!isValidInput && (
        <Typography.Text type="danger" className={styles.errorText}>
          无效的日期格式
        </Typography.Text>
      )}
      {FORMATS.map((format, idx) => {
        const value = formatDate(format, normalizedDate);
        return (
          <div key={format.name} className={styles.resultRow}>
            <Typography.Text strong className={styles.resultLabel}>
              {format.name}
            </Typography.Text>
            <Input
              value={value}
              readOnly
              size="small"
              placeholder={normalizedDate ? "" : "无效日期..."}
              style={{ flex: 1, fontFamily: "monospace" }}
              status={
                idx === formatIndex && !isValidInput ? "error" : undefined
              }
            />
            <Button
              icon={<CopyOutlined />}
              size="small"
              disabled={!value}
              onClick={() => copy(value)}
            />
          </div>
        );
      })}
    </div>
  );
}
