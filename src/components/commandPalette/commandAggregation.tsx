import React from "react";
import {
  SettingOutlined,
  SkinOutlined,
  ApiOutlined,
  InfoCircleOutlined,
  HistoryOutlined,
  ReloadOutlined,
  GlobalOutlined,
} from "@ant-design/icons";
import type { NavigateFunction } from "react-router-dom";
import { tools } from "@/tools/registry";
import {
  SETTINGS_PAGES,
  SETTINGS_CATEGORIES,
} from "@/settings/core/pageRegistry";
import {
  getAllDefinitions,
  getAllSections,
} from "@/settings/core/fieldRegistry";
import type { ConfGroup } from "@/settings/core/fieldRegistry";
import "@/settings/core/fieldDefinitions";
import { useSettingsStore } from "@/stores/settingsStore";
import {
  dispatchConfPatch,
  dispatchThemeChanged,
} from "@/settings/core/settingsDispatcher";
import { defineCommand, clearCommands } from "./commandRegistry";
import { translateKey } from "@/i18n";

function tokenize(text: string): string[] {
  return text
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .split(/[\s/_-]+/)
    .filter(Boolean)
    .map((t) => t.toLowerCase());
}

export function mergeKeywords(
  autoSources: string[],
  explicitKeywords: string[],
): string[] {
  const all = new Set<string>();
  for (const src of autoSources) {
    for (const token of tokenize(src)) {
      all.add(token);
    }
  }
  for (const kw of explicitKeywords) {
    all.add(kw.toLowerCase());
  }
  return [...all];
}

const CATEGORY_ICON_MAP: Record<string, React.ReactNode> = {
  general: <SettingOutlined />,
  provider: <ApiOutlined />,
  about: <InfoCircleOutlined />,
};

const GROUP_TO_PAGE: Record<ConfGroup, string> = {
  app: "general",
  orb: "appearance",
  mascot: "appearance",
};

function registerSettingsCommands(navigate: NavigateFunction): void {
  for (const page of SETTINGS_PAGES) {
    const catLabel =
      SETTINGS_CATEGORIES.find((c) => c.id === page.categoryId)?.label ?? "";
    const keywords = mergeKeywords(
      [page.label, page.id, `/settings/${page.id}`, catLabel],
      [...page.keywords],
    );
    defineCommand({
      id: `nav:settings-${page.id}`,
      name: page.label,
      description: catLabel
        ? translateKey("command.action.setting_page", { label: catLabel })
        : undefined,
      keywords,
      category: "navigation",
      icon: CATEGORY_ICON_MAP[page.categoryId] ?? <SettingOutlined />,
      action: () => navigate(`/settings/${page.id}`),
    });
  }
}

function registerSettingSectionCommands(navigate: NavigateFunction): void {
  const allSections = getAllSections();
  for (const sec of allSections) {
    const page = SETTINGS_PAGES.find((p) => p.id === sec.pageId);
    if (!page) continue;
    const autoSources: string[] = [sec.title, sec.id, sec.pageId];
    if (sec.description) autoSources.push(sec.description);
    const keywords = mergeKeywords(autoSources, [...(sec.keywords ?? [])]);
    defineCommand({
      id: `nav:setting-section:${sec.id}`,
      name: sec.title,
      description: `${page.label} · ${sec.title}`,
      keywords,
      category: "navigation",
      icon: CATEGORY_ICON_MAP[page.categoryId] ?? <SettingOutlined />,
      action: () => navigate(`/settings/${sec.pageId}?focus=section:${sec.id}`),
    });
  }
}

function registerSettingFieldCommands(navigate: NavigateFunction): void {
  const allDefs = getAllDefinitions();
  for (const def of allDefs) {
    const pageId = def.pageId ?? GROUP_TO_PAGE[def.group];
    if (!pageId) continue;
    const autoSources: string[] = [def.label, def.key, def.group];
    if (def.description && typeof def.description === "string")
      autoSources.push(def.description);
    if (def.extra && typeof def.extra === "string") autoSources.push(def.extra);
    if (def.options) {
      for (const opt of def.options) autoSources.push(opt.label);
    }
    const keywords = mergeKeywords(autoSources, []);
    defineCommand({
      id: `nav:setting-field:${def.key}`,
      name: def.label,
      description: translateKey("command.action.setting_item", {
        key: def.key,
      }),
      keywords,
      category: "navigation",
      action: () => navigate(`/settings/${pageId}?focus=${def.key}`),
    });
  }
}

