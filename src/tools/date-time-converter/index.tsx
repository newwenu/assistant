import { useState, useMemo } from "react";
import { Input, Typography, Button, Select, message } from "antd";
import { CopyOutlined } from "@ant-design/icons";
import {
  FORMATS,
  formatDateSafe,
  isValid,
  type DateFormat,
} from "./date-time-converter.service";
import styles from "./date-time-converter.module.css";

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
    const matchingIndex = FORMATS.findIndex((f: DateFormat) => f.match(value));
    if (matchingIndex !== -1) {
      setFormatIndex(matchingIndex);
    }
  };

  const setNow = () => {
    const now = new Date();
    setInputDate(FORMATS[formatIndex].fromDate(now));
  };

  const copy = (value: string) => {
    void navigator.clipboard.writeText(value);
    message.success("已复制");
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
          options={FORMATS.map((f: DateFormat, i: number) => ({
            label: f.name,
            value: i,
          }))}
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
      {FORMATS.map((format: DateFormat, idx: number) => {
        const value = formatDateSafe(format, normalizedDate);
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
