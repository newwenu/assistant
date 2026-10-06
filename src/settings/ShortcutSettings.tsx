import { useEffect, useMemo, useState } from "react";
import { Input, Button, Typography, Space, Tag, message } from "antd";
import { SearchOutlined, UndoOutlined } from "@ant-design/icons";
import { useSettingsStore } from "@/stores/settingsStore";
import { ShortcutRecorder } from "./ShortcutRecorder";
import {
  SHORTCUT_DEFINITIONS,
  SHORTCUT_GROUPS,
  DEFAULT_KEYBINDINGS,
  formatBinding,
  detectConflicts,
  type ConflictInfo,
} from "./shortcutTypes";

export const ShortcutSettings: React.FC = () => {
  const appConf = useSettingsStore((s) => s.appConf);
  const loadAppConf = useSettingsStore((s) => s.loadAppConf);
  const updateKeybinding = useSettingsStore((s) => s.updateKeybinding);
  const resetKeybindings = useSettingsStore((s) => s.resetKeybindings);
  const [searchQuery, setSearchQuery] = useState("");
  const [messageApi, contextHolder] = message.useMessage();

  useEffect(() => {
    loadAppConf();
  }, [loadAppConf]);

  const keybindings = appConf?.keybindings ?? DEFAULT_KEYBINDINGS;

  const conflicts = useMemo(() => detectConflicts(keybindings), [keybindings]);

  const filteredDefinitions = useMemo(() => {
    if (!searchQuery.trim()) return SHORTCUT_DEFINITIONS;
    const q = searchQuery.toLowerCase();
    return SHORTCUT_DEFINITIONS.filter(
      (s) =>
        s.label.toLowerCase().includes(q) ||
        s.id.toLowerCase().includes(q) ||
        formatBinding(keybindings[s.id] ?? s.defaultBinding)
          .toLowerCase()
          .includes(q),
    );
  }, [searchQuery, keybindings]);

  const handleConfirm = (id: string) => async (binding: string) => {
    await updateKeybinding(id, binding);
    messageApi.success("快捷键已更新");
  };

  const handleClear = (id: string) => async () => {
    await updateKeybinding(id, "");
    messageApi.info("快捷键已清除");
  };

  const handleReset = async () => {
    await resetKeybindings();
    messageApi.success("已恢复默认快捷键");
  };

  return (
    <>
      {contextHolder}
      <div style={{ maxWidth: 700, margin: "0 auto", padding: 24 }}>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            marginBottom: 16,
          }}
        >
          <Typography.Title level={5} style={{ margin: 0 }}>
            快捷键
          </Typography.Title>
          <Button icon={<UndoOutlined />} onClick={handleReset}>
            恢复默认
          </Button>
        </div>

        <Input
          placeholder="搜索快捷键..."
          prefix={<SearchOutlined />}
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          style={{ marginBottom: 16 }}
          allowClear
        />

        {SHORTCUT_GROUPS.map((group) => {
          const groupItems = filteredDefinitions.filter(
            (s) => s.group === group.key,
          );
          if (groupItems.length === 0) return null;

          return (
            <div key={group.key} style={{ marginBottom: 16 }}>
              <Typography.Text
                strong
                style={{ fontSize: 13, color: "var(--color-text-secondary)" }}
              >
                {group.label}
              </Typography.Text>
              <div style={{ marginTop: 8 }}>
                {groupItems.map((shortcut) => {
                  const currentBinding =
                    keybindings[shortcut.id] ?? shortcut.defaultBinding;
                  const conflict: ConflictInfo | undefined =
                    conflicts[shortcut.id];

                  return (
                    <div
                      key={shortcut.id}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "space-between",
                        padding: "8px 12px",
                        borderBottom: "1px solid var(--color-border-secondary)",
                        background: conflict
                          ? conflict.level === "error"
                            ? "var(--color-error-bg)"
                            : "var(--color-warning-bg)"
                          : "transparent",
                      }}
                    >
                      <div
                        style={{
                          display: "flex",
                          flexDirection: "column",
                          gap: 2,
                        }}
                      >
                        <Typography.Text>{shortcut.label}</Typography.Text>
                        {conflict && (
                          <Typography.Text
                            type={
                              conflict.level === "error" ? "danger" : "warning"
                            }
                            style={{ fontSize: 12 }}
                          >
                            {conflict.level === "error"
                              ? "同组快捷键冲突"
                              : "跨组快捷键冲突，可能无法同时生效"}
                          </Typography.Text>
                        )}
                      </div>
                      <Space>
                        <ShortcutRecorder
                          currentBinding={currentBinding}
                          onConfirm={handleConfirm(shortcut.id)}
                          onClear={handleClear(shortcut.id)}
                        />
                        {currentBinding !== shortcut.defaultBinding && (
                          <Tag color="orange">已修改</Tag>
                        )}
                      </Space>
                    </div>
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>
    </>
  );
};