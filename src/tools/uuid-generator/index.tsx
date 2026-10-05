import { useState } from "react";
import { Input, InputNumber, Button, Typography, Select, message } from "antd";
import { CopyOutlined, ReloadOutlined } from "@ant-design/icons";

const { TextArea } = Input;

function generateUUIDv4(): string {
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === "x" ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

function generateUUIDv1(): string {
  const now = Date.now();
  const hex = now.toString(16).padStart(12, "0");
  const rand = () => Math.floor(Math.random() * 16).toString(16);
  const timeLow = hex.slice(-8);
  const timeMid = hex.slice(-12, -8);
  const timeHi = "1" + hex.slice(-15, -12).padStart(3, "0");
  const clockSeq = ((Math.random() * 0x3fff) | 0).toString(16).padStart(4, "0");
  const node = Array.from({ length: 12 }, () => rand()).join("");
  return `${timeLow}-${timeMid}-${timeHi}-${clockSeq}-${node}`;
}

const GENERATORS: Record<string, () => string> = {
  v1: generateUUIDv1,
  v4: generateUUIDv4,
};

export default function UUIDGenerator() {
  const [version, setVersion] = useState("v4");
  const [count, setCount] = useState(1);
  const [uuids, setUuids] = useState<string[]>(() => [generateUUIDv4()]);

  const regenerate = () => {
    const gen = GENERATORS[version] ?? generateUUIDv4;
    setUuids(Array.from({ length: count }, () => gen()));
  };

  const text = uuids.join("\n");

  const copy = () => {
    navigator.clipboard.writeText(text);
    message.success("已复制");
  };

  return (
    <div>
      <div
        style={{
          display: "flex",
          gap: 12,
          marginBottom: 16,
          alignItems: "center",
        }}
      >
        <div>
          <Typography.Text strong style={{ marginRight: 8 }}>
            版本
          </Typography.Text>
          <Select
            value={version}
            onChange={(v) => {
              setVersion(v);
              regenerate();
            }}
            options={[
              { value: "v1", label: "v1" },
              { value: "v4", label: "v4" },
            ]}
            style={{ width: 80 }}
          />
        </div>
        <div>
          <Typography.Text strong style={{ marginRight: 8 }}>
            数量
          </Typography.Text>
          <InputNumber
            min={1}
            max={50}
            value={count}
            onChange={(v) => v && setCount(v)}
            style={{ width: 80 }}
          />
        </div>
        <Button icon={<ReloadOutlined />} onClick={regenerate}>
          重新生成
        </Button>
        <Button icon={<CopyOutlined />} onClick={copy}>
          复制
        </Button>
      </div>
      <TextArea
        value={text}
        readOnly
        rows={Math.min(count, 10)}
        style={{ fontFamily: "monospace" }}
      />
    </div>
  );
}
