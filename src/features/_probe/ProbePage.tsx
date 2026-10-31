import { useState } from "react";
import { Button, Card, Input, Space, Typography } from "antd";
import { useNavigate } from "react-router-dom";
import { probeEcho } from "./ipc";

const DEBUG_ROUTES = [
  "/probe",
  "/settings/probe",
  "/settings/vault",
  "/settings/general",
];

export const ProbePage: React.FC = () => {
  const navigate = useNavigate();
  const [echo, setEcho] = useState("");
  const [pending, setPending] = useState(false);
  const [route, setRoute] = useState("/settings/vault");

  const handleEcho = async () => {
    setPending(true);
    try {
      setEcho(await probeEcho("ping"));
    } finally {
      setPending(false);
    }
  };

  return (
    <Space orientation="vertical" size="middle" style={{ display: "flex" }}>
      <Card>
        <Typography.Title level={4}>探针页面</Typography.Title>
        <Space orientation="vertical">
          <Button
            type="primary"
            loading={pending}
            onClick={() => void handleEcho()}
          >
            调用 probe_echo
          </Button>
          <Typography.Text type="secondary">
            返回：{echo || "（未调用）"}
          </Typography.Text>
        </Space>
      </Card>
      <Card>
        <Typography.Title level={4}>调试导航</Typography.Title>
        <Space orientation="vertical" style={{ display: "flex" }}>
          <Space.Compact style={{ width: "100%" }}>
            <Input
              value={route}
              onChange={(e) => setRoute(e.target.value)}
              placeholder="/settings/vault"
            />
            <Button onClick={() => void navigate(route)}>跳转</Button>
          </Space.Compact>
          <Space wrap>
            {DEBUG_ROUTES.map((item) => (
              <Button
                key={item}
                size="small"
                onClick={() => void navigate(item)}
              >
                {item}
              </Button>
            ))}
          </Space>
        </Space>
      </Card>
    </Space>
  );
};
