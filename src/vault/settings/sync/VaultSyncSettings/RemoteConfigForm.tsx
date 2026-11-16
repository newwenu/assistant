import type { RefObject } from "react";
import { Button, Form, Input, Space } from "antd";
import type { InputRef } from "antd";
import { LinkOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";

interface RemoteConfigFormProps {
  remoteUrl: string;
  onRemoteUrlChange: (value: string) => void;
  remoteUrlRef: RefObject<InputRef | null>;
  testing: boolean;
  onTestConnection: () => void;
  sshHostsVisible: boolean;
  sshHosts: string;
  onSshHostsChange: (value: string) => void;
  isNoSync: boolean;
  onSaveConfig: () => void;
}

/**
 * 远端地址与 SSH 主机白名单表单：本地编辑态由上层持有，
 * 本组件只负责布局与受控回写。
 */
export const RemoteConfigForm: React.FC<RemoteConfigFormProps> = ({
  remoteUrl,
  onRemoteUrlChange,
  remoteUrlRef,
  testing,
  onTestConnection,
  sshHostsVisible,
  sshHosts,
  onSshHostsChange,
  isNoSync,
  onSaveConfig,
}) => {
  const { t } = useTranslation();

  return (
    <Form layout="vertical">
      <Form.Item label={t("vault.sync.remote_url")}>
        <Space.Compact style={{ width: "100%" }}>
          <Input
            ref={remoteUrlRef}
            value={remoteUrl}
            onChange={(e) => onRemoteUrlChange(e.target.value)}
            placeholder="git@github.com:user/vault.git"
          />
          <Button
            icon={<LinkOutlined />}
            loading={testing}
            disabled={!remoteUrl.trim()}
            onClick={onTestConnection}
          >
            {t("vault.sync.test_connection")}
          </Button>
        </Space.Compact>
      </Form.Item>

      {sshHostsVisible && (
        <Form.Item
          label={t("vault.sync.ssh_hosts")}
          extra={t("vault.sync.ssh_hosts_hint")}
        >
          <Input
            value={sshHosts}
            onChange={(e) => onSshHostsChange(e.target.value)}
            placeholder="github.com, gitlab.com"
          />
        </Form.Item>
      )}

      {!isNoSync && (
        <Form.Item>
          <Button type="primary" onClick={onSaveConfig}>
            {t("common.actions.save")}
          </Button>
        </Form.Item>
      )}
    </Form>
  );
};
