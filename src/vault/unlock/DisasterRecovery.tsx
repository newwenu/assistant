import { useState } from "react";
import { Button } from "antd";
import { SyncOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import PaperRecoveryFlow from "./PaperRecoveryFlow";

interface DisasterRecoveryProps {
  disabled?: boolean;
}

export default function DisasterRecovery({ disabled }: DisasterRecoveryProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);

  return (
    <>
      <Button
        type="link"
        size="small"
        icon={<SyncOutlined />}
        disabled={disabled}
        onClick={() => setOpen(true)}
      >
        {t("vault.recovery.paper.entry")}
      </Button>
      <PaperRecoveryFlow open={open} onClose={() => setOpen(false)} />
    </>
  );
}
