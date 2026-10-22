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
  InputNumber,
  message,
  Typography,
} from "antd";
import {
  PlusOutlined,
  MinusCircleOutlined,
  LinkOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "./vaultStore";
import type {
  VaultCategory,
  CustomFieldInput,
  ParsedOtpAuth,
} from "./vault.service";
import {
  generatePassword,
  generatePassphrase,
} from "../password-generator/password-generator.service";
import styles from "./password-vault.module.css";

type PasswordGenMode = "random" | "passphrase";

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
  const { selectedEntry, createEntry, updateEntry, totpParseUri } =
    useVaultStore();

  const [category, setCategory] = useState<VaultCategory>("login");
  const [title, setTitle] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [url, setUrl] = useState("");
  const [notes, setNotes] = useState("");
  const [totpSecret, setTotpSecret] = useState("");
  const [totpUriInput, setTotpUriInput] = useState("");
  const [totpUriVisible, setTotpUriVisible] = useState(false);
  const [tags, setTags] = useState<string[]>([]);
  const [tagInput, setTagInput] = useState("");
  const [isFavorite, setIsFavorite] = useState(false);
  const [customFields, setCustomFields] = useState<CustomFieldInput[]>([]);
  const [genMode, setGenMode] = useState<PasswordGenMode>("random");
  const [genLength, setGenLength] = useState(16);
  const [genWordCount, setGenWordCount] = useState(4);
  const [genSeparator, setGenSeparator] = useState("-");
  const [genCapitalize, setGenCapitalize] = useState(false);

  const isEditingExistingTotp = !!(editId && selectedEntry?.hasTotp);

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

  const handleImportTotpUri = async () => {
    const uri = totpUriInput.trim();
    if (!uri) return;
    try {
      const parsed: ParsedOtpAuth = await totpParseUri(uri);
      setTotpSecret(parsed.secret);
      if (parsed.issuer && !title) {
        setTitle(parsed.issuer);
      }
      if (parsed.account && !username) {
        setUsername(parsed.account);
      }
      setTotpUriInput("");
      setTotpUriVisible(false);
      message.success(t("tools.password_vault.totp_uri_imported"));
    } catch {
      message.error(t("tools.password_vault.totp_uri_invalid"));
    }
  };

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
            <Space orientation="vertical" style={{ width: "100%" }}>
              <Input.Password
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
              <Space>
                <Select
                  value={genMode}
                  onChange={setGenMode}
                  size="small"
                  style={{ width: 120 }}
                  options={[
                    {
                      value: "random",
                      label: t("tools.password_vault.gen_mode_random"),
                    },
                    {
                      value: "passphrase",
                      label: t("tools.password_vault.gen_mode_passphrase"),
                    },
                  ]}
                />
                {genMode === "random" && (
                  <InputNumber
                    min={8}
                    max={64}
                    value={genLength}
                    onChange={(v) => setGenLength(v ?? 16)}
                    size="small"
                    style={{ width: 70 }}
                  />
                )}
                {genMode === "passphrase" && (
                  <>
                    <InputNumber
                      min={3}
                      max={10}
                      value={genWordCount}
                      onChange={(v) => setGenWordCount(v ?? 4)}
                      size="small"
                      style={{ width: 70 }}
                    />
                    <Input
                      value={genSeparator}
                      onChange={(e) => setGenSeparator(e.target.value)}
                      size="small"
                      style={{ width: 50 }}
                    />
                    <Switch
                      checked={genCapitalize}
                      onChange={setGenCapitalize}
                      size="small"
                    />
                  </>
                )}
                <Button
                  size="small"
                  onClick={() => {
                    if (genMode === "random") {
                      setPassword(
                        generatePassword(genLength, true, true, true, true),
                      );
                    } else {
                      setPassword(
                        generatePassphrase(
                          genWordCount,
                          genSeparator,
                          genCapitalize,
                        ),
                      );
                    }
                  }}
                >
                  {t("tools.password_vault.generate")}
                </Button>
              </Space>
            </Space>
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
            <Space orientation="vertical" style={{ width: "100%" }}>
              {isEditingExistingTotp && !totpSecret && (
                <Typography.Text
                  type="secondary"
                  className={styles.totpEditHint}
                >
                  {t("tools.password_vault.totp_edit_hint")}
                </Typography.Text>
              )}
              <Input.Password
                value={totpSecret}
                onChange={(e) => setTotpSecret(e.target.value)}
                placeholder={
                  isEditingExistingTotp
                    ? t("tools.password_vault.totp_placeholder_keep")
                    : undefined
                }
              />
              {!totpUriVisible ? (
                <Button
                  type="link"
                  size="small"
                  icon={<LinkOutlined />}
                  onClick={() => setTotpUriVisible(true)}
                >
                  {t("tools.password_vault.totp_import_uri")}
                </Button>
              ) : (
                <Space.Compact style={{ width: "100%" }}>
                  <Input
                    value={totpUriInput}
                    onChange={(e) => setTotpUriInput(e.target.value)}
                    placeholder="otpauth://totp/..."
                    onPressEnter={() => void handleImportTotpUri()}
                    style={{ flex: 1 }}
                  />
                  <Button
                    type="primary"
                    size="small"
                    onClick={() => void handleImportTotpUri()}
                  >
                    {t("tools.password_vault.totp_import")}
                  </Button>
                  <Button
                    size="small"
                    onClick={() => {
                      setTotpUriVisible(false);
                      setTotpUriInput("");
                    }}
                  >
                    {t("common.actions.cancel")}
                  </Button>
                </Space.Compact>
              )}
            </Space>
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
