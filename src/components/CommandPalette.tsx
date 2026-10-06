import React, { useEffect, useRef, useState } from "react";
import { Modal, Input, Typography, Divider } from "antd";
import {
  SearchOutlined,
  ThunderboltOutlined,
  AppstoreOutlined,
  BulbOutlined,
} from "@ant-design/icons";
import { useNavigate } from "react-router-dom";
import Fuse from "fuse.js";
import { tools } from "@/tools/registry";
import { COMMAND_PALETTE_MAX_PER_CATEGORY } from "@/config/constants";
import { useSettingsStore } from "@/stores/settingsStore";
import { dispatchThemeChanged } from "@/settings/settingsDispatcher";

interface PaletteOption {
  id: string;
  name: string;
  description?: string;
  category: string;
  action: () => void;
}

const IS_MAC = /\bMacintosh\b/i.test(navigator.userAgent);

export const CommandPalette: React.FC = () => {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const navigate = useNavigate();
  const appConf = useSettingsStore((s) => s.appConf);
  const saveAppConf = useSettingsStore((s) => s.saveAppConf);

  const allOptions: PaletteOption[] = React.useMemo(() => {
    const toolOpts: PaletteOption[] = tools.map((t) => ({
      id: `tool:${t.id}`,
      name: t.name,
      description: t.description,
      category: "Tools",
      action: () => navigate(`/tools/${t.id}`),
    }));

    const navOpts: PaletteOption[] = [
      {
        id: "nav:history",
        name: "消息历史",
        description: "查看对话历史记录",
        category: "Navigation",
        action: () => navigate("/history"),
      },
      {
        id: "nav:settings-general",
        name: "通用设置",
        description: "语言、开机自启、关闭到托盘",
        category: "Navigation",
        action: () => navigate("/settings/general"),
      },
      {
        id: "nav:settings-appearance",
        name: "外观设置",
        description: "主题、Orb 外观配置",
        category: "Navigation",
        action: () => navigate("/settings/appearance"),
      },
      {
        id: "nav:settings-shortcuts",
        name: "快捷键",
        description: "全局与应用内快捷键配置",
        category: "Navigation",
        action: () => navigate("/settings/shortcuts"),
      },
      {
        id: "nav:settings-providers",
        name: "Provider 管理",
        description: "管理 AI Provider 配置",
        category: "Navigation",
        action: () => navigate("/settings/providers"),
      },
      {
        id: "nav:settings-templates",
        name: "Prompt 模板",
        description: "管理 Prompt 模板",
        category: "Navigation",
        action: () => navigate("/settings/templates"),
      },
      {
        id: "nav:settings-about",
        name: "关于",
        description: "版本信息与更新",
        category: "Navigation",
        action: () => navigate("/settings/about"),
      },
    ];

    const actionOpts: PaletteOption[] = [
      {
        id: "action:random-tool",
        name: "随机工具",
        description: "随机打开一个工具",
        category: "Actions",
        action: () => {
          const t = tools[Math.floor(Math.random() * tools.length)];
          navigate(`/tools/${t.id}`);
        },
      },
      {
        id: "action:toggle-theme",
        name: "切换主题",
        description: "循环切换 亮色 → 暗色 → 跟随系统",
        category: "Actions",
        action: async () => {
          if (!appConf) return;
          const order: Array<"system" | "light" | "dark"> = [
            "system",
            "light",
            "dark",
          ];
          const current = appConf.theme as "system" | "light" | "dark";
          const next = order[(order.indexOf(current) + 1) % order.length];
          await saveAppConf({ ...appConf, theme: next });
          await dispatchThemeChanged(next);
        },
      },
    ];

    return [...toolOpts, ...navOpts, ...actionOpts];
  }, [navigate, appConf, saveAppConf]);

  const fuse = React.useMemo(
    () =>
      new Fuse(allOptions, {
        keys: [
          { name: "name", weight: 2 },
          { name: "description", weight: 1 },
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

  const flatResults = React.useMemo(
    () => Object.values(groupedResults).flat(),
    [groupedResults],
  );

  useEffect(() => {
    const binding = appConf?.keybindings?.command_palette ?? "Super+K";

    const requireMeta = /\bsuper\b/i.test(binding) || /\bcmd\b/i.test(binding);
    const requireCtrl = /\bctrl\b/i.test(binding) && !requireMeta;
    const requireShift = /\bshift\b/i.test(binding);
    const requireAlt = /\balt\b/i.test(binding);
    const keyMatch = binding.match(/\b([A-Z0-9])\s*$/i);
    const key = keyMatch ? keyMatch[1].toLowerCase() : "k";

    const handler = (e: KeyboardEvent) => {
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
      if (opt) {
        opt.action();
        setOpen(false);
        setQuery("");
      }
    } else if (e.key === "Escape") {
      setOpen(false);
      setQuery("");
    }
  };

  const categoryIcons: Record<string, React.ReactNode> = {
    Tools: <ThunderboltOutlined />,
    Navigation: <AppstoreOutlined />,
    Actions: <BulbOutlined />,
  };

  return (
    <Modal
      open={open}
      onCancel={() => {
        setOpen(false);
        setQuery("");
      }}
      footer={null}
      closable={false}
      width={520}
      styles={{ body: { padding: 0 } }}
    >
      <div style={{ padding: "12px 16px 8px" }}>
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
      <div
        style={{
          maxHeight: 320,
          overflowY: "auto",
          padding: "0 16px 12px",
        }}
      >
        {Object.entries(groupedResults).map(([cat, opts]) => (
          <div key={cat}>
            <Divider style={{ margin: "8px 0 4px" }}>
              <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                {categoryIcons[cat]} {cat}
              </Typography.Text>
            </Divider>
            {opts.map((opt) => {
              const flatIdx = flatResults.indexOf(opt);
              return (
                <div
                  key={opt.id}
                  onClick={() => {
                    opt.action();
                    setOpen(false);
                    setQuery("");
                  }}
                  style={{
                    padding: "6px 8px",
                    borderRadius: 4,
                    cursor: "pointer",
                    background:
                      flatIdx === selectedIndex
                        ? "var(--color-bg-elevated, #f0f0f0)"
                        : "transparent",
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                  }}
                  onMouseEnter={() => setSelectedIndex(flatIdx)}
                >
                  <span>{opt.name}</span>
                  {opt.description && (
                    <Typography.Text type="secondary" style={{ fontSize: 12 }}>
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
