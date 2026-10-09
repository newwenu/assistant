import { Button, Tooltip } from "antd";
import { DesktopOutlined, SunOutlined, MoonOutlined } from "@ant-design/icons";
import { useThemeStore } from "@/stores/themeStore";

const THEME_ICONS = {
  system: DesktopOutlined,
  light: SunOutlined,
  dark: MoonOutlined,
} as const;

const THEME_LABELS = {
  system: "跟随系统",
  light: "亮色模式",
  dark: "暗色模式",
} as const;

const NEXT_THEME = {
  system: "light" as const,
  light: "dark" as const,
  dark: "system" as const,
};

export const ThemeToggle: React.FC = () => {
  const mode = useThemeStore((s) => s.mode);
  const cycleMode = useThemeStore((s) => s.cycleMode);

  const Icon = THEME_ICONS[mode];
  const nextLabel = THEME_LABELS[NEXT_THEME[mode]];

  return (
    <Tooltip title={`切换到${nextLabel}`} mouseEnterDelay={0.5}>
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
