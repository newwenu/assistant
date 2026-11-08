import { Button, Typography, Space, App, Tag } from "antd";
import { CloudUploadOutlined, DeleteOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { useConfGroup } from "@/settings/core/useConfGroup";
import type { SshKeyInfo } from "../core/vault.types";
import styles from "../vault.module.css";

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
  const navigate = useNavigate();
  const sshAutoAddEnabled = useConfGroup("vault")?.vaultSshAutoAdd ?? false;

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

      <Typography.Paragraph type="secondary">
        {t("vault.ssh.auto_add_on_unlock")}
        <Tag
          style={{ marginInline: 8 }}
          color={sshAutoAddEnabled ? "green" : undefined}
        >
          {sshAutoAddEnabled ? t("vault.ssh.on") : t("vault.ssh.off")}
        </Tag>
        <Button
          type="link"
          size="small"
          style={{ paddingInline: 0 }}
          onClick={() => {
            void navigate("/settings/vault");
          }}
        >
          {t("vault.ssh.auto_add_in_settings")}
        </Button>
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

      <div className={styles.borderedList}>
        {sshAgentKeys.map((key) => (
          <div key={key.fingerprint} className={styles.borderedListItem}>
            <div className={styles.listItemMeta}>
              <div className={styles.listItemMetaTitle}>
                {`${key.keyType} ${key.fingerprint}`}
              </div>
              <div className={styles.listItemMetaDescription}>
                {`${key.comment} (${key.bits} bits)`}
              </div>
            </div>
            {key.addedByVault && (
              <Button
                type="text"
                danger
                size="small"
                icon={<DeleteOutlined />}
                onClick={() => void sshRemoveFromAgent(key.fingerprint)}
              />
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
