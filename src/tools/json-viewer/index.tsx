import { useState, useMemo } from "react";
import {
  Input,
  Button,
  Switch,
  InputNumber,
  Typography,
  message,
  Space,
} from "antd";
import { CopyOutlined, CompressOutlined } from "@ant-design/icons";
import JSON5 from "json5";
import styles from "./json-viewer.module.css";

const { TextArea } = Input;

export default function JsonViewer() {
  const [rawJson, setRawJson] = useState('{"hello": "world", "foo": "bar"}');
  const [indentSize, setIndentSize] = useState(2);
  const [sortKeys, setSortKeys] = useState(true);

  const formatted = useMemo(() => {
    try {
      const obj = JSON5.parse(rawJson);
      const sortObj = (o: unknown): unknown => {
        if (Array.isArray(o)) return o.map(sortObj);
        if (o && typeof o === "object") {
          const sorted: Record<string, unknown> = {};
          const keys = Object.keys(o as Record<string, unknown>);
          if (sortKeys) keys.sort();
          for (const k of keys)
            sorted[k] = sortObj((o as Record<string, unknown>)[k]);
          return sorted;
        }
        return o;
      };
      return JSON.stringify(sortObj(obj), null, indentSize);
    } catch {
      return null;
    }
  }, [rawJson, indentSize, sortKeys]);

  const minified = useMemo(() => {
    try {
      return JSON.stringify(JSON5.parse(rawJson));
    } catch {
      return null;
    }
  }, [rawJson]);

  const isValid = formatted !== null;

  const copy = (value: string) => {
    navigator.clipboard.writeText(value);
    message.success("已复制");
  };

  return (
    <div>
      <Space className={styles.optionsBar}>
        <Typography.Text>排序键:</Typography.Text>
        <Switch checked={sortKeys} onChange={setSortKeys} size="small" />
        <Typography.Text>缩进:</Typography.Text>
        <InputNumber
          min={0}
          max={10}
          value={indentSize}
          onChange={(v) => v !== null && setIndentSize(v)}
          size="small"
          style={{ width: 60 }}
        />
      </Space>
      <Typography.Text strong className={styles.sectionLabel}>
        输入 JSON
      </Typography.Text>
      <TextArea
        value={rawJson}
        onChange={(e) => setRawJson(e.target.value)}
        rows={8}
        style={{ fontFamily: "monospace", marginBottom: 8 }}
        status={isValid ? undefined : "error"}
      />
      {!isValid && (
        <Typography.Text type="danger" className={styles.errorText}>
          JSON 格式无效（支持 JSON5 语法）
        </Typography.Text>
      )}
      {isValid && (
        <>
          <div className={styles.buttonRow}>
            <Button
              icon={<CopyOutlined />}
              onClick={() => formatted && copy(formatted)}
            >
              复制格式化
            </Button>
            <Button
              icon={<CompressOutlined />}
              onClick={() => minified && copy(minified)}
            >
              复制压缩
            </Button>
          </div>
          <Typography.Text strong className={styles.sectionLabel}>
            格式化结果
          </Typography.Text>
          <TextArea
            value={formatted ?? ""}
            readOnly
            rows={10}
            style={{ fontFamily: "monospace" }}
          />
        </>
      )}
    </div>
  );
}
