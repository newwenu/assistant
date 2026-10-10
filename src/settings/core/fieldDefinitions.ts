import { defineSetting } from "./fieldRegistry";

export const APP_SETTINGS = [
  defineSetting({
    key: "language",
    group: "app",
    type: "select",
    default: "zh-CN",
    label: "语言",
    options: [{ label: "简体中文", value: "zh-CN" }],
  }),
  defineSetting({
    key: "launch_at_login",
    group: "app",
    type: "boolean",
    default: false,
    label: "开机自启",
    sideEffect: "autostart",
    saveMessage: "开机自启已更新",
  }),
  defineSetting({
    key: "close_to_tray",
    group: "app",
    type: "boolean",
    default: true,
    label: "关闭到托盘",
    extra: "关闭主窗口时最小化到托盘而非退出",
  }),
  defineSetting({
    key: "stay_on_top",
    group: "app",
    type: "boolean",
    default: false,
    label: "置顶主窗口",
    sideEffect: "always_on_top",
    saveMessage: "置顶状态已更新",
  }),
];

export const ORB_BALL_SETTINGS = [
  defineSetting({
    key: "opacity_percent",
    group: "orb",
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
    type: "boolean",
    default: false,
    label: "悬停隐藏",
  }),
  defineSetting({
    key: "hover_delay_ms",
    group: "orb",
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
    type: "boolean",
    default: true,
    label: "始终置顶",
    sideEffect: "orb_always_on_top",
  }),
];

export const MASCOT_SETTINGS = [
  defineSetting({
    key: "primary_color",
    group: "mascot",
    type: "color",
    default: null,
    label: "主色",
  }),
  defineSetting({
    key: "secondary_color",
    group: "mascot",
    type: "color",
    default: null,
    label: "辅色",
  }),
];
