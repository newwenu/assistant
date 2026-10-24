import { useEffect } from "react";
import { Form, Button, Typography, Spin } from "antd";
import { SafetyCertificateOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { useSettingsStore } from "@/stores/settingsStore";
import { useVaultStore } from "@/vault/core/vaultStore";
import { useVaultStatusReady } from "@/vault/core/useVaultStatusReady";
import { SettingsGroup } from "../shared/sections/SettingsGroup";
import { SettingsSection } from "../shared/layout/SettingsSection";
import { SettingsPageShell } from "../shared/layout/SettingsPageShell";
import {
  VAULT_GENERAL_SETTINGS,
  VAULT_SSH_SETTINGS,
} from "../core/field/fieldDefinitions";
import { useConfPatchHandlers } from "../core/useConfPatchHandlers";

export const VaultSettingsPage: React.FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const loadAppConf = useSettingsStore((s) => s.loadAppConf);
  const isInitialized = useVaultStore((s) => s.isInitialized);
  const statusReady = useVaultStatusReady();
  const { onPatch, onPatchComplete } = useConfPatchHandlers("app");

  useEffect(() => {
    void loadAppConf();
  }, [loadAppConf]);

  if (!statusReady) {
    return (
      <SettingsPageShell>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            minHeight: 240,
          }}
        >
          <Spin />
        </div>
      </SettingsPageShell>
    );
  }

  if (!isInitialized) {
    return (
      <SettingsPageShell>
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            minHeight: 240,
            textAlign: "center",
          }}
        >
          <SafetyCertificateOutlined
            style={{
              fontSize: 48,
              color: "var(--ant-color-text-quaternary)",
              marginBottom: 16,
            }}
          />
          <Typography.Title level={5} style={{ margin: 0 }}>
            {t("settings.vault.not_initialized.title")}
          </Typography.Title>
          <Typography.Text
            type="secondary"
            style={{ marginTop: 8, maxWidth: 360 }}
          >
            {t("settings.vault.not_initialized.description")}
          </Typography.Text>
          <Button
            type="primary"
            style={{ marginTop: 16 }}
            onClick={() => void navigate("/vault")}
          >
            {t("settings.vault.not_initialized.go_create")}
          </Button>
        </div>
      </SettingsPageShell>
    );
  }

  return (
    <SettingsPageShell>
      <SettingsSection id="vault-general" style={{ marginTop: 0 }}>
        <Form layout="vertical">
          <SettingsGroup
            group="app"
            definitions={VAULT_GENERAL_SETTINGS}
            onPatch={onPatch}
            onPatchComplete={onPatchComplete}
          />
        </Form>
      </SettingsSection>

      <SettingsSection id="vault-ssh">
        <Form layout="vertical">
          <SettingsGroup
            group="app"
            definitions={VAULT_SSH_SETTINGS}
            onPatch={onPatch}
            onPatchComplete={onPatchComplete}
          />
        </Form>
      </SettingsSection>
    </SettingsPageShell>
  );
};
