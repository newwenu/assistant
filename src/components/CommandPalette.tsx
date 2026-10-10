import React, { useEffect, useRef, useState, useCallback } from "react";
import { Modal, Input, Typography, Divider } from "antd";
import {
  SearchOutlined,
  ThunderboltOutlined,
  AppstoreOutlined,
  BulbOutlined,
  GlobalOutlined,
  HistoryOutlined,
  SettingOutlined,
  SkinOutlined,
  KeyOutlined,
  ApiOutlined,
  InfoCircleOutlined,
  ReloadOutlined,
} from "@ant-design/icons";
import { useNavigate } from "react-router-dom";
import Fuse from "fuse.js";
import { tools } from "@/tools/registry";
import { COMMAND_PALETTE_MAX_PER_CATEGORY } from "@/config/constants";
import { useSettingsStore } from "@/stores/settingsStore";
import {
  dispatchConfPatch,
  dispatchThemeChanged,
} from "@/settings/core/settingsDispatcher";
import styles from "./CommandPalette.module.css";

interface PaletteOption {
  id: string;
  name: string;
  description?: string;
  keywords?: string[];
  category: string;
  icon?: React.ReactNode;
  action: () => void;
  closeOnSelect?: boolean;
}

const IS_MAC = /\bMacintosh\b/i.test(navigator.userAgent);

const CATEGORY_ICONS: Record<string, React.ReactNode> = {
  Tools: <ThunderboltOutlined />,
  Navigation: <AppstoreOutlined />,
  Actions: <BulbOutlined />,
  External: <GlobalOutlined />,
};

const CATEGORY_ORDER = ["Tools", "Navigation", "Actions", "External"];

