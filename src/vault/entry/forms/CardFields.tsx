import { useState } from "react";
import { Form, Input, Space, Select, InputNumber, Switch, Button } from "antd";
import { useTranslation } from "react-i18next";
import {
  generatePassword,
  generatePassphrase,
} from "@/tools/password-generator/password-generator.service";

type PasswordGenMode = "random" | "passphrase";

interface CardFieldsProps {
  username: string;
  onUsernameChange: (value: string) => void;
  password: string;
  onPasswordChange: (value: string) => void;
  hasPassword?: boolean;
}

export default function CardFields({
  username,
  onUsernameChange,
  password,
  onPasswordChange,
  hasPassword = false,
}: CardFieldsProps) {
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
    </>
  );
}
