import { useState } from "react";
import { Input, Button, Switch, Typography, message } from "antd";
import { CopyOutlined, SwapOutlined } from "@ant-design/icons";
import styles from "./base64-string-converter.module.css";

const { TextArea } = Input;

function textToBase64(text: string, urlSafe: boolean): string {
  const encoded = btoa(unescape(encodeURIComponent(text)));
  if (urlSafe) {
    return encoded.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  }
  return encoded;
}

function base64ToText(base64: string, urlSafe: boolean): string {
  let b64 = base64;
  if (urlSafe) {
    b64 = b64.replace(/-/g, "+").replace(/_/g, "/");
    const pad = (4 - (b64.length % 4)) % 4;
    b64 += "=".repeat(pad);
  }
  return decodeURIComponent(escape(atob(b64)));
}

function isValidBase64(base64: string, urlSafe: boolean): boolean {
  try {
    base64ToText(base64, urlSafe);
    return true;
  } catch {
    return false;
  }
}

export default function Base64StringConverter() {
  const [input, setInput] = useState("");
  const [output, setOutput] = useState("");
  const [mode, setMode] = useState<"encode" | "decode">("encode");
  const [urlSafe, setUrlSafe] = useState(false);

  const convert = () => {
    try {
      if (mode === "encode") {
        setOutput(textToBase64(input, urlSafe));
      } else {
        if (!isValidBase64(input, urlSafe)) {
          message.error("输入不是有效的 Base64 字符串");
          setOutput("");
          return;
        }
        setOutput(base64ToText(input, urlSafe));
      }
    } catch {
      message.error(
        mode === "encode" ? "编码失败" : "解码失败，输入不是有效的 Base64",
      );
      setOutput("");
    }
  };

  const copy = () => {
    navigator.clipboard.writeText(output);
    message.success("已复制");
  };

  const swap = () => {
    setInput(output);
    setOutput("");
    setMode(mode === "encode" ? "decode" : "encode");
  };

  return (
    <div>
      <div className={styles.headerRow}>
        <Typography.Text strong>
          {mode === "encode" ? "字符串 → Base64" : "Base64 → 字符串"}
        </Typography.Text>
        <div className={styles.headerActions}>
          <Switch
            checked={urlSafe}
            onChange={setUrlSafe}
            checkedChildren="URL-safe"
            unCheckedChildren="标准"
            size="small"
          />
          <Button icon={<SwapOutlined />} size="small" onClick={swap}>
            切换
          </Button>
        </div>
      </div>
      <TextArea
        value={input}
        onChange={(e) => setInput(e.target.value)}
        placeholder={mode === "encode" ? "输入字符串..." : "输入 Base64..."}
        rows={4}
        style={{ marginBottom: 8 }}
      />
      <Button type="primary" onClick={convert} style={{ marginBottom: 8 }}>
        {mode === "encode" ? "编码" : "解码"}
      </Button>
      <div className={styles.outputWrapper}>
        <TextArea
          value={output}
          readOnly
          rows={4}
          style={{ fontFamily: "monospace" }}
        />
        {output && (
          <Button
            icon={<CopyOutlined />}
            size="small"
            onClick={copy}
            className={styles.copyButton}
          />
        )}
      </div>
    </div>
  );
}
