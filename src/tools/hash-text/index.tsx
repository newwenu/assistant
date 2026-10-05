import { useState, useEffect } from "react";
import { Input, Typography, Button, message } from "antd";
import { CopyOutlined } from "@ant-design/icons";

const { TextArea } = Input;

const ALGORITHMS = ["SHA-1", "SHA-256", "SHA-512"] as const;

async function computeHash(text: string, algo: string): Promise<string> {
  const encoder = new TextEncoder();
  const data = encoder.encode(text);
  const hashBuffer = await crypto.subtle.digest(algo, data);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map((b) => b.toString(16).padStart(2, "0")).join("");
}

export default function HashText() {
  const [text, setText] = useState("");
  const [results, setResults] = useState<Record<string, string>>({});

  useEffect(() => {
    if (!text) {
      setResults({});
      return;
    }
    const computeAll = async () => {
      const r: Record<string, string> = {};
      for (const algo of ALGORITHMS) {
        r[algo] = await computeHash(text, algo);
      }
      setResults(r);
    };
    computeAll();
  }, [text]);

  const copy = (value: string) => {
    navigator.clipboard.writeText(value);
    message.success("已复制");
  };

  return (
    <div>
      <TextArea
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder="输入要哈希的文本..."
        rows={3}
        style={{ marginBottom: 16 }}
      />
      {ALGORITHMS.map((algo) => (
        <div
          key={algo}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            marginBottom: 8,
          }}
        >
          <Typography.Text strong style={{ flex: "0 0 80px", fontSize: 13 }}>
            {algo}
          </Typography.Text>
          <Input
            value={results[algo] ?? ""}
            readOnly
            size="small"
            style={{ flex: 1, fontFamily: "monospace" }}
          />
          <Button
            icon={<CopyOutlined />}
            size="small"
            onClick={() => results[algo] && copy(results[algo])}
          />
        </div>
      ))}
    </div>
  );
}
