import { defineSetting, defineSection } from "./fieldRegistry";
import { ThemeRadioRender } from "../../shared/renderers/ThemeRadioRender";
import { RendererSelectRender } from "../../shared/renderers/RendererSelectRender";
import { MascotColorGroupRender } from "../../shared/renderers/MascotColorGroupRender";
import {
  SHORTCUT_DEFINITIONS,
  DEFAULT_KEYBINDINGS,
} from "../types/shortcutTypes";
import { createMascotCapabilityVisible } from "@/mascot";
import {
  COMPACTION_THRESHOLD_PERCENT,
  MAX_SUMMARY_REQUEST_TOKENS,
} from "@/config/constants";

defineSection({
  id: "general-main",
  pageId: "general",
  title: "settings.general.main.title",
});

defineSection({
  id: "theme",
  pageId: "appearance",
  title: "settings.appearance.theme.title",
  keywords: ["dark", "light", "system", "暗色", "亮色"],
});

defineSection({
  id: "orb-ball",
  pageId: "appearance",
  title: "settings.appearance.orb_ball.title",
  description: "settings.appearance.orb_ball.description",
  keywords: ["orb", "ball", "透明度", "缩放", "吉祥物", "助手形象"],
});

defineSection({
  id: "orb-panel",
  pageId: "appearance",
  title: "settings.appearance.orb_panel.title",
  keywords: ["panel", "置顶"],
});

defineSection({
  id: "shortcuts-main",
  pageId: "shortcuts",
  title: "settings.shortcuts.main.title",
  keywords: ["keyboard", "hotkey", "热键"],
});

defineSection({
  id: "about-main",
  pageId: "about",
  title: "settings.about.main.title",
  keywords: ["version", "版本"],
});

export const APP_SETTINGS = [
  defineSetting({
    key: "language",
    group: "app",
    section: "general-main",
    type: "select",
    default: "zh-CN",
    label: "settings.general.language.label",
    options: [
      { label: "简体中文", value: "zh-CN" },
      { label: "English", value: "en-US" },
    ],
    sideEffect: "locale_change",
  }),
  defineSetting({
    key: "launch_at_login",
    group: "app",
    section: "general-main",
    type: "boolean",
    default: false,
    label: "settings.general.launch_at_login.label",
    saveMessage: "settings.general.launch_at_login.save_message",
  }),
  defineSetting({
    key: "close_to_tray",
    group: "app",
    section: "general-main",
    type: "boolean",
    default: true,
    label: "settings.general.close_to_tray.label",
    extra: "settings.general.close_to_tray.extra",
  }),
  defineSetting({
    key: "stay_on_top",
    group: "app",
    section: "general-main",
    type: "boolean",
    default: false,
    label: "settings.general.stay_on_top.label",
    saveMessage: "settings.general.stay_on_top.save_message",
  }),
];

export const THEME_SETTING = defineSetting({
  key: "theme",
  group: "app",
  section: "theme",
  type: "select",
  default: "system",
  label: "settings.appearance.theme.label",
  options: [
    { label: "settings.appearance.theme.options.system", value: "system" },
    { label: "settings.appearance.theme.options.light", value: "light" },
    { label: "settings.appearance.theme.options.dark", value: "dark" },
  ],
  sideEffect: "theme_change",
  saveMessage: "settings.appearance.theme.save_message",
  customRender: ThemeRadioRender,
});

export const ORB_BALL_SETTINGS = [
  defineSetting({
    key: "opacity_percent",
    group: "orb",
    section: "orb-ball",
    type: "slider",
    default: 100,
    label: "settings.orb.opacity_percent.label",
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
    label: "settings.orb.idle_opacity_percent.label",
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
    label: "settings.orb.scale_percent.label",
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
    label: "settings.orb.click_through.label",
    extra: "settings.orb.click_through.extra",
    saveMessage: false,
  }),
  defineSetting({
    key: "hide_on_pointer_hover",
    group: "orb",
    section: "orb-ball",
    type: "boolean",
    default: false,
    label: "settings.orb.hide_on_pointer_hover.label",
  }),
  defineSetting({
    key: "hover_delay_ms",
    group: "orb",
    section: "orb-ball",
    type: "slider",
    default: 0,
    label: "settings.orb.hover_delay_ms.label",
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
    label: "settings.orb.always_on_top.label",
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
  label: "settings.mascot.renderer.label",
  description: "settings.mascot.renderer.description",
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
    saveMessage: "settings.shortcuts.save_message",
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

export const COMPACT_SETTINGS = [
  defineSetting({
    key: "auto_compact",
    group: "app",
    section: "general-main",
    type: "boolean",
    default: true,
    label: "settings.general.compact.auto_compact.label",
  }),
  defineSetting({
    key: "compact_threshold_percent",
    group: "app",
    section: "general-main",
    type: "slider",
    default: COMPACTION_THRESHOLD_PERCENT,
    label: "settings.general.compact.compact_threshold_percent.label",
    min: 50,
    max: 99,
    step: 1,
    marks: { 50: "50%", 80: "80%", 99: "99%" },
    visible: (conf: Record<string, unknown>) => !!conf.auto_compact,
  }),
  defineSetting({
    key: "compact_summary_prompt",
    group: "app",
    section: "general-main",
    type: "textarea",
    default: null,
    label: "settings.general.compact.compact_summary_prompt.label",
    placeholder: "settings.general.compact.compact_summary_prompt.placeholder",
    rows: 6,
    visible: (conf: Record<string, unknown>) => !!conf.auto_compact,
  }),
  defineSetting({
    key: "compact_summary_max_tokens",
    group: "app",
    section: "general-main",
    type: "number",
    default: MAX_SUMMARY_REQUEST_TOKENS,
    label: "settings.general.compact.compact_summary_max_tokens.label",
    min: 512,
    max: 16384,
    step: 256,
    visible: (conf: Record<string, unknown>) => !!conf.auto_compact,
  }),
];
