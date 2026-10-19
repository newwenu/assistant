import { Button, Tooltip } from "antd";
import { DesktopOutlined, SunOutlined, MoonOutlined } from "@ant-design/icons";
import { useThemeStore } from "@/stores/themeStore";
import { useTranslation } from "react-i18next";

const THEME_ICONS = {
  system: DesktopOutlined,
  light: SunOutlined,
  dark: MoonOutlined,
} as const;

const THEME_KEYS = {
  system: "common.theme.system",
  light: "common.theme.light",
  dark: "common.theme.dark",
} as const;

const NEXT_THEME = {
  system: "light" as const,
  light: "dark" as const,
  dark: "system" as const,
};

export const ThemeToggle: React.FC = () => {
  const mode = useThemeStore((s) => s.mode);
  const cycleMode = useThemeStore((s) => s.cycleMode);
  const { t } = useTranslation();

  const Icon = THEME_ICONS[mode];
  const nextLabel = t(THEME_KEYS[NEXT_THEME[mode]]);

  return (
    <Tooltip
      title={t("common.theme.switch_to", { label: nextLabel })}
      mouseEnterDelay={0.5}
    >
      <Button
        type="text"
        size="small"
        icon={<Icon />}
        onClick={cycleMode}
        style={{ width: 32, height: 28 }}
      />
    </Tooltip>
  );
};
