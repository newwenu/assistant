import { useState } from "react";
import { Input, Button, Typography, message } from "antd";
import { CopyOutlined, SwapOutlined } from "@ant-design/icons";

const { TextArea } = Input;

export default function Base64StringConverter() {
  const [input, setInput] = useState("");
  const [output, setOutput] = useState("");
  const [mode, setMode] = useState<"encode" | "decode">("encode");

  const convert = () => {
    try {
      if (mode === "encode") {
        setOutput(btoa(unescape(encodeURIComponent(input))));
      } else {
        setOutput(decodeURIComponent(escape(atob(input))));
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
      <div
        style={{
          display: "flex",
          gap: 8,
          marginBottom: 12,
          alignItems: "center",
        }}
      >
        <Typography.Text strong>
          {mode === "encode" ? "字符串 → Base64" : "Base64 → 字符串"}
        </Typography.Text>
        <Button icon={<SwapOutlined />} size="small" onClick={swap}>
          切换
        </Button>
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
      <div style={{ position: "relative" }}>
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
            style={{ position: "absolute", top: 4, right: 4 }}
          />
        )}
      </div>
    </div>
  );
}
