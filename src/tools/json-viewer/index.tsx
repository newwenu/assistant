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
import { formatJson, minifyJson } from "./json-viewer.service";
import styles from "./json-viewer.module.css";

const { TextArea } = Input;

export default function JsonViewer() {
  const [rawJson, setRawJson] = useState('{"hello": "world", "foo": "bar"}');
  const [indentSize, setIndentSize] = useState(2);
  const [sortKeys, setSortKeys] = useState(true);

  const formatted = useMemo(
    () => formatJson(rawJson, indentSize, sortKeys),
    [rawJson, indentSize, sortKeys],
  );

  const minified = useMemo(() => minifyJson(rawJson), [rawJson]);

  const isValid = formatted !== null;

  const copy = (value: string) => {
    void navigator.clipboard.writeText(value);
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