function registerToolCommands(navigate: NavigateFunction): void {
  for (const tool of tools) {
    const keywords = mergeKeywords(
      [tool.name, tool.id, tool.category],
      [...tool.keywords],
    );
    const IconComp = tool.icon;
    defineCommand({
      id: `tool:${tool.id}`,
      name: tool.name,
      description: tool.description,
      keywords,
      category: "tools",
      icon: React.createElement(IconComp),
      action: () => navigate(`/tools/${tool.id}`),
    });
  }
}

function registerManualCommands(): void {
  defineCommand({
    id: "action:toggle-theme",
    name: translateKey("command.action.toggle_theme"),
    description: translateKey("command.action.toggle_theme_desc"),
    keywords: mergeKeywords(
      [translateKey("command.action.toggle_theme"), "toggle theme"],
      ["dark", "light", "system", "theme", "主题", "暗色", "亮色"],
    ),
    category: "actions",
    icon: <SkinOutlined />,
    closeOnSelect: false,
    action: async () => {
      const appConf = useSettingsStore.getState().appConf;
      if (!appConf) return;
      const order: Array<"system" | "light" | "dark"> = [
        "system",
        "light",
        "dark",
      ];
      const current = appConf.theme as "system" | "light" | "dark";
      const next = order[(order.indexOf(current) + 1) % order.length];
      await dispatchConfPatch("app", { theme: next });
      await dispatchThemeChanged(next);
    },
  });

  defineCommand({
    id: "action:random-tool",
    name: translateKey("command.action.random_tool"),
    description: translateKey("command.action.random_tool_desc"),
    keywords: mergeKeywords(
      [translateKey("command.action.random_tool"), "random tool"],
      ["random", "dice", "随机", "随便"],
    ),
    category: "actions",
    icon: <ReloadOutlined />,
    action: () => {
      const t = tools[Math.floor(Math.random() * tools.length)];
      window.location.hash = `#/tools/${t.id}`;
    },
  });

  defineCommand({
    id: "ext:github",
    name: translateKey("command.action.github_repo"),
    description: translateKey("command.action.github_repo_desc"),
    keywords: mergeKeywords(
      [translateKey("command.action.github_repo")],
      ["github", "repo", "source", "代码", "源码"],
    ),
    category: "external",
    icon: <GlobalOutlined />,
    action: () => {
      window.open("https://github.com", "_blank");
    },
  });

  defineCommand({
    id: "ext:bug-report",
    name: translateKey("command.action.bug_report"),
    description: translateKey("command.action.bug_report_desc"),
    keywords: mergeKeywords(
      [translateKey("command.action.bug_report")],
      ["bug", "issue", "report", "问题", "反馈", "建议"],
    ),
    category: "external",
    icon: <GlobalOutlined />,
    action: () => {
      window.open("https://github.com/issues/new/choose", "_blank");
    },
  });

  defineCommand({
    id: "nav:history",
    name: translateKey("command.action.chat_history"),
    description: translateKey("command.action.chat_history_desc"),
    keywords: mergeKeywords(
      [translateKey("command.action.chat_history")],
      ["history", "chat", "对话", "历史"],
    ),
    category: "navigation",
    icon: <HistoryOutlined />,
    action: () => {
      window.location.hash = "#/history";
    },
  });
}

export function aggregateCommands(navigate: NavigateFunction): void {
  clearCommands();
  registerSettingsCommands(navigate);
  registerSettingSectionCommands(navigate);
  registerSettingFieldCommands(navigate);
  registerToolCommands(navigate);
  registerManualCommands();
}
