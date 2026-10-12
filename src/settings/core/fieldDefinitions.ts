import { defineSetting, defineSection } from "./fieldRegistry";
import { ThemeRadioRender } from "../shared/ThemeRadioRender";
import { RendererSelectRender } from "../shared/RendererSelectRender";
import { MascotColorGroupRender } from "../shared/MascotColorGroupRender";
import { SHORTCUT_DEFINITIONS, DEFAULT_KEYBINDINGS } from "./shortcutTypes";
import { createMascotCapabilityVisible } from "@/mascot";

defineSection({ id: "general-main", pageId: "general", title: "通用设置" });

defineSection({
  id: "theme",
  pageId: "appearance",
  title: "主题",
  keywords: ["dark", "light", "system", "暗色", "亮色"],
});

defineSection({
  id: "orb-ball",
  pageId: "appearance",
  title: "悬浮球",
  description: "助手形象渲染、颜色，以及透明度、缩放等",
  keywords: ["orb", "ball", "透明度", "缩放", "吉祥物", "助手形象"],
});

defineSection({
  id: "orb-panel",
  pageId: "appearance",
  title: "悬浮窗",
  keywords: ["panel", "置顶"],
});

defineSection({
  id: "shortcuts-main",
  pageId: "shortcuts",
  title: "快捷键",
  keywords: ["keyboard", "hotkey", "热键"],
});

defineSection({
  id: "about-main",
  pageId: "about",
  title: "关于",
  keywords: ["version", "版本"],
});

export const APP_SETTINGS = [
  defineSetting({
    key: "language",
    group: "app",
    section: "general-main",
    type: "select",
    default: "zh-CN",
    label: "语言",
    options: [{ label: "简体中文", value: "zh-CN" }],
  }),
  defineSetting({
    key: "launch_at_login",
    group: "app",
    section: "general-main",
    type: "boolean",
    default: false,
    label: "开机自启",
    sideEffect: "autostart",
    saveMessage: "开机自启已更新",
  }),
  defineSetting({
    key: "close_to_tray",
    group: "app",
    section: "general-main",
    type: "boolean",
    default: true,
    label: "关闭到托盘",
    extra: "关闭主窗口时最小化到托盘而非退出",
  }),
  defineSetting({
    key: "stay_on_top",
    group: "app",
    section: "general-main",
    type: "boolean",
    default: false,
    label: "置顶主窗口",
    sideEffect: "always_on_top",
    saveMessage: "置顶状态已更新",
  }),
];

export const THEME_SETTING = defineSetting({
  key: "theme",
  group: "app",
  section: "theme",
  type: "select",
  default: "system",
  label: "主题模式",
  options: [
    { label: "跟随系统", value: "system" },
    { label: "亮色", value: "light" },
    { label: "暗色", value: "dark" },
  ],
  sideEffect: "theme_change",
  saveMessage: "主题已切换",
  customRender: ThemeRadioRender,
});

export const ORB_BALL_SETTINGS = [
  defineSetting({
    key: "opacity_percent",
    group: "orb",
    section: "orb-ball",
    type: "slider",
    default: 100,
    label: "悬停透明度",
    min: 10,
    max: 100,
    marks: { 10: "10%", 100: "100%" },
  }),
  defineSetting({
    key: "idle_opacity_percent",
    group: "orb",
    section: "orb-ball",
    type: "slider",
    default: 85,
    label: "空闲透明度",
    min: 10,
    max: 100,
    marks: { 10: "10%", 85: "85%", 100: "100%" },
  }),
  defineSetting({
    key: "scale_percent",
    group: "orb",
    section: "orb-ball",
    type: "slider",
    default: 100,
    label: "缩放",
    min: 50,
    max: 200,
    marks: { 50: "50%", 100: "100%", 200: "200%" },
  }),
  defineSetting({
    key: "click_through",
    group: "orb",
    section: "orb-ball",
    type: "boolean",
    default: false,
    label: "鼠标穿透",
    extra: "开启后鼠标可穿透悬浮球点击下方内容",
    sideEffect: "orb_click_through",
    saveMessage: false,
  }),
  defineSetting({
    key: "hide_on_pointer_hover",
    group: "orb",
    section: "orb-ball",
    type: "boolean",
    default: false,
    label: "悬停隐藏",
  }),
  defineSetting({
    key: "hover_delay_ms",
    group: "orb",
    section: "orb-ball",
    type: "slider",
    default: 0,
    label: "悬停延迟 (ms)",
    min: 0,
    max: 5000,
    step: 50,
    marks: { 0: "0", 300: "300", 1000: "1s" },
  }),
];

export const ORB_PANEL_SETTINGS = [
  defineSetting({
    key: "always_on_top",
    group: "orb",
    section: "orb-panel",
    type: "boolean",
    default: true,
    label: "始终置顶",
    sideEffect: "orb_always_on_top",
  }),
];

export const MASCOT_COLOR_SETTING = defineSetting({
  key: "primary_color",
  group: "mascot",
  section: "orb-ball",
  type: "color",
  default: null,
  label: "",
  customRender: MascotColorGroupRender,
  visible: createMascotCapabilityVisible(
    "colorCustomization",
    "hasDefaultColors",
  ),
});

export const MASCOT_RENDERER_SETTING = defineSetting({
  key: "renderer",
  group: "mascot",
  section: "orb-ball",
  type: "select",
  default: "rive",
  label: "渲染方式",
  description: "选择吉祥物的渲染方式",
  customRender: RendererSelectRender,
});

export const SHORTCUT_SETTINGS = SHORTCUT_DEFINITIONS.map((shortcut) =>
  defineSetting({
    key: `keybindings.${shortcut.id}`,
    group: "app",
    section: "shortcuts-main",
    type: "keybinding",
    default: shortcut.defaultBinding,
    label: shortcut.label,
    pageId: "shortcuts",
    saveMessage: "快捷键已更新",
    readValue: (conf: Record<string, unknown>) =>
      (conf.keybindings as Record<string, string>)?.[shortcut.id] ??
      DEFAULT_KEYBINDINGS[shortcut.id],
    buildPatch: (
      value: unknown,
      conf: Record<string, unknown>,
    ): Record<string, unknown> => ({
      keybindings: {
        ...(conf.keybindings as Record<string, string>),
        [shortcut.id]: value,
      },
    }),
  }),
);
