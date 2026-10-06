import { useEffect, useState } from "react";
import { Descriptions, Button, Typography } from "antd";
import { invoke } from "@tauri-apps/api/core";

export const AboutPage: React.FC = () => {
  const [version, setVersion] = useState("");
  const [rustcVersion, setRustcVersion] = useState("");
  const [buildDate] = useState(() => {
    try {
      return (
        (window as unknown as Record<string, string>).__BUILD_DATE__ ?? "-"
      );
    } catch {
      return "-";
    }
  });

  useEffect(() => {
    invoke<string>("get_app_version")
      .then((v) => setVersion(v))
      .catch(() => setVersion("-"));
    invoke<string>("get_rustc_version")
      .then((v) => setRustcVersion(v))
      .catch(() => setRustcVersion("-"));
  }, []);

  return (
    <div style={{ maxWidth: 600, margin: "0 auto", padding: 24 }}>
      <Typography.Title level={5} style={{ marginBottom: 16 }}>
        关于
      </Typography.Title>
      <Descriptions column={1} bordered size="small">
        <Descriptions.Item label="应用名称">Assistant</Descriptions.Item>
        <Descriptions.Item label="版本号">
          <Typography.Text copyable>{version || "-"}</Typography.Text>
        </Descriptions.Item>
        <Descriptions.Item label="构建日期">{buildDate}</Descriptions.Item>
        <Descriptions.Item label="Rust 工具链">
          <Typography.Text copyable>{rustcVersion || "-"}</Typography.Text>
        </Descriptions.Item>
        <Descriptions.Item label="前端技术栈">
          Tauri 2 + React 19 + Ant Design
        </Descriptions.Item>
        <Descriptions.Item label="许可证">MIT</Descriptions.Item>
      </Descriptions>
      <div style={{ marginTop: 16, textAlign: "center" }}>
        <Button type="default" disabled>
          检查更新（即将推出）
        </Button>
      </div>
    </div>
  );
};