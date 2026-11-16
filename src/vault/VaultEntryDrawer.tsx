import { useState, useEffect } from "react";
import {
  Drawer,
  Form,
  Input,
  Select,
  Switch,
  Button,
  Space,
  Tag,
  App,
} from "antd";
import { PlusOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultShallow } from "./core/vaultStore";
import { vaultService } from "./core/vault.service";
import { vaultCategoryOptions } from "./core/display/vaultCategory";
import type {
  VaultCategory,
  CustomFieldInput,
  ParsedOtpAuth,
  TotpAlgorithm,
} from "./core/vault.types";
import LoginFields from "./forms/LoginFields";
import CardFields from "./forms/CardFields";
import NoteFields from "./forms/NoteFields";
import IdentityFields from "./forms/IdentityFields";
import SshKeyFields from "./forms/SshKeyFields";
import CustomFields from "./forms/CustomFields";

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
    useVaultShallow((s) => ({
      selectedEntry: s.selectedEntry,
      createEntry: s.createEntry,
      updateEntry: s.updateEntry,
      totpParseUri: s.totpParseUri,
    }));

  const [category, setCategory] = useState<VaultCategory>("login");
  const [title, setTitle] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [url, setUrl] = useState("");
  const [notes, setNotes] = useState("");
  const [totpSecret, setTotpSecret] = useState("");
  const [totpDigits, setTotpDigits] = useState(6);
  const [totpPeriod, setTotpPeriod] = useState(30);
  const [totpAlgorithm, setTotpAlgorithm] = useState<TotpAlgorithm>("sha1");
  const [totpUriInput, setTotpUriInput] = useState("");
  const [totpUriVisible, setTotpUriVisible] = useState(true);
  const [totpSecretVisible, setTotpSecretVisible] = useState(false);
  const [tags, setTags] = useState<string[]>([]);
  const [tagInput, setTagInput] = useState("");
  const [isFavorite, setIsFavorite] = useState(false);
  const [customFields, setCustomFields] = useState<CustomFieldInput[]>([]);
  const [saving, setSaving] = useState(false);
  const [passwordDirty, setPasswordDirty] = useState(false);
  const [notesDirty, setNotesDirty] = useState(false);
  const [totpDirty, setTotpDirty] = useState(false);

  const isEditing = !!editId;
  const isEditingExistingTotp = !!(editId && selectedEntry?.hasTotp);

  const handlePasswordChange = (v: string) => {
    setPassword(v);
    setPasswordDirty(true);
  };
  const handleNotesChange = (v: string) => {
    setNotes(v);
    setNotesDirty(true);
  };
  const handleTotpSecretChange = (v: string) => {
    setTotpSecret(v);
    setTotpDirty(true);
  };

  useEffect(() => {
    if (!open) return;
    if (editId && selectedEntry) {
      setCategory(selectedEntry.category);
      setTitle(selectedEntry.title);
      setUsername(selectedEntry.username ?? "");
      setUrl(selectedEntry.url ?? "");
      setIsFavorite(selectedEntry.isFavorite);
      setTags(selectedEntry.tags);
      setPassword("");
      setTotpSecret("");
      setTotpDigits(selectedEntry.totpDigits ?? 6);
      setTotpPeriod(selectedEntry.totpPeriod ?? 30);
      setTotpAlgorithm(selectedEntry.totpAlgorithm ?? "sha1");
      setPasswordDirty(false);
      setNotesDirty(false);
      setTotpDirty(false);
      if (selectedEntry.hasNotes) {
        vaultService
          .revealNotes(editId)
          .then((result) => setNotes(result.value))
          .catch(() => setNotes(""));
      } else {
        setNotes("");
      }
    } else {
      setCategory("login");
      setTitle("");
      setUsername("");
      setPassword("");
      setUrl("");
      setNotes("");
      setTotpSecret("");
      setTotpDigits(6);
      setTotpPeriod(30);
      setTotpAlgorithm("sha1");
      setTotpUriInput("");
      setTotpUriVisible(true);
      setTotpSecretVisible(false);
      setTags([]);
      setIsFavorite(false);
      setCustomFields([]);
      setPasswordDirty(false);
      setNotesDirty(false);
      setTotpDirty(false);
    }
  }, [open, editId, selectedEntry]);

  const handleImportTotpUri = async () => {
    const uri = totpUriInput.trim();
    if (!uri) return;
    try {
      const parsed: ParsedOtpAuth = await totpParseUri(uri);
      handleTotpSecretChange(parsed.secret);
      setTotpDigits(parsed.digits);
      setTotpPeriod(parsed.period);
      setTotpAlgorithm(parsed.algorithm);
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

  const handleSave = async () => {
    if (!title.trim()) {
      message.error(t("vault.entry.title_required"));
      return;
    }

    const totp = (() => {
      if (isEditing && !totpDirty) return undefined;
      if (isEditing) {
        return {
          secret: totpSecret || null,
          digits: totpDigits,
          period: totpPeriod,
          algorithm: totpAlgorithm,
        };
      }
      return totpSecret
        ? {
            secret: totpSecret,
            digits: totpDigits,
            period: totpPeriod,
            algorithm: totpAlgorithm,
          }
        : undefined;
    })();

    const entryData = {
      category,
      title: title.trim(),
      username: username || undefined,
      password: isEditing && !passwordDirty ? undefined : password || undefined,
      url: url || undefined,
      notes: isEditing && !notesDirty ? undefined : notes || undefined,
      totp,
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
        const detail = await createEntry(entryData);
        onSaved(detail.id);
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
        isEditing ? t("vault.entry.edit_entry") : t("vault.entry.add_entry")
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
            options={vaultCategoryOptions(t)}
          />
        </Form.Item>

        <Form.Item label={t("vault.entry.title")} required>
          <Input value={title} onChange={(e) => setTitle(e.target.value)} />
        </Form.Item>

        {category === "login" && (
          <LoginFields
            username={username}
            onUsernameChange={setUsername}
            password={password}
            onPasswordChange={handlePasswordChange}
            url={url}
            onUrlChange={setUrl}
            totpSecret={totpSecret}
            onTotpSecretChange={handleTotpSecretChange}
            totpUriInput={totpUriInput}
            onTotpUriInputChange={setTotpUriInput}
            totpUriVisible={totpUriVisible}
            onTotpUriVisibleChange={setTotpUriVisible}
            totpSecretVisible={totpSecretVisible}
            onTotpSecretVisibleChange={setTotpSecretVisible}
            isEditingExistingTotp={isEditingExistingTotp}
            onImportTotpUri={handleImportTotpUri}
            hasPassword={!!selectedEntry?.hasPassword}
          />
        )}

        {category === "card" && (
          <CardFields
            username={username}
            onUsernameChange={setUsername}
            password={password}
            onPasswordChange={handlePasswordChange}
            hasPassword={!!selectedEntry?.hasPassword}
          />
        )}

        {category === "identity" && <IdentityFields />}
        {category === "sshKey" && (
          <SshKeyFields
            privateKey={password}
            onPrivateKeyChange={handlePasswordChange}
            hasPrivateKey={isEditing && !!selectedEntry?.hasPassword}
          />
        )}

        <NoteFields
          notes={notes}
          onNotesChange={handleNotesChange}
          hasNotes={!!selectedEntry?.hasNotes}
        />

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

        <CustomFields
          customFields={customFields}
          onCustomFieldsChange={setCustomFields}
        />
      </Form>
    </Drawer>
  );
}