export const CommandPalette: React.FC = () => {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const navigate = useNavigate();
  const appConf = useSettingsStore((s) => s.appConf);

  const allOptions: PaletteOption[] = React.useMemo(() => {
    const toolOpts: PaletteOption[] = tools.map((t) => ({
      id: `tool:${t.id}`,
      name: t.name,
      description: t.description,
      keywords: t.keywords,
      category: "Tools",
      action: () => navigate(`/tools/${t.id}`),
    }));

    const navOpts: PaletteOption[] = [
      {
        id: "nav:history",
        name: "消息历史",
        description: "查看对话历史记录",
        keywords: ["history", "chat", "对话", "历史"],
        category: "Navigation",
        icon: <HistoryOutlined />,
        action: () => navigate("/history"),
      },
      {
        id: "nav:settings-general",
        name: "通用设置",
        description: "语言、开机自启、关闭到托盘",
        keywords: ["general", "language", "语言", "autostart", "tray"],
        category: "Navigation",
        icon: <SettingOutlined />,
        action: () => navigate("/settings/general"),
      },
      {
        id: "nav:settings-appearance",
        name: "外观设置",
        description: "主题、Orb 外观配置",
        keywords: ["appearance", "theme", "dark", "light", "外观", "主题"],
        category: "Navigation",
        icon: <SkinOutlined />,
        action: () => navigate("/settings/appearance"),
      },
      {
        id: "nav:settings-shortcuts",
        name: "快捷键",
        description: "全局与应用内快捷键配置",
        keywords: ["shortcuts", "keybindings", "快捷键", "键绑定"],
        category: "Navigation",
        icon: <KeyOutlined />,
        action: () => navigate("/settings/shortcuts"),
      },
      {
        id: "nav:settings-providers",
        name: "Provider 管理",
        description: "管理 AI Provider 配置",
        keywords: ["provider", "api", "模型", "provider"],
        category: "Navigation",
        icon: <ApiOutlined />,
        action: () => navigate("/settings/providers"),
      },
      {
        id: "nav:settings-about",
        name: "关于",
        description: "版本信息与更新",
        keywords: ["about", "version", "版本", "更新"],
        category: "Navigation",
        icon: <InfoCircleOutlined />,
        action: () => navigate("/settings/about"),
      },
    ];

    const actionOpts: PaletteOption[] = [
      {
        id: "action:random-tool",
        name: "随机工具",
        description: "随机打开一个工具",
        keywords: ["random", "dice", "随机", "随便"],
        category: "Actions",
        icon: <ReloadOutlined />,
        action: () => {
          const t = tools[Math.floor(Math.random() * tools.length)];
          navigate(`/tools/${t.id}`);
        },
      },
      {
        id: "action:toggle-theme",
        name: "切换主题",
        description: "循环切换 亮色 → 暗色 → 跟随系统",
        keywords: ["dark", "light", "system", "theme", "主题", "暗色", "亮色"],
        category: "Actions",
        icon: <SkinOutlined />,
        closeOnSelect: false,
        action: async () => {
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
      },
    ];

    const externalOpts: PaletteOption[] = [
      {
        id: "ext:github",
        name: "GitHub 仓库",
        description: "查看源代码",
        keywords: ["github", "repo", "source", "代码", "源码"],
        category: "External",
        icon: <GlobalOutlined />,
        action: () => window.open("https://github.com", "_blank"),
      },
      {
        id: "ext:bug-report",
        name: "报告问题",
        description: "提交 Bug 或功能建议",
        keywords: ["bug", "issue", "report", "问题", "反馈", "建议"],
        category: "External",
        icon: <GlobalOutlined />,
        action: () =>
          window.open("https://github.com/issues/new/choose", "_blank"),
      },
    ];

    return [...toolOpts, ...navOpts, ...actionOpts, ...externalOpts];
  }, [navigate, appConf]);

  const fuse = React.useMemo(
    () =>
      new Fuse(allOptions, {
        keys: [
          { name: "name", weight: 2 },
          { name: "description", weight: 1 },
          { name: "keywords", weight: 1.5 },
        ],
        threshold: 0.3,
      }),
    [allOptions],
  );

  const filteredResults = React.useMemo(() => {
    if (!query.trim()) return allOptions;
    return fuse.search(query).map((r) => r.item);
  }, [query, allOptions, fuse]);

  const groupedResults = React.useMemo(() => {
    const groups: Record<string, PaletteOption[]> = {};
    for (const opt of filteredResults) {
      if (!groups[opt.category]) groups[opt.category] = [];
      if (groups[opt.category].length < COMMAND_PALETTE_MAX_PER_CATEGORY) {
        groups[opt.category].push(opt);
      }
    }
    return groups;
  }, [filteredResults]);

  const sortedGroups = React.useMemo(
    () =>
      Object.entries(groupedResults).sort(
        ([a], [b]) =>
          (CATEGORY_ORDER.indexOf(a) ?? 99) - (CATEGORY_ORDER.indexOf(b) ?? 99),
      ),
    [groupedResults],
  );

  const flatResults = React.useMemo(
    () => sortedGroups.flatMap(([, opts]) => opts),
    [sortedGroups],
  );

  const close = useCallback(() => {
    setOpen(false);
    setQuery("");
    setSelectedIndex(0);
  }, []);

  const activate = useCallback(
    (opt: PaletteOption) => {
      opt.action();
      if (opt.closeOnSelect !== false) close();
    },
    [close],
  );

  useEffect(() => {
    const binding = appConf?.keybindings?.command_palette ?? "Meta+K";

    const requireMeta =
      /\bmeta\b/i.test(binding) ||
      /\bsuper\b/i.test(binding) ||
      /\bcmd\b/i.test(binding);
    const requireCtrl = /\bctrl\b/i.test(binding) && !requireMeta;
    const requireShift = /\bshift\b/i.test(binding);
    const requireAlt = /\balt\b/i.test(binding);
    const keyMatch = binding.match(/\b([A-Z0-9])\s*$/i);
    const key = keyMatch ? keyMatch[1].toLowerCase() : "k";

    const handler = (e: KeyboardEvent) => {
      if (
        e.target instanceof HTMLInputElement &&
        (e.target as HTMLInputElement).isContentEditable
      )
        return;
      const metaOk = requireMeta
        ? IS_MAC
          ? e.metaKey
          : e.ctrlKey
        : !e.metaKey && !e.ctrlKey;
      const ctrlOk = requireCtrl ? e.ctrlKey : true;
      const shiftOk = requireShift ? e.shiftKey : !e.shiftKey;
      const altOk = requireAlt ? e.altKey : !e.altKey;

      if (metaOk && ctrlOk && shiftOk && altOk && e.key.toLowerCase() === key) {
        e.preventDefault();
        setOpen((v) => !v);
        setQuery("");
        setSelectedIndex(0);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [appConf?.keybindings?.command_palette]);

  useEffect(() => {
    if (open) {
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [open]);

  useEffect(() => {
    if (!open || flatResults.length === 0) return;
    const el = listRef.current?.children[selectedIndex] as
      HTMLElement | undefined;
    el?.scrollIntoView({ block: "nearest" });
  }, [open, selectedIndex, flatResults.length]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((i) => Math.min(i + 1, flatResults.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const opt = flatResults[selectedIndex];
      if (opt) activate(opt);
    } else if (e.key === "Escape") {
      close();
    }
  };

  return (
    <Modal
      open={open}
      onCancel={close}
      footer={null}
      closable={false}
      width={520}
      styles={{ body: { padding: 0 } }}
    >
      <div className={styles.inputWrapper}>
        <Input
          ref={inputRef as any}
          prefix={<SearchOutlined />}
          placeholder="搜索工具、页面、操作..."
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setSelectedIndex(0);
          }}
          onKeyDown={handleKeyDown}
          size="large"
          variant="borderless"
        />
      </div>
      <div className={styles.resultList} ref={listRef}>
        {sortedGroups.map(([cat, opts]) => (
          <div key={cat}>
            <Divider className={styles.categoryDivider}>
              <Typography.Text
                type="secondary"
                className={styles.categoryLabel}
              >
                {CATEGORY_ICONS[cat]} {cat}
              </Typography.Text>
            </Divider>
            {opts.map((opt) => {
              const flatIdx = flatResults.indexOf(opt);
              const selected = flatIdx === selectedIndex;
              return (
                <div
                  key={opt.id}
                  className={`${styles.option} ${selected ? styles.optionSelected : ""}`}
                  onClick={() => activate(opt)}
                  onMouseEnter={() => setSelectedIndex(flatIdx)}
                >
                  <span className={styles.optionName}>
                    {opt.icon && (
                      <span className={styles.optionIcon}>{opt.icon}</span>
                    )}
                    {opt.name}
                  </span>
                  {opt.description && (
                    <Typography.Text
                      type="secondary"
                      className={styles.optionDesc}
                    >
                      {opt.description}
                    </Typography.Text>
                  )}
                </div>
              );
            })}
          </div>
        ))}
      </div>
    </Modal>
  );
};