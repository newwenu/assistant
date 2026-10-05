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

const { TextArea } = Input;

export default function JsonViewer() {
  const [rawJson, setRawJson] = useState('{"hello": "world", "foo": "bar"}');
  const [indentSize, setIndentSize] = useState(2);
  const [sortKeys, setSortKeys] = useState(true);

  const formatted = useMemo(() => {
    try {
      const obj = JSON.parse(rawJson);
      const sortObj = (o: any): any => {
        if (Array.isArray(o)) return o.map(sortObj);
        if (o && typeof o === "object") {
          const sorted: any = {};
          const keys = Object.keys(o);
          if (sortKeys) keys.sort();
          for (const k of keys) sorted[k] = sortObj(o[k]);
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
      return JSON.stringify(JSON.parse(rawJson));
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
      <Space style={{ marginBottom: 12 }}>
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
      <Typography.Text strong style={{ display: "block", marginBottom: 4 }}>
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
        <Typography.Text
          type="danger"
          style={{ display: "block", marginBottom: 8 }}
        >
          JSON 格式无效
        </Typography.Text>
      )}
      {isValid && (
        <>
          <div style={{ display: "flex", gap: 8, marginBottom: 8 }}>
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
          <Typography.Text strong style={{ display: "block", marginBottom: 4 }}>
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
