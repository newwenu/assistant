import { Button } from "antd";
import { EyeOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";

interface RevealFieldButtonProps {
  onClick: () => void;
}

export default function RevealFieldButton({ onClick }: RevealFieldButtonProps) {
  const { t } = useTranslation();

  return (
    <Button type="text" size="small" icon={<EyeOutlined />} onClick={onClick}>
      {t("vault.entry.reveal")}
    </Button>
  );
}
