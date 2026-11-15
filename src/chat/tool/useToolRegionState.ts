import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useOrbOverlayClose } from "@/orb/orbOverlays";
import type { ConversationToolConfig } from "../core/chatTypes";
import { diffToolConfig, isEmptyToolConfigDiff } from "../core/toolConfig";
import type { ToolRegionInput, ToolRegionView } from "../layouts/layoutProps";
import { useToolCatalog } from "./useToolCatalog";

/** 装配变更提示的展示时长（I38d §4.7）。 */
const NOTICE_TIMEOUT_MS = 6000;

/**
 * 工具面板的「内部」区域：面板根、概览条 chip，以及面板内 `Select` 的弹层。
 * 弹层由 antd 挂到 orb 窗口根节点（见 `OrbWindow` 的 `getPopupContainer`），
 * 不在面板 DOM 子树内，必须按类名单独放行，否则点选项会被判成「点外部」，
 * 面板连同弹层一起卸载，选择动作丢失。
 */
const TOOL_PANEL_INNER_SELECTORS = [
  "[data-tool-panel]",
  "[data-tool-panel-anchor]",
  ".tool-panel-dropdown",
];

function isToolPanelInner(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  return TOOL_PANEL_INNER_SELECTORS.some(
    (selector) => target.closest(selector) !== null,
  );
}

const noop = () => {};

/**
 * 工具区域状态的唯一持有者（I48 §5）：面板开合、工具目录、外部点击收起、
 * 切会话 / orb 收起重置、变更提示与定时器、默认值写回进行态。
 *
 * 禁止 import `chatService`、任何 store 的写操作、`settings` 域模块——
 * 这些属于视图（命令侧），由 `ToolRegionInput` 的回调注入。
 */
export function useToolRegionState(
  input: ToolRegionInput & { enabled: boolean },
): ToolRegionView {
  const { enabled, conversationId, config, onChange, onSetDefault } = input;
  const { t } = useTranslation();
  const { tools, loading } = useToolCatalog(enabled);
  const [open, setOpen] = useState(false);
  const [settingDefault, setSettingDefault] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const noticeTimerRef = useRef<number | null>(null);

  const clearNoticeTimer = useCallback(() => {
    if (noticeTimerRef.current !== null) {
      window.clearTimeout(noticeTimerRef.current);
      noticeTimerRef.current = null;
    }
  }, []);

  const clearNotice = useCallback(() => {
    clearNoticeTimer();
    setNotice(null);
  }, [clearNoticeTimer]);

  const showNotice = useCallback(
    (text: string) => {
      setNotice(text);
      clearNoticeTimer();
      noticeTimerRef.current = window.setTimeout(() => {
        noticeTimerRef.current = null;
        setNotice(null);
      }, NOTICE_TIMEOUT_MS);
    },
    [clearNoticeTimer],
  );

  /** 卸载时清定时器：只清资源，不写状态。 */
  useEffect(() => clearNoticeTimer, [clearNoticeTimer]);

  /**
   * 离开 agent 链路即收起（重新进入不自动展开，有意收紧，I48 §5）；
   * 切会话时面板与提示一并归零（面板内容属于当前会话的工具配置）。
   */
  useEffect(() => {
    setOpen(false);
    if (!enabled) return;
    clearNotice();
  }, [enabled, conversationId, clearNotice]);

  /** 面板不设「完成」按钮：点击面板外部（对话区任意位置）即收起。 */
  useEffect(() => {
    if (!enabled || !open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (isToolPanelInner(event.target)) return;
      setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown, true);
    return () =>
      document.removeEventListener("pointerdown", onPointerDown, true);
  }, [enabled, open]);

  /** orb 收起时同步关闭面板：否则球上会残留展开中的面板 / 弹层。 */
  const handleOrbClose = useCallback(() => {
    if (enabled) setOpen(false);
  }, [enabled]);
  useOrbOverlayClose(handleOrbClose);

  const toggle = useCallback(() => {
    setOpen((prev) => (enabled ? !prev : false));
  }, [enabled]);

  const applyChange = useCallback(
    (next: ConversationToolConfig) => {
      const diff = diffToolConfig(config, next);
      if (!isEmptyToolConfigDiff(diff)) {
        const nameOf = (id: string) =>
          tools.find((tool) => tool.id === id)?.name ?? id;
        const parts: string[] = [];
        if (diff.addedTools.length > 0) {
          parts.push(
            t("chat.tool.notice.enabled", {
              names: diff.addedTools.map(nameOf).join(", "),
            }),
          );
        }
        if (diff.removedTools.length > 0) {
          parts.push(
            t("chat.tool.notice.disabled", {
              names: diff.removedTools.map(nameOf).join(", "),
            }),
          );
        }
        if (diff.tierChanged) {
          parts.push(
            t("chat.tool.notice.tier", {
              tier: t(`chat.tool.approval.tier.${diff.tierChanged}`),
            }),
          );
        }
        if (diff.levelChanged) {
          parts.push(
            t("chat.tool.notice.level", {
              level: t(`chat.tool.approval.level.${diff.levelChanged}`),
            }),
          );
        }
        showNotice(parts.join(t("chat.tool.notice.separator")));
      }
      onChange(next);
    },
    [config, tools, t, showNotice, onChange],
  );

  const applySetDefault = useCallback(async () => {
    setSettingDefault(true);
    try {
      await onSetDefault();
    } finally {
      setSettingDefault(false);
    }
  }, [onSetDefault]);

  const handleSetDefault = useCallback(() => {
    void applySetDefault();
  }, [applySetDefault]);

  if (!enabled) {
    return {
      open: false,
      toolbar: { config, total: 0, onToggle: noop },
      panel: {
        config,
        tools: [],
        loading: false,
        canSetDefault: false,
        settingDefault: false,
        onChange: noop,
        onSetDefault: noop,
      },
      notice: null,
    };
  }

  return {
    open,
    toolbar: { config, total: tools.length, onToggle: toggle },
    panel: {
      config,
      tools,
      loading,
      canSetDefault: true,
      settingDefault,
      onChange: applyChange,
      onSetDefault: handleSetDefault,
    },
    notice,
  };
}
