import { useEffect, useMemo, useState } from "react";
import { Input, Button, Typography, message } from "antd";
import { SearchOutlined, UndoOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useSettingsStore } from "@/stores/settingsStore";
import { SettingsGroup } from "../shared/SettingsGroup";
import { SettingsSection } from "../shared/SettingsSection";
import { SettingsPageShell } from "../shared/SettingsPageShell";
import { SHORTCUT_SETTINGS } from "../core/fieldDefinitions";
import {
  SHORTCUT_DEFINITIONS,
  SHORTCUT_GROUPS,
  DEFAULT_KEYBINDINGS,
  formatBinding,
} from "../core/shortcutTypes";

export const ShortcutSettings: React.FC = () => {
  const { t } = useTranslation();
  const loadAppConf = useSettingsStore((s) => s.loadAppConf);
  const resetKeybindings = useSettingsStore((s) => s.resetKeybindings);
  const appConf = useSettingsStore((s) => s.appConf);
  const [searchQuery, setSearchQuery] = useState("");
  const [messageApi, contextHolder] = message.useMessage();

  useEffect(() => {
    void loadAppConf();
  }, [loadAppConf]);

  const keybindings = appConf?.keybindings ?? DEFAULT_KEYBINDINGS;

  const filteredDefinitions = useMemo(() => {
    if (!searchQuery.trim()) return SHORTCUT_SETTINGS;
    const q = searchQuery.toLowerCase();
    return SHORTCUT_SETTINGS.filter((def) => {
      const shortcutId = def.key.replace("keybindings.", "");
      const shortcut = SHORTCUT_DEFINITIONS.find((s) => s.id === shortcutId);
      const currentBinding = shortcut
        ? formatBinding(keybindings[shortcut.id] ?? shortcut.defaultBinding)
        : "";
      return (
        def.label.toLowerCase().includes(q) ||
        def.key.toLowerCase().includes(q) ||
        currentBinding.toLowerCase().includes(q)
      );
    });
  }, [searchQuery, keybindings]);

  const handleReset = async () => {
    await resetKeybindings();
    messageApi.success(t("settings.shortcuts.reset_success"));
  };

  const groupedDefinitions = useMemo(() => {
    const groups: Record<string, typeof SHORTCUT_SETTINGS> = {};
    for (const def of filteredDefinitions) {
      const shortcutId = def.key.replace("keybindings.", "");
      const shortcut = SHORTCUT_DEFINITIONS.find((s) => s.id === shortcutId);
      const groupKey = shortcut?.group ?? "other";
      if (!groups[groupKey]) groups[groupKey] = [];
      groups[groupKey].push(def);
    }
    return groups;
  }, [filteredDefinitions]);

  return (
    <>
      {contextHolder}
      <SettingsPageShell>
        <SettingsSection
          id="shortcuts-main"
          style={{ marginTop: 0 }}
          extra={
            <Button icon={<UndoOutlined />} onClick={() => void handleReset()}>
              {t("settings.shortcuts.reset_default")}
            </Button>
          }
        >
          <Input
            placeholder={t("settings.shortcuts.search_placeholder")}
            prefix={<SearchOutlined />}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            style={{ marginBottom: 16 }}
            allowClear
          />

          {SHORTCUT_GROUPS.map((group) => {
            const groupDefs = groupedDefinitions[group.key];
            if (!groupDefs || groupDefs.length === 0) return null;

            return (
              <div key={group.key} style={{ marginBottom: 16 }}>
                <Typography.Text strong style={{ fontSize: 14 }}>
                  {t(group.label)}
                </Typography.Text>
                <div style={{ marginTop: 8 }}>
                  <SettingsGroup group="app" definitions={groupDefs} />
                </div>
              </div>
            );
          })}
        </SettingsSection>
      </SettingsPageShell>
    </>
  );
};
