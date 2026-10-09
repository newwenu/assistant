import { useState } from "react";
import { Input, InputNumber, Button, Typography, Select, message } from "antd";
import { CopyOutlined, ReloadOutlined } from "@ant-design/icons";
import { v1, v3, v4, v5, v7, NIL } from "uuid";
import styles from "./uuid-generator.module.css";

const { TextArea } = Input;

const NAMESPACES: Record<string, string> = {
  DNS: "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
  URL: "6ba7b811-9dad-11d1-80b4-00c04fd430c8",
  OID: "6ba7b812-9dad-11d1-80b4-00c04fd430c8",
  X500: "6ba7b814-9dad-11d1-80b4-00c04fd430c8",
};

const NAMESPACE_OPTIONS = Object.keys(NAMESPACES).map((k) => ({
  value: k,
  label: `${k} (${NAMESPACES[k].slice(0, 8)}...)`,
}));

const VERSION_OPTIONS = [
  { value: "NIL", label: "NIL (全零)" },
  { value: "v1", label: "v1 (时间)" },
  { value: "v3", label: "v3 (MD5 命名)" },
  { value: "v4", label: "v4 (随机)" },
  { value: "v5", label: "v5 (SHA1 命名)" },
  { value: "v7", label: "v7 (时间排序)" },
];

export default function UUIDGenerator() {
  const [version, setVersion] = useState("v4");
  const [count, setCount] = useState(1);
  const [uuids, setUuids] = useState<string[]>(() => [v4()]);
  const [namespace, setNamespace] = useState("URL");
  const [name, setName] = useState("example.com");

  const regenerate = () => {
    const results: string[] = [];
    for (let i = 0; i < count; i++) {
      switch (version) {
        case "NIL":
          results.push(NIL);
          break;
        case "v1":
          results.push(v1());
          break;
        case "v3":
          results.push(v3(name, NAMESPACES[namespace] ?? NAMESPACES.URL));
          break;
        case "v4":
          results.push(v4());
          break;
        case "v5":
          results.push(v5(name, NAMESPACES[namespace] ?? NAMESPACES.URL));
          break;
        case "v7":
          results.push(v7());
          break;
        default:
          results.push(v4());
      }
    }
    setUuids(results);
  };

  const text = uuids.join("\n");

  const copy = () => {
    navigator.clipboard.writeText(text);
    message.success("已复制");
  };

  const needsNameArgs = version === "v3" || version === "v5";

  return (
    <div>
      <div className={styles.toolbar}>
        <div className={styles.toolbarItem}>
          <Typography.Text strong>版本</Typography.Text>
          <Select
            value={version}
            onChange={(v) => {
              setVersion(v);
              regenerate();
            }}
            options={VERSION_OPTIONS}
            style={{ width: 160 }}
          />
        </div>
        <div className={styles.toolbarItem}>
          <Typography.Text strong>数量</Typography.Text>
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

      {needsNameArgs && (
        <div className={styles.v35Row}>
          <div className={styles.toolbarItem}>
            <Typography.Text strong>命名空间</Typography.Text>
            <Select
              value={namespace}
              onChange={setNamespace}
              options={NAMESPACE_OPTIONS}
              style={{ width: 200 }}
            />
          </div>
          <div className={styles.toolbarItem}>
            <Typography.Text strong>名称</Typography.Text>
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="输入名称..."
              style={{ width: 200 }}
            />
          </div>
        </div>
      )}

      <TextArea
        value={text}
        readOnly
        rows={Math.min(count, 10)}
        className={styles.resultArea}
      />
    </div>
  );
}
