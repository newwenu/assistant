import { useState, useEffect } from "react";
import { Input, Typography, Button, Select, message } from "antd";
import { CopyOutlined } from "@ant-design/icons";
import {
  MD5,
  SHA1,
  SHA256,
  SHA224,
  SHA512,
  SHA384,
  SHA3,
  RIPEMD160,
  enc,
  type lib,
} from "crypto-js";
import styles from "./hash-text.module.css";

const { TextArea } = Input;

const ALGORITHMS = {
  MD5,
  SHA1,
  SHA256,
  SHA224,
  SHA512,
  SHA384,
  SHA3,
  RIPEMD160,
} as const;

type AlgoName = keyof typeof ALGORITHMS;
type Encoding = "Hex" | "Base64" | "Base64url" | "Bin";
const ALGO_NAMES = Object.keys(ALGORITHMS) as AlgoName[];

const ENCODING_OPTIONS: { value: Encoding; label: string }[] = [
  { value: "Hex", label: "十六进制 (Hex)" },
  { value: "Base64", label: "Base64" },
  { value: "Base64url", label: "Base64url" },
  { value: "Bin", label: "二进制 (Binary)" },
];

function hexToBin(hex: string): string {
  return hex
    .trim()
    .split("")
    .map((byte) => Number.parseInt(byte, 16).toString(2).padStart(4, "0"))
    .join("");
}

function formatOutput(words: lib.WordArray, encoding: Encoding): string {
  if (encoding === "Bin") {
    return hexToBin(words.toString(enc.Hex));
  }
  return words.toString(enc[encoding]);
}

export default function HashText() {
  const [text, setText] = useState("");
  const [encoding, setEncoding] = useState<Encoding>("Hex");
  const [results, setResults] = useState<Record<string, string>>({});

  useEffect(() => {
    if (!text) {
      setResults({});
      return;
    }
    const r: Record<string, string> = {};
    for (const algo of ALGO_NAMES) {
      r[algo] = formatOutput(ALGORITHMS[algo](text), encoding);
    }
    setResults(r);
  }, [text, encoding]);

  const copy = (value: string) => {
    void navigator.clipboard.writeText(value);
    message.success("已复制");
  };

  return (
    <div>
      <TextArea
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder="输入要哈希的文本..."
        rows={3}
        style={{ marginBottom: 12 }}
      />
      <div className={styles.encodingRow}>
        <Typography.Text strong className={styles.encodingLabel}>
          输出编码
        </Typography.Text>
        <Select
          value={encoding}
          onChange={setEncoding}
          options={ENCODING_OPTIONS}
          style={{ width: 180 }}
        />
      </div>
      {ALGO_NAMES.map((algo) => (
        <div key={algo} className={styles.resultRow}>
          <Typography.Text strong className={styles.algoLabel}>
            {algo}
          </Typography.Text>
          <Input
            value={results[algo] ?? ""}
            readOnly
            size="small"
            className={styles.algoInput}
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
