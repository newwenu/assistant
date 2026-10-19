import { useState } from "react";
import { Input, Button, Typography, message } from "antd";
import { CopyOutlined, SwapOutlined } from "@ant-design/icons";
import styles from "./url-encoder.module.css";

const { TextArea } = Input;

export default function UrlEncoder() {
  const [input, setInput] = useState("");
  const [output, setOutput] = useState("");
  const [mode, setMode] = useState<"encode" | "decode">("encode");

  const convert = () => {
    try {
      if (mode === "encode") {
        setOutput(encodeURIComponent(input));
      } else {
        setOutput(decodeURIComponent(input));
      }
    } catch {
      message.error(mode === "encode" ? "编码失败" : "解码失败");
      setOutput("");
    }
  };

  const swap = () => {
    setInput(output);
    setOutput("");
    setMode(mode === "encode" ? "decode" : "encode");
  };

  const copy = () => {
    void navigator.clipboard.writeText(output);
    message.success("已复制");
  };

  const parseUrl = () => {
    try {
      const url = new URL(input);
      const parts = [
        `协议: ${url.protocol}`,
        `主机: ${url.hostname}`,
        url.port ? `端口: ${url.port}` : "",
        `路径: ${url.pathname}`,
        url.search ? `查询: ${url.search}` : "",
        url.hash ? `片段: ${url.hash}` : "",
      ]
        .filter(Boolean)
        .join("\n");
      setOutput(parts);
    } catch {
      message.error("无效 URL");
    }
  };

  return (
    <div>
      <div className={styles.headerRow}>
        <Typography.Text strong>
          {mode === "encode" ? "URL 编码" : "URL 解码"}
        </Typography.Text>
        <Button icon={<SwapOutlined />} size="small" onClick={swap}>
          切换
        </Button>
      </div>
      <TextArea
        value={input}
        onChange={(e) => setInput(e.target.value)}
        placeholder={
          mode === "encode" ? "输入要编码的文本..." : "输入要解码的 URL..."
        }
        rows={3}
        style={{ marginBottom: 8 }}
      />
      <div className={styles.buttonRow}>
        <Button type="primary" onClick={convert}>
          {mode === "encode" ? "编码" : "解码"}
        </Button>
        <Button onClick={parseUrl}>解析 URL</Button>
      </div>
      <div className={styles.outputWrapper}>
        <TextArea
          value={output}
          readOnly
          rows={3}
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
