import { Button, Typography, Space, App, Switch } from "antd";
import { CloudUploadOutlined, DeleteOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useConfGroup } from "@/settings/core/useConfGroup";
import { dispatchConfPatch } from "@/settings/core/settingsDispatcher";
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
  const sshAutoAddEnabled = useConfGroup("vault")?.vaultSshAutoAdd ?? false;

  const handleToggleAutoAdd = async (checked: boolean) => {
    try {
      await dispatchConfPatch("vault", { vaultSshAutoAdd: checked });
      message.success(t("common.actions.saved"));
    } catch {
      message.error(t("vault.entry.save_failed"));
    }
  };

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

      <Space style={{ marginBottom: 12 }}>
        <Switch
          checked={sshAutoAddEnabled}
          onChange={(checked) => void handleToggleAutoAdd(checked)}
        />
        <Typography.Text>{t("vault.ssh.auto_add_on_unlock")}</Typography.Text>
      </Space>

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
