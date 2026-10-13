import { Button, Tooltip } from "antd";
import { SettingOutlined } from "@ant-design/icons";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";

export const SettingsEntry: React.FC = () => {
  const navigate = useNavigate();
  const { t } = useTranslation();

  return (
    <Tooltip title={t("common.actions.settings")} mouseEnterDelay={0.5}>
      <Button
        type="text"
        size="small"
        icon={<SettingOutlined />}
        onClick={() => {
          void navigate("/settings/general");
        }}
        style={{ width: 32, height: 28 }}
      />
    </Tooltip>
  );
};
