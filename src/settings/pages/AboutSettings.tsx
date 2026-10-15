import { useEffect, useMemo, useState } from "react";
import { Descriptions, Button, Typography } from "antd";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import type { FieldSchema } from "../core/types/fieldSchema";
import { SettingsSection } from "../shared/layout/SettingsSection";
import { SettingsPageShell } from "../shared/layout/SettingsPageShell";

export const AboutSettings: React.FC = () => {
  const { t } = useTranslation();
  const [version, setVersion] = useState("");
  const [rustcVersion, setRustcVersion] = useState("");

  const fieldSchemas = useMemo<FieldSchema[]>(
    () => [
      {
        key: "app_name",
        type: "readonly",
        label: t("settings.about.field.app_name.label"),
      },
      {
        key: "version",
        type: "readonly",
        label: t("settings.about.field.version.label"),
        copyable: true,
      },
      {
        key: "build_date",
        type: "readonly",
        label: t("settings.about.field.build_date.label"),
      },
      {
        key: "rust_toolchain",
        type: "readonly",
        label: t("settings.about.field.rust_toolchain.label"),
        copyable: true,
      },
      {
        key: "tech_stack",
        type: "readonly",
        label: t("settings.about.field.tech_stack.label"),
      },
      {
        key: "license",
        type: "readonly",
        label: t("settings.about.field.license.label"),
      },
    ],
    [t],
  );

  const fieldValues = useMemo(
    () => ({
      app_name: t("settings.about.app_name_value"),
      version: version || "-",
      build_date: __BUILD_DATE__,
      rust_toolchain: rustcVersion || "-",
      tech_stack: t("settings.about.tech_stack_value"),
      license: t("settings.about.license_value"),
    }),
    [t, version, rustcVersion],
  );

  useEffect(() => {
    invoke<string>("get_app_version")
      .then((v) => setVersion(v))
      .catch(() => setVersion("-"));
    invoke<string>("get_rustc_version")
      .then((v) => setRustcVersion(v))
      .catch(() => setRustcVersion("-"));
  }, []);

  return (
    <SettingsPageShell>
      <SettingsSection id="about-main" style={{ marginTop: 0 }}>
        <Descriptions column={1} bordered size="small">
          {fieldSchemas.map((schema) => (
            <Descriptions.Item key={schema.key} label={schema.label}>
              {schema.copyable ? (
                <Typography.Text copyable>
                  {fieldValues[schema.key as keyof typeof fieldValues]}
                </Typography.Text>
              ) : (
                fieldValues[schema.key as keyof typeof fieldValues]
              )}
            </Descriptions.Item>
          ))}
        </Descriptions>
        <div style={{ marginTop: 16, textAlign: "center" }}>
          <Button type="default" disabled>
            {t("settings.about.check_update")}
          </Button>
        </div>
      </SettingsSection>
    </SettingsPageShell>
  );
};
