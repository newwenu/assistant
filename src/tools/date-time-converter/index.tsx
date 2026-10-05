import { useState, useEffect } from "react";
import { Input, Typography, Button, message, Divider } from "antd";
import { CopyOutlined } from "@ant-design/icons";

export default function DateTimeConverter() {
  const [timestamp, setTimestamp] = useState<string>(
    String(Math.floor(Date.now() / 1000)),
  );
  const [readable, setReadable] = useState("");
  const [dateInput, setDateInput] = useState("");

  useEffect(() => {
    const ts = Number(timestamp);
    if (isNaN(ts)) {
      setReadable("无效时间戳");
      return;
    }
    try {
      const ms = ts > 1e12 ? ts : ts * 1000;
      setReadable(new Date(ms).toLocaleString());
    } catch {
      setReadable("无效时间戳");
    }
  }, [timestamp]);

  const fromDateString = () => {
    try {
      const d = new Date(dateInput);
      if (isNaN(d.getTime())) {
        message.error("无效日期");
        return;
      }
      setTimestamp(String(Math.floor(d.getTime() / 1000)));
    } catch {
      message.error("无效日期");
    }
  };

  const now = () => setTimestamp(String(Math.floor(Date.now() / 1000)));

  const copy = (value: string) => {
    navigator.clipboard.writeText(value);
    message.success("已复制");
  };

  return (
    <div>
      <div
        style={{
          display: "flex",
          gap: 8,
          marginBottom: 12,
          alignItems: "center",
        }}
      >
        <Typography.Text strong style={{ flex: "0 0 80px" }}>
          Unix 时间戳
        </Typography.Text>
        <Input
          value={timestamp}
          onChange={(e) => setTimestamp(e.target.value)}
          style={{ flex: 1, fontFamily: "monospace" }}
        />
        <Button size="small" onClick={now}>
          当前
        </Button>
        <Button
          icon={<CopyOutlined />}
          size="small"
          onClick={() => copy(timestamp)}
        />
      </div>
      <div
        style={{
          display: "flex",
          gap: 8,
          marginBottom: 12,
          alignItems: "center",
        }}
      >
        <Typography.Text strong style={{ flex: "0 0 80px" }}>
          可读日期
        </Typography.Text>
        <Input value={readable} readOnly style={{ flex: 1 }} />
        <Button
          icon={<CopyOutlined />}
          size="small"
          onClick={() => copy(readable)}
        />
      </div>
      <Divider />
      <Typography.Text strong style={{ display: "block", marginBottom: 8 }}>
        日期 → 时间戳
      </Typography.Text>
      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
        <Input
          value={dateInput}
          onChange={(e) => setDateInput(e.target.value)}
          placeholder="2025-01-01 或 2025-01-01T12:00:00"
          style={{ flex: 1 }}
        />
        <Button type="primary" onClick={fromDateString}>
          转换
        </Button>
      </div>
    </div>
  );
}
