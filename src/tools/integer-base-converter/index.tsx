import { useState } from "react";
import { Input, Typography, Button, message } from "antd";
import { CopyOutlined } from "@ant-design/icons";

const BASES = [
  { label: "二进制 (2)", base: 2, prefix: "0b" },
  { label: "八进制 (8)", base: 8, prefix: "0o" },
  { label: "十进制 (10)", base: 10, prefix: "" },
  { label: "十六进制 (16)", base: 16, prefix: "0x" },
];

export default function IntegerBaseConverter() {
  const [inputValue, setInputValue] = useState("");
  const [inputBase] = useState(10);
  const [results, setResults] = useState<Record<number, string>>({});

  const convert = () => {
    try {
      const decimal = parseInt(inputValue, inputBase);
      if (isNaN(decimal)) {
        message.error("输入值无效");
        setResults({});
        return;
      }
      const r: Record<number, string> = {};
      for (const b of BASES) {
        r[b.base] = decimal.toString(b.base).toUpperCase();
      }
      setResults(r);
    } catch {
      message.error("转换失败");
      setResults({});
    }
  };

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
          输入
        </Typography.Text>
        <Input
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          placeholder="输入数值..."
          style={{ flex: 1, fontFamily: "monospace" }}
        />
        <Button type="primary" onClick={convert}>
          转换
        </Button>
      </div>
      {BASES.map((b) => (
        <div
          key={b.base}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            marginBottom: 8,
          }}
        >
          <Typography.Text strong style={{ flex: "0 0 120px", fontSize: 13 }}>
            {b.label}
          </Typography.Text>
          <Input
            value={results[b.base] ?? ""}
            readOnly
            size="small"
            style={{ flex: 1, fontFamily: "monospace" }}
          />
          <Button
            icon={<CopyOutlined />}
            size="small"
            onClick={() => results[b.base] && copy(results[b.base])}
          />
        </div>
      ))}
    </div>
  );
}
