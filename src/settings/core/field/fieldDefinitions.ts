import { defineSetting, defineSection } from "./fieldRegistry";
import { ThemeRadioRender } from "../../shared/renderers/ThemeRadioRender";
import { RendererSelectRender } from "../../shared/renderers/RendererSelectRender";
import { MascotColorGroupRender } from "../../shared/renderers/MascotColorGroupRender";
import { FolderPickerField } from "../../shared/renderers/FolderPickerField";
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
  id: "agent-main",
  pageId: "general",
  title: "settings.general.agent_tools.title",
  description: "settings.general.agent_tools.description",
  keywords: ["workspace", "工作区", "审批", "approval", "超时"],
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
    key: "launchAtLogin",
    group: "app",
    section: "general-main",
    type: "boolean",
    default: false,
    label: "settings.general.launch_at_login.label",
    saveMessage: "settings.general.launch_at_login.save_message",
  }),
  defineSetting({
    key: "closeToTray",
    group: "app",
    section: "general-main",
    type: "boolean",
    default: true,
    label: "settings.general.close_to_tray.label",
    extra: "settings.general.close_to_tray.extra",
  }),
  defineSetting({
    key: "stayOnTop",
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
    key: "opacityPercent",
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
    key: "idleOpacityPercent",
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
    key: "scalePercent",
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
    key: "clickThrough",
    group: "orb",
    section: "orb-ball",
    type: "boolean",
    default: false,
    label: "settings.orb.click_through.label",
    extra: "settings.orb.click_through.extra",
    saveMessage: false,
  }),
  defineSetting({
    key: "hideOnPointerHover",
    group: "orb",
    section: "orb-ball",
    type: "boolean",
    default: false,
    label: "settings.orb.hide_on_pointer_hover.label",
  }),
  defineSetting({
    key: "hoverDelayMs",
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
    key: "alwaysOnTop",
    group: "orb",
    section: "orb-panel",
    type: "boolean",
    default: true,
    label: "settings.orb.always_on_top.label",
  }),
];

export const MASCOT_COLOR_SETTING = defineSetting({
  key: "primaryColor",
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
    key: "autoCompact",
    group: "app",
    section: "general-main",
    type: "boolean",
    default: true,
    label: "settings.general.compact.auto_compact.label",
  }),
  defineSetting({
    key: "compactThresholdPercentOfBudget",
    group: "app",
    section: "general-main",
    type: "slider",
    default: COMPACTION_THRESHOLD_PERCENT,
    label: "settings.general.compact.compact_threshold_percent_of_budget.label",
    min: 50,
    max: 99,
    step: 1,
    marks: { 50: "50%", 80: "80%", 99: "99%" },
    visible: (conf: Record<string, unknown>) => !!conf.autoCompact,
  }),
  defineSetting({
    key: "compactSummaryPrompt",
    group: "app",
    section: "general-main",
    type: "textarea",
    default: null,
    label: "settings.general.compact.compact_summary_prompt.label",
    placeholder: "settings.general.compact.compact_summary_prompt.placeholder",
    rows: 6,
    visible: (conf: Record<string, unknown>) => !!conf.autoCompact,
  }),
  defineSetting({
    key: "compactSummaryMaxTokens",
    group: "app",
    section: "general-main",
    type: "number",
    default: MAX_SUMMARY_REQUEST_TOKENS,
    label: "settings.general.compact.compact_summary_max_tokens.label",
    min: 512,
    max: 16384,
    step: 256,
    visible: (conf: Record<string, unknown>) => !!conf.autoCompact,
  }),
];

export const AGENT_TOOL_SETTINGS = [
  defineSetting({
    key: "workspaceRoot",
    group: "app",
    section: "agent-main",
    type: "string",
    default: null,
    label: "settings.general.agent_tools.workspace_root.label",
    description: "settings.general.agent_tools.workspace_root.description",
    placeholder: "settings.general.agent_tools.workspace_root.placeholder",
    customRender: FolderPickerField,
    readValue: (conf: Record<string, unknown>) =>
      typeof conf.workspaceRoot === "string" ? conf.workspaceRoot : null,
    buildPatch: (value: unknown): Record<string, unknown> => ({
      workspaceRoot:
        typeof value === "string" && value.length > 0 ? value : null,
    }),
  }),
  defineSetting({
    key: "approvalTimeoutSecs",
    group: "app",
    section: "agent-main",
    type: "number",
    default: null,
    label: "settings.general.agent_tools.approval_timeout_secs.label",
    description:
      "settings.general.agent_tools.approval_timeout_secs.description",
    placeholder:
      "settings.general.agent_tools.approval_timeout_secs.placeholder",
    min: 5,
    max: 3600,
    step: 5,
    readValue: (conf: Record<string, unknown>) =>
      typeof conf.approvalTimeoutSecs === "number"
        ? conf.approvalTimeoutSecs
        : null,
    buildPatch: (value: unknown): Record<string, unknown> => ({
      approvalTimeoutSecs:
        typeof value === "number" && value > 0 ? value : null,
    }),
  }),
  defineSetting({
    key: "userQuestionTimeoutSecs",
    group: "app",
    section: "agent-main",
    type: "number",
    default: null,
    label: "settings.general.agent_tools.user_question_timeout_secs.label",
    description:
      "settings.general.agent_tools.user_question_timeout_secs.description",
    placeholder:
      "settings.general.agent_tools.user_question_timeout_secs.placeholder",
    min: 0,
    max: 3600,
    step: 5,
    readValue: (conf: Record<string, unknown>) =>
      typeof conf.userQuestionTimeoutSecs === "number"
        ? conf.userQuestionTimeoutSecs
        : null,
    buildPatch: (value: unknown): Record<string, unknown> => ({
      userQuestionTimeoutSecs:
        typeof value === "number" && value >= 0 ? value : null,
    }),
  }),
];

defineSetting({
  key: "app_name",
  group: "app",
  section: "about-main",
  type: "readonly",
  default: "",
  label: "settings.about.field.app_name.label",
  readValue: () => "Assistant",
});

defineSetting({
  key: "build_date",
  group: "app",
  section: "about-main",
  type: "readonly",
  default: "",
  label: "settings.about.field.build_date.label",
  readValue: () => __BUILD_DATE__,
});

defineSetting({
  key: "tech_stack",
  group: "app",
  section: "about-main",
  type: "readonly",
  default: "",
  label: "settings.about.field.tech_stack.label",
  readValue: () => "Tauri 2 + React 19 + Ant Design",
});

defineSetting({
  key: "license",
  group: "app",
  section: "about-main",
  type: "readonly",
  default: "",
  label: "settings.about.field.license.label",
  readValue: () => "MIT",
});
