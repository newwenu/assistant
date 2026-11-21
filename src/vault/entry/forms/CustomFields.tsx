import { Input, Switch, Button, Divider } from "antd";
import { PlusOutlined, MinusCircleOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { CustomFieldInput } from "../../core/vault.types";
import styles from "../../vault.module.css";

interface CustomFieldsProps {
  customFields: CustomFieldInput[];
  onCustomFieldsChange: (fields: CustomFieldInput[]) => void;
}

export default function CustomFields({
  customFields,
  onCustomFieldsChange,
}: CustomFieldsProps) {
  const { t } = useTranslation();

  const handleAdd = () => {
    onCustomFieldsChange([
      ...customFields,
      { name: "", value: "", isProtected: false },
    ]);
  };

  return (
    <>
      <Divider>{t("vault.entry.custom_fields")}</Divider>
      {customFields.map((cf, idx) => (
        <div key={idx} className={styles.customFieldRow}>
          <Input
            placeholder={t("vault.entry.field_name")}
            value={cf.name}
            onChange={(e) => {
              const updated = [...customFields];
              updated[idx] = { ...updated[idx], name: e.target.value };
              onCustomFieldsChange(updated);
            }}
            style={{ flex: 1 }}
          />
          {cf.isProtected ? (
            <Input.Password
              placeholder={t("vault.entry.field_value")}
              value={cf.value}
              onChange={(e) => {
                const updated = [...customFields];
                updated[idx] = { ...updated[idx], value: e.target.value };
                onCustomFieldsChange(updated);
              }}
              style={{ flex: 1 }}
            />
          ) : (
            <Input
              placeholder={t("vault.entry.field_value")}
              value={cf.value}
              onChange={(e) => {
                const updated = [...customFields];
                updated[idx] = { ...updated[idx], value: e.target.value };
                onCustomFieldsChange(updated);
              }}
              style={{ flex: 1 }}
            />
          )}
          <Switch
            checked={cf.isProtected}
            onChange={(v) => {
              const updated = [...customFields];
              updated[idx] = { ...updated[idx], isProtected: v };
              onCustomFieldsChange(updated);
            }}
            checkedChildren="🔒"
            unCheckedChildren="📝"
          />
          <Button
            type="text"
            danger
            icon={<MinusCircleOutlined />}
            onClick={() =>
              onCustomFieldsChange(customFields.filter((_, i) => i !== idx))
            }
          />
        </div>
      ))}
      <Button type="dashed" icon={<PlusOutlined />} onClick={handleAdd} block>
        {t("vault.entry.add_custom_field")}
      </Button>
    </>
  );
}
