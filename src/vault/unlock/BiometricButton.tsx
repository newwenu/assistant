import { Button, Divider, Typography } from "antd";
import { SafetyOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";

interface BiometricButtonProps {
  unlockMethod: "password" | "biometric" | null;
  onUnlock: () => void;
  primary?: boolean;
  resetRequired?: boolean;
}

export default function BiometricButton({
  unlockMethod,
  onUnlock,
  primary = false,
  resetRequired = false,
}: BiometricButtonProps) {
  const { t } = useTranslation();

  if (resetRequired) {
    return (
      <Typography.Text
        type="warning"
        style={{
          display: "block",
          textAlign: "center",
          marginBottom: 12,
          fontSize: 12,
        }}
      >
        {t("vault.biometric.reenroll_required")}
      </Typography.Text>
    );
  }

  return (
    <>
      {!primary && <Divider plain>{t("common.or")}</Divider>}
      <Button
        type={primary ? "primary" : "default"}
        size="large"
        block
        icon={<SafetyOutlined />}
        loading={unlockMethod === "biometric"}
        disabled={!!unlockMethod && unlockMethod !== "biometric"}
        onClick={() => void onUnlock()}
      >
        {t("vault.unlock.biometric_unlock")}
      </Button>
    </>
  );
}
