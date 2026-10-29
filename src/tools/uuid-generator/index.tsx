import { useState } from "react";
import { Input, InputNumber, Button, Typography, Select, message } from "antd";
import { CopyOutlined, ReloadOutlined } from "@ant-design/icons";
import {
  NAMESPACE_OPTIONS,
  VERSION_OPTIONS,
  generateUuids,
} from "./uuid-generator.service";
import styles from "./uuid-generator.module.css";

const { TextArea } = Input;

export default function UUIDGenerator() {
  const [version, setVersion] = useState("v4");
  const [count, setCount] = useState(1);
  const [uuids, setUuids] = useState<string[]>(() =>
    generateUuids("v4", 1, "URL", ""),
  );
  const [namespace, setNamespace] = useState("URL");
  const [name, setName] = useState("example.com");

  const regenerate = () => {
    setUuids(generateUuids(version, count, namespace, name));
  };

  const text = uuids.join("\n");

  const copy = () => {
    void navigator.clipboard.writeText(text);
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
