import { useState } from "react";
import { Alert, Descriptions, Form, Input, Typography } from "antd";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "../core/vaultStore";
import type { SshKeyMeta } from "../core/vault.types";

interface SshKeyFieldsProps {
  privateKey: string;
  onPrivateKeyChange: (value: string) => void;
  hasPrivateKey?: boolean;
}

export default function SshKeyFields({
  privateKey,
  onPrivateKeyChange,
  hasPrivateKey = false,
}: SshKeyFieldsProps) {
  const { t } = useTranslation();
  const sshParseKey = useVaultStore((s) => s.sshParseKey);
  const [meta, setMeta] = useState<SshKeyMeta | null>(null);
  const [parseFailed, setParseFailed] = useState(false);

  const handleChange = (value: string) => {
    onPrivateKeyChange(value);
    if (meta) setMeta(null);
    if (parseFailed) setParseFailed(false);
  };

  const handleBlur = async () => {
    const value = privateKey.trim();
    if (!value) {
      setMeta(null);
      setParseFailed(false);
      return;
    }
    const result = await sshParseKey(value);
    setMeta(result);
    setParseFailed(!result);
  };

  return (
    <>
      <Form.Item label={t("vault.entry.ssh_private_key")}>
        <Input.TextArea
          rows={6}
          value={privateKey}
          onChange={(e) => handleChange(e.target.value)}
          onBlur={() => void handleBlur()}
          placeholder={
            hasPrivateKey && !privateKey
              ? t("vault.entry.ssh_private_key_set_hint")
              : "-----BEGIN OPENSSH PRIVATE KEY-----"
          }
          spellCheck={false}
          style={{ fontFamily: "monospace" }}
        />
      </Form.Item>

      {parseFailed && (
        <Alert type="error" showIcon title={t("vault.ssh.parse_failed")} />
      )}

      {meta && (
        <Descriptions
          size="small"
          column={1}
          bordered
          style={{ marginBottom: 16 }}
        >
          <Descriptions.Item label={t("vault.ssh.meta_key_type")}>
            {meta.keyType}
          </Descriptions.Item>
          <Descriptions.Item label={t("vault.ssh.meta_bits")}>
            {meta.bits}
          </Descriptions.Item>
          <Descriptions.Item label={t("vault.ssh.meta_fingerprint")}>
            <Typography.Text copyable>{meta.fingerprint}</Typography.Text>
          </Descriptions.Item>
          {meta.comment && (
            <Descriptions.Item label={t("vault.ssh.meta_comment")}>
              {meta.comment}
            </Descriptions.Item>
          )}
          <Descriptions.Item label={t("vault.ssh.public_key")}>
            <Typography.Text copyable code>
              {meta.publicKey}
            </Typography.Text>
          </Descriptions.Item>
        </Descriptions>
      )}
    </>
  );
}
