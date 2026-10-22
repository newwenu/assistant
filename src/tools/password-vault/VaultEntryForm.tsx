import { useState, useEffect } from "react";
import {
  Form,
  Input,
  Select,
  Switch,
  Button,
  Space,
  Divider,
  Tag,
  message,
} from "antd";
import { PlusOutlined, MinusCircleOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "./vaultStore";
import type { VaultCategory, CustomFieldInput } from "./vault.service";
import styles from "./password-vault.module.css";

const CATEGORY_OPTIONS: { value: VaultCategory; label: string }[] = [
  { value: "login", label: "Login" },
  { value: "note", label: "Note" },
  { value: "card", label: "Card" },
  { value: "identity", label: "Identity" },
  { value: "sshKey", label: "SSH Key" },
  { value: "custom", label: "Custom" },
];

interface VaultEntryFormProps {
  editId?: string;
  onCancel: () => void;
  onSaved: (id: string) => void;
}

export default function VaultEntryForm({
  editId,
  onCancel,
  onSaved,
}: VaultEntryFormProps) {
  const { t } = useTranslation();
  const { selectedEntry, createEntry, updateEntry } = useVaultStore();

  const [category, setCategory] = useState<VaultCategory>("login");
  const [title, setTitle] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [url, setUrl] = useState("");
  const [notes, setNotes] = useState("");
  const [totpSecret, setTotpSecret] = useState("");
  const [tags, setTags] = useState<string[]>([]);
  const [tagInput, setTagInput] = useState("");
  const [isFavorite, setIsFavorite] = useState(false);
  const [customFields, setCustomFields] = useState<CustomFieldInput[]>([]);

  useEffect(() => {
    if (editId && selectedEntry) {
      setCategory(selectedEntry.category);
      setTitle(selectedEntry.title);
      setUsername(selectedEntry.username ?? "");
      setUrl(selectedEntry.url ?? "");
      setIsFavorite(selectedEntry.isFavorite);
      setTags(selectedEntry.tags);
    }
  }, [editId, selectedEntry]);

  const handleAddTag = () => {
    if (tagInput && !tags.includes(tagInput)) {
      setTags([...tags, tagInput]);
      setTagInput("");
    }
  };

  const handleRemoveTag = (tag: string) => {
    setTags(tags.filter((t) => t !== tag));
  };

  const handleAddCustomField = () => {
    setCustomFields([
      ...customFields,
      { name: "", value: "", isProtected: false },
    ]);
  };

  const handleSave = async () => {
    if (!title.trim()) {
      message.error(t("tools.password_vault.title_required"));
      return;
    }

    const entryData = {
      category,
      title: title.trim(),
      username: username || undefined,
      password: password || undefined,
      url: url || undefined,
      notes: notes || undefined,
      totpSecret: totpSecret || undefined,
      tags,
      customFields,
      isFavorite,
    };

    try {
      if (editId) {
        await updateEntry(editId, entryData);
        onSaved(editId);
      } else {
        const id = await createEntry(entryData);
        onSaved(id);
      }
    } catch {
      message.error(t("tools.password_vault.save_failed"));
    }
  };

  return (
    <div className={styles.formContainer}>
      <Form layout="vertical">
        <Form.Item label={t("tools.password_vault.category")}>
          <Select
            value={category}
            onChange={(v) => setCategory(v)}
            options={CATEGORY_OPTIONS.map((o) => ({
              ...o,
              label: t(`tools.password_vault.category_${o.value}`),
            }))}
          />
        </Form.Item>

        <Form.Item label={t("tools.password_vault.title")} required>
          <Input value={title} onChange={(e) => setTitle(e.target.value)} />
        </Form.Item>

        {(category === "login" || category === "card") && (
          <Form.Item label={t("tools.password_vault.username")}>
            <Input
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
          </Form.Item>
        )}

        {(category === "login" || category === "card") && (
          <Form.Item label={t("tools.password_vault.password")}>
            <Input.Password
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </Form.Item>
        )}

        {category === "login" && (
          <Form.Item label={t("tools.password_vault.url")}>
            <Input value={url} onChange={(e) => setUrl(e.target.value)} />
          </Form.Item>
        )}

        <Form.Item label={t("tools.password_vault.notes")}>
          <Input.TextArea
            rows={3}
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
          />
        </Form.Item>

        {category === "login" && (
          <Form.Item label={t("tools.password_vault.totp_secret")}>
            <Input.Password
              value={totpSecret}
              onChange={(e) => setTotpSecret(e.target.value)}
            />
          </Form.Item>
        )}

        <Form.Item label={t("tools.password_vault.tags")}>
          <Space wrap>
            {tags.map((tag) => (
              <Tag key={tag} closable onClose={() => handleRemoveTag(tag)}>
                {tag}
              </Tag>
            ))}
            <Input
              size="small"
              value={tagInput}
              onChange={(e) => setTagInput(e.target.value)}
              onPressEnter={handleAddTag}
              placeholder={t("tools.password_vault.add_tag")}
              style={{ width: 120 }}
              suffix={
                <PlusOutlined
                  onClick={handleAddTag}
                  style={{ cursor: "pointer" }}
                />
              }
            />
          </Space>
        </Form.Item>

        <Form.Item label={t("tools.password_vault.favorite")}>
          <Switch checked={isFavorite} onChange={setIsFavorite} />
        </Form.Item>

        <Divider>{t("tools.password_vault.custom_fields")}</Divider>
        {customFields.map((cf, idx) => (
          <div key={idx} className={styles.customFieldRow}>
            <Input
              placeholder={t("tools.password_vault.field_name")}
              value={cf.name}
              onChange={(e) => {
                const updated = [...customFields];
                updated[idx] = { ...updated[idx], name: e.target.value };
                setCustomFields(updated);
              }}
              style={{ flex: 1 }}
            />
            {cf.isProtected ? (
              <Input.Password
                placeholder={t("tools.password_vault.field_value")}
                value={cf.value}
                onChange={(e) => {
                  const updated = [...customFields];
                  updated[idx] = { ...updated[idx], value: e.target.value };
                  setCustomFields(updated);
                }}
                style={{ flex: 1 }}
              />
            ) : (
              <Input
                placeholder={t("tools.password_vault.field_value")}
                value={cf.value}
                onChange={(e) => {
                  const updated = [...customFields];
                  updated[idx] = { ...updated[idx], value: e.target.value };
                  setCustomFields(updated);
                }}
                style={{ flex: 1 }}
              />
            )}
            <Switch
              checked={cf.isProtected}
              onChange={(v) => {
                const updated = [...customFields];
                updated[idx] = { ...updated[idx], isProtected: v };
                setCustomFields(updated);
              }}
              checkedChildren="🔒"
              unCheckedChildren="📝"
            />
            <Button
              type="text"
              danger
              icon={<MinusCircleOutlined />}
              onClick={() =>
                setCustomFields(customFields.filter((_, i) => i !== idx))
              }
            />
          </div>
        ))}
        <Button
          type="dashed"
          icon={<PlusOutlined />}
          onClick={handleAddCustomField}
          block
        >
          {t("tools.password_vault.add_custom_field")}
        </Button>

        <Divider />
        <Space>
          <Button type="primary" onClick={() => void handleSave()}>
            {t("common.actions.save")}
          </Button>
          <Button onClick={onCancel}>{t("common.actions.cancel")}</Button>
        </Space>
      </Form>
    </div>
  );
}
