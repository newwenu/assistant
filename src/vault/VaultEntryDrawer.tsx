import { useState, useEffect } from "react";
import {
  Drawer,
  Form,
  Input,
  Select,
  Switch,
  Button,
  Space,
  Divider,
  Tag,
  InputNumber,
  App,
  Typography,
} from "antd";
import {
  PlusOutlined,
  MinusCircleOutlined,
  LinkOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "./core/vaultStore";
import type {
  VaultCategory,
  CustomFieldInput,
  ParsedOtpAuth,
} from "./core/vault.service";
import {
  generatePassword,
  generatePassphrase,
} from "@/tools/password-generator/password-generator.service";
import styles from "./vault.module.css";

type PasswordGenMode = "random" | "passphrase";

const CATEGORY_OPTIONS: { value: VaultCategory; label: string }[] = [
  { value: "login", label: "Login" },
  { value: "note", label: "Note" },
  { value: "card", label: "Card" },
  { value: "identity", label: "Identity" },
  { value: "sshKey", label: "SSH Key" },
  { value: "custom", label: "Custom" },
];

interface VaultEntryDrawerProps {
  open: boolean;
  editId: string | null;
  onClose: () => void;
  onSaved: (id: string) => void;
}

export default function VaultEntryDrawer({
  open,
  editId,
  onClose,
  onSaved,
}: VaultEntryDrawerProps) {
  const { t } = useTranslation();
  const { message } = App.useApp();
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
  const [totpUriVisible, setTotpUriVisible] = useState(true);
  const [totpSecretVisible, setTotpSecretVisible] = useState(false);
  const [tags, setTags] = useState<string[]>([]);
  const [tagInput, setTagInput] = useState("");
  const [isFavorite, setIsFavorite] = useState(false);
  const [customFields, setCustomFields] = useState<CustomFieldInput[]>([]);
  const [genMode, setGenMode] = useState<PasswordGenMode>("random");
  const [genLength, setGenLength] = useState(16);
  const [genWordCount, setGenWordCount] = useState(4);
  const [genSeparator, setGenSeparator] = useState("-");
  const [genCapitalize, setGenCapitalize] = useState(false);
  const [saving, setSaving] = useState(false);

  const isEditing = !!editId;
  const isEditingExistingTotp = !!(editId && selectedEntry?.hasTotp);

  useEffect(() => {
    if (!open) return;
    if (editId && selectedEntry) {
      setCategory(selectedEntry.category);
      setTitle(selectedEntry.title);
      setUsername(selectedEntry.username ?? "");
      setUrl(selectedEntry.url ?? "");
      setIsFavorite(selectedEntry.isFavorite);
      setTags(selectedEntry.tags);
    } else {
      setCategory("login");
      setTitle("");
      setUsername("");
      setPassword("");
      setUrl("");
      setNotes("");
      setTotpSecret("");
      setTotpUriInput("");
      setTotpUriVisible(true);
      setTotpSecretVisible(false);
      setTags([]);
      setIsFavorite(false);
      setCustomFields([]);
    }
  }, [open, editId, selectedEntry]);

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
      setTotpSecretVisible(false);
      message.success(t("vault.totp.uri_imported"));
    } catch {
      message.error(t("vault.totp.uri_invalid"));
    }
  };

  const handleAddTag = () => {
    if (tagInput && !tags.includes(tagInput)) {
      setTags([...tags, tagInput]);
      setTagInput("");
    }
  };

  const handleRemoveTag = (tag: string) => {
    setTags(tags.filter((v) => v !== tag));
  };

  const handleAddCustomField = () => {
    setCustomFields([
      ...customFields,
      { name: "", value: "", isProtected: false },
    ]);
  };

  const handleSave = async () => {
    if (!title.trim()) {
      message.error(t("vault.entry.title_required"));
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

    setSaving(true);
    try {
      if (editId) {
        await updateEntry(editId, entryData);
        onSaved(editId);
      } else {
        const id = await createEntry(entryData);
        onSaved(id);
      }
    } catch {
      message.error(t("vault.entry.save_failed"));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Drawer
      title={
        isEditing
          ? t("vault.entry.edit_entry")
          : t("vault.entry.add_entry")
      }
      open={open}
      onClose={onClose}
      size="large"
      destroyOnHidden
      extra={
        <Button
          type="primary"
          loading={saving}
          onClick={() => void handleSave()}
        >
          {t("common.actions.save")}
        </Button>
      }
    >
      <Form layout="vertical">
        <Form.Item label={t("vault.entry.category")}>
          <Select
            value={category}
            onChange={(v) => setCategory(v)}
            options={CATEGORY_OPTIONS.map((o) => ({
              ...o,
              label: t(`vault.entry.category_${o.value}`),
            }))}
          />
        </Form.Item>

        <Form.Item label={t("vault.entry.title")} required>
          <Input value={title} onChange={(e) => setTitle(e.target.value)} />
        </Form.Item>

        {(category === "login" || category === "card") && (
          <Form.Item label={t("vault.entry.username")}>
            <Input
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
          </Form.Item>
        )}

        {(category === "login" || category === "card") && (
          <Form.Item label={t("vault.entry.password")}>
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
                      label: t("vault.entry.gen_mode_random"),
                    },
                    {
                      value: "passphrase",
                      label: t("vault.entry.gen_mode_passphrase"),
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
                  {t("vault.entry.generate")}
                </Button>
              </Space>
            </Space>
          </Form.Item>
        )}

        {category === "login" && (
          <Form.Item label={t("vault.entry.url")}>
            <Input value={url} onChange={(e) => setUrl(e.target.value)} />
          </Form.Item>
        )}

        <Form.Item label={t("vault.entry.notes")}>
          <Input.TextArea
            rows={3}
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
          />
        </Form.Item>

        {category === "login" && (
          <Form.Item label={t("vault.entry.totp_secret")}>
            <Space orientation="vertical" style={{ width: "100%" }}>
              {isEditingExistingTotp && !totpSecret && (
                <span className={styles.totpEditHint}>
                  {t("vault.totp.edit_hint")}
                </span>
              )}
              {totpSecret && !totpSecretVisible && !totpUriVisible && (
                <div className={styles.totpSecretStatus}>
                  <Typography.Text
                    type="success"
                    className={styles.totpSecretBadge}
                  >
                    ✓ {t("vault.totp.secret_set")}
                  </Typography.Text>
                  <Space size={4}>
                    <Button
                      type="link"
                      size="small"
                      onClick={() => {
                        setTotpSecret("");
                        setTotpUriVisible(true);
                      }}
                    >
                      {t("vault.totp.clear")}
                    </Button>
                    <Button
                      type="link"
                      size="small"
                      onClick={() => setTotpSecretVisible(true)}
                    >
                      {t("vault.totp.edit_secret")}
                    </Button>
                  </Space>
                </div>
              )}
              {totpUriVisible && (
                <div className={styles.totpUriSection}>
                  <Input
                    value={totpUriInput}
                    onChange={(e) => setTotpUriInput(e.target.value)}
                    placeholder="otpauth://totp/..."
                    onPressEnter={() => void handleImportTotpUri()}
                    prefix={<LinkOutlined />}
                    className={styles.totpUriInput}
                  />
                  <Space size={8} style={{ marginTop: 8 }}>
                    <Button
                      type="primary"
                      size="small"
                      onClick={() => void handleImportTotpUri()}
                    >
                      {t("vault.totp.import")}
                    </Button>
                    <Button
                      type="link"
                      size="small"
                      onClick={() => {
                        setTotpUriVisible(false);
                        setTotpUriInput("");
                        setTotpSecretVisible(true);
                      }}
                    >
                      {t("vault.totp.manual_input")}
                    </Button>
                  </Space>
                </div>
              )}
              {totpSecretVisible && (
                <div className={styles.totpSecretSection}>
                  <Input.Password
                    value={totpSecret}
                    onChange={(e) => setTotpSecret(e.target.value)}
                    placeholder={
                      isEditingExistingTotp
                        ? t("vault.totp.placeholder_keep")
                        : t("vault.totp.secret_placeholder")
                    }
                  />
                  <Space size={8} style={{ marginTop: 8 }}>
                    <Button
                      type="link"
                      size="small"
                      icon={<LinkOutlined />}
                      onClick={() => {
                        setTotpSecretVisible(false);
                        setTotpUriVisible(true);
                      }}
                    >
                      {t("vault.totp.import_uri")}
                    </Button>
                  </Space>
                </div>
              )}
            </Space>
          </Form.Item>
        )}

        <Form.Item label={t("vault.entry.tags")}>
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
              placeholder={t("vault.entry.add_tag")}
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

        <Form.Item label={t("vault.entry.favorite")}>
          <Switch checked={isFavorite} onChange={setIsFavorite} />
        </Form.Item>

        <Divider>{t("vault.entry.custom_fields")}</Divider>
        {customFields.map((cf, idx) => (
          <div key={idx} className={styles.customFieldRow}>
            <Input
              placeholder={t("vault.entry.field_name")}
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
                placeholder={t("vault.entry.field_value")}
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
                placeholder={t("vault.entry.field_value")}
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
          {t("vault.entry.add_custom_field")}
        </Button>
      </Form>
    </Drawer>
  );
}