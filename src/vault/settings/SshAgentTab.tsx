import { Button, Typography, Space, List, App } from "antd";
import { CloudUploadOutlined, DeleteOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { SshKeyInfo } from "../core/vault.types";

interface SshAgentTabProps {
  sshAgentKeys: SshKeyInfo[];
  sshAgentRunning: boolean;
  sshListAgent: () => Promise<void>;
  sshCheckAgentRunning: () => Promise<void>;
  sshRemoveFromAgent: (fingerprint: string) => Promise<void>;
  sshAutoAddAll: () => Promise<string[]>;
}

export default function SshAgentTab({
  sshAgentKeys,
  sshAgentRunning,
  sshListAgent,
  sshCheckAgentRunning,
  sshRemoveFromAgent,
  sshAutoAddAll,
}: SshAgentTabProps) {
  const { t } = useTranslation();
  const { message } = App.useApp();

  const handleAutoAddAll = async () => {
    const added = await sshAutoAddAll();
    if (added.length > 0) {
      message.success(t("vault.ssh.auto_added", { count: added.length }));
    } else {
      message.info(t("vault.ssh.no_keys_to_add"));
    }
  };

  return (
    <div>
      <Typography.Paragraph type="secondary">
        {t("vault.ssh.agent_description")}
      </Typography.Paragraph>

      <Space style={{ marginBottom: 8 }}>
        <Button
          icon={<CloudUploadOutlined />}
          onClick={() => void handleAutoAddAll()}
        >
          {t("vault.ssh.add_all_to_agent")}
        </Button>
        <Button
          onClick={() => {
            void sshListAgent();
            void sshCheckAgentRunning();
          }}
        >
          {t("vault.ssh.refresh_agent_keys")}
        </Button>
      </Space>

      <Typography.Text
        type="secondary"
        style={{ display: "block", marginBottom: 8 }}
      >
        {sshAgentRunning
          ? t("vault.ssh.agent_running")
          : t("vault.ssh.agent_not_running")}
      </Typography.Text>

      <List
        size="small"
        bordered
        dataSource={sshAgentKeys}
        renderItem={(key) => (
          <List.Item
            actions={[
              key.addedByVault && (
                <Button
                  key="remove"
                  type="text"
                  danger
                  size="small"
                  icon={<DeleteOutlined />}
                  onClick={() => void sshRemoveFromAgent(key.fingerprint)}
                />
              ),
            ].filter(Boolean)}
          >
            <List.Item.Meta
              title={`${key.keyType} ${key.fingerprint}`}
              description={`${key.comment} (${key.bits} bits)`}
            />
          </List.Item>
        )}
      />
    </div>
  );
}
