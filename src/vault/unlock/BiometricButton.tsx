import { Button, Divider } from "antd";
import { SafetyOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";

interface BiometricButtonProps {
  unlockMethod: "password" | "biometric" | null;
  onUnlock: () => void;
  primary?: boolean;
}

export default function BiometricButton({
  unlockMethod,
  onUnlock,
  primary = false,
}: BiometricButtonProps) {
  const { t } = useTranslation();

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