import { useState, useEffect } from "react";
import { Input, Typography, Button, Select, App } from "antd";
import { CopyOutlined } from "@ant-design/icons";
import {
  ALGO_NAMES,
  ENCODING_OPTIONS,
  hashWithAlgo,
  type Encoding,
} from "./hash-text.service";
import styles from "./hash-text.module.css";

const { TextArea } = Input;

export default function HashText() {
  const { message } = App.useApp();
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
      r[algo] = hashWithAlgo(algo, text, encoding);
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
