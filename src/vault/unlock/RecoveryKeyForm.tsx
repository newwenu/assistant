import { Form, Input, Button, Divider } from "antd";
import { KeyOutlined, UnlockOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";

interface RecoveryKeyFormProps {
  recoveryKey: string;
  onRecoveryKeyChange: (value: string) => void;
  showRecoveryKey: boolean;
  onShowRecoveryKeyChange: (show: boolean) => void;
  unlockMethod: "password" | "biometric" | "recoveryKey" | null;
  onSubmit: () => void;
}

export default function RecoveryKeyForm({
  recoveryKey,
  onRecoveryKeyChange,
  showRecoveryKey,
  onShowRecoveryKeyChange,
  unlockMethod,
  onSubmit,
}: RecoveryKeyFormProps) {
  const { t } = useTranslation();

  if (!showRecoveryKey) {
    return (
      <>
        <Divider plain>{t("common.or")}</Divider>
        <Button
          size="large"
          block
          icon={<KeyOutlined />}
          disabled={!!unlockMethod}
          onClick={() => onShowRecoveryKeyChange(true)}
        >
          {t("vault.unlock.recovery_key_unlock")}
        </Button>
      </>
    );
  }

  return (
    <>
      <Divider plain>{t("vault.unlock.recovery_key_divider")}</Divider>
      <Form.Item>
        <Input
          size="large"
          placeholder={t("vault.unlock.recovery_key_placeholder")}
          value={recoveryKey}
          onChange={(e) => onRecoveryKeyChange(e.target.value)}
          prefix={<KeyOutlined />}
          autoFocus
        />
      </Form.Item>
      <Form.Item>
        <Button
          type="primary"
          size="large"
          block
          icon={<UnlockOutlined />}
          loading={unlockMethod === "recoveryKey"}
          disabled={!recoveryKey || !!unlockMethod}
          onClick={() => void onSubmit()}
        >
          {t("vault.unlock.unlock_with_recovery_key")}
        </Button>
      </Form.Item>
    </>
  );
}
