import { useState } from "react";
import {
  Form,
  Input,
  Space,
  Select,
  InputNumber,
  Switch,
  Button,
  Typography,
} from "antd";
import { LinkOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import {
  generatePassword,
  generatePassphrase,
} from "@/tools/password-generator/password-generator.service";
import styles from "../vault.module.css";

type PasswordGenMode = "random" | "passphrase";

interface LoginFieldsProps {
  username: string;
  onUsernameChange: (value: string) => void;
  password: string;
  onPasswordChange: (value: string) => void;
  url: string;
  onUrlChange: (value: string) => void;
  totpSecret: string;
  onTotpSecretChange: (value: string) => void;
  totpUriInput: string;
  onTotpUriInputChange: (value: string) => void;
  totpUriVisible: boolean;
  onTotpUriVisibleChange: (visible: boolean) => void;
  totpSecretVisible: boolean;
  onTotpSecretVisibleChange: (visible: boolean) => void;
  isEditingExistingTotp: boolean;
  onImportTotpUri: () => Promise<void>;
  hasPassword?: boolean;
}

export default function LoginFields({
  username,
  onUsernameChange,
  password,
  onPasswordChange,
  url,
  onUrlChange,
  totpSecret,
  onTotpSecretChange,
  totpUriInput,
  onTotpUriInputChange,
  totpUriVisible,
  onTotpUriVisibleChange,
  totpSecretVisible,
  onTotpSecretVisibleChange,
  isEditingExistingTotp,
  onImportTotpUri,
  hasPassword = false,
}: LoginFieldsProps) {
  const { t } = useTranslation();
  const [genMode, setGenMode] = useState<PasswordGenMode>("random");
  const [genLength, setGenLength] = useState(16);
  const [genWordCount, setGenWordCount] = useState(4);
  const [genSeparator, setGenSeparator] = useState("-");
  const [genCapitalize, setGenCapitalize] = useState(false);

  return (
    <>
      <Form.Item label={t("vault.entry.username")}>
        <Input
          value={username}
          onChange={(e) => onUsernameChange(e.target.value)}
        />
      </Form.Item>

      <Form.Item label={t("vault.entry.password")}>
        <Space orientation="vertical" style={{ width: "100%" }}>
          <Input.Password
            value={password}
            onChange={(e) => onPasswordChange(e.target.value)}
            placeholder={
              hasPassword && !password
                ? t("vault.entry.password_set_hint")
                : undefined
            }
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
                  onPasswordChange(
                    generatePassword(genLength, true, true, true, true),
                  );
                } else {
                  onPasswordChange(
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

      <Form.Item label={t("vault.entry.url")}>
        <Input value={url} onChange={(e) => onUrlChange(e.target.value)} />
      </Form.Item>

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
                    onTotpSecretChange("");
                    onTotpUriVisibleChange(true);
                  }}
                >
                  {t("vault.totp.clear")}
                </Button>
                <Button
                  type="link"
                  size="small"
                  onClick={() => onTotpSecretVisibleChange(true)}
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
                onChange={(e) => onTotpUriInputChange(e.target.value)}
                placeholder="otpauth://totp/..."
                onPressEnter={() => void onImportTotpUri()}
                prefix={<LinkOutlined />}
                className={styles.totpUriInput}
              />
              <Space size={8} style={{ marginTop: 8 }}>
                <Button
                  type="primary"
                  size="small"
                  onClick={() => void onImportTotpUri()}
                >
                  {t("vault.totp.import")}
                </Button>
                <Button
                  type="link"
                  size="small"
                  onClick={() => {
                    onTotpUriVisibleChange(false);
                    onTotpUriInputChange("");
                    onTotpSecretVisibleChange(true);
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
                onChange={(e) => onTotpSecretChange(e.target.value)}
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
                    onTotpSecretVisibleChange(false);
                    onTotpUriVisibleChange(true);
                  }}
                >
                  {t("vault.totp.import_uri")}
                </Button>
              </Space>
            </div>
          )}
        </Space>
      </Form.Item>
    </>
  );
}
