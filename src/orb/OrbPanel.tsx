import React, { useEffect, useCallback, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { UiEvent } from "@/types/events";
import { useTranslation } from "react-i18next";
import { Button, Tooltip, Dropdown, Input, message } from "antd";
import {
  LeftOutlined,
  RightOutlined,
  MenuOutlined,
  PushpinOutlined,
  PushpinFilled,
} from "@ant-design/icons";
import type { MenuProps } from "antd";
import {
  ORB_BLUR_COLLAPSE_DELAY_MS,
  ORB_HISTORY_LIST_LIMIT,
} from "../config/constants";
import { useOrbStore } from "../stores/orbStore";
import { formatConversationTitle } from "../utils/formatConversationTitle";
import { isOrbBlurCollapseSuppressed, useOrbExpand } from "./OrbExpand";
import { openFromPanel } from "./panelActions";
import { ConversationView } from "../chat/ConversationView";
import { useChatStore, getChatOrchestrator } from "../stores/chatStore";
import {
  subscribeChatStreamEvents,
  setChatOrchestrator,
} from "../services/chatStreamEvents";
import { subscribeChatLifecycleEvents } from "../services/chatLifecycleEvents";
import { useMascotChatSync } from "../mascot/useMascotChatSync";
import {
  getMouseLogicalPos,
  getWindowLogicalRect,
  isPointInRect,
} from "./OrbCoord";
import { formatRelativeTime } from "../utils/formatRelativeTime";
import { useNotifyStore } from "../stores/notifyStore";
import { translateKey } from "../i18n";

function useMouseOnlyTooltip(): {
  open: boolean;
  onOpenChange: (open: boolean) => void;
} {
  const [open, setOpen] = useState(false);
  const [pointerType, setPointerType] = useState("mouse");

  useEffect(() => {
    const onPointer = (e: PointerEvent) => {
      setPointerType(e.pointerType);
      if (e.pointerType !== "mouse") setOpen(false);
    };
    document.addEventListener("pointermove", onPointer, true);
    document.addEventListener("pointerdown", onPointer, true);
    return () => {
      document.removeEventListener("pointermove", onPointer, true);
      document.removeEventListener("pointerdown", onPointer, true);
    };
  }, []);

  const onOpenChange = useCallback(
    (next: boolean) => {
      setOpen(next && pointerType === "mouse");
    },
    [pointerType],
  );

  return { open: open && pointerType === "mouse", onOpenChange };
}

export const OrbPanel: React.FC = () => {
  const { expand, collapse } = useOrbExpand();
  const expandDirection = useOrbStore((s) => s.expandDirection);
  const keepExpanded = useOrbStore((s) => s.keepExpanded);
  const setKeepExpanded = useOrbStore((s) => s.setKeepExpanded);
  const historyTip = useMouseOnlyTooltip();
  const keepExpandedTip = useMouseOnlyTooltip();
  const collapseTip = useMouseOnlyTooltip();
  const chat = useChatStore();
  const panelRef = useRef<HTMLDivElement>(null);
  const [messageApi, messageContextHolder] = message.useMessage({
    getContainer: () => panelRef.current ?? document.body,
  });
  const notifyPending = useNotifyStore((s) => s.pending);
  const notifyDequeue = useNotifyStore((s) => s.dequeue);

  React.useEffect(() => {
    while (notifyPending.length > 0) {
      const item = notifyDequeue();
      if (!item) break;
      const content = translateKey(item.titleKey, item.options);
      const method =
        item.level === "warning"
          ? messageApi.warning
          : item.level === "info"
            ? messageApi.info
            : messageApi.error;
      method({ content, duration: 3 });
    }
  }, [notifyPending, notifyDequeue, messageApi]);
  const { t } = useTranslation();
  useMascotChatSync();
  const [search, setSearch] = useState("");
  const [historyOpen, setHistoryOpen] = useState(false);
  const modelOpenRef = useRef(false);

  const closeAllMenus = useCallback(() => {
    setHistoryOpen(false);
    modelOpenRef.current = false;
    window.dispatchEvent(new CustomEvent("orb-close-menus"));
  }, []);

  useEffect(() => {
    let timer: number | null = null;
    const unlistenBlur = listen(UiEvent.OrbBlur, () => {
      closeAllMenus();
      if (useOrbStore.getState().keepExpanded) return;
      if (isOrbBlurCollapseSuppressed()) return;
      if (timer !== null) clearTimeout(timer);
      timer = window.setTimeout(() => {
        void (async () => {
          if (isOrbBlurCollapseSuppressed()) return;
          const state = useOrbStore.getState();
          if (state.isDragging) return;
          try {
            const [mouse, winRect] = await Promise.all([
              getMouseLogicalPos(),
              getWindowLogicalRect(),
            ]);
            if (mouse && isPointInRect(mouse.x, mouse.y, winRect)) return;
          } catch {
            // if coord check fails, fall through to collapse
          }
          void collapse();
        })();
      }, ORB_BLUR_COLLAPSE_DELAY_MS);
    });
    const unlistenNewChat = listen(UiEvent.OrbNewConversation, () => {
      void (async () => {
        if (!useOrbStore.getState().isExpanded) {
          await expand();
        }
        window.dispatchEvent(new CustomEvent("chat-new-conversation"));
      })();
    });
    const orchestrator = getChatOrchestrator();
    if (orchestrator) setChatOrchestrator(orchestrator);
    const cleanupChatStream = subscribeChatStreamEvents();
    const cleanupChatLifecycle = subscribeChatLifecycleEvents();
    return () => {
      void unlistenBlur.then((fn) => fn());
      void unlistenNewChat.then((fn) => fn());
      cleanupChatStream();
      cleanupChatLifecycle();
      if (timer !== null) clearTimeout(timer);
    };
  }, [expand, collapse, closeAllMenus]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        if (historyOpen || modelOpenRef.current) {
          closeAllMenus();
          return;
        }
        const state = useOrbStore.getState();
        if (state.isDragging) return;
        void collapse();
      }
    },
    [collapse, historyOpen, closeAllMenus],
  );

  const CollapseIcon =
    expandDirection === "left" ? RightOutlined : LeftOutlined;

  const filtered = chat.conversations.filter((c) =>
    c.title.toLowerCase().includes(search.toLowerCase()),
  );

  const truncated = filtered.slice(0, ORB_HISTORY_LIST_LIMIT);
  const isTruncated = filtered.length > ORB_HISTORY_LIST_LIMIT;

  const handleOpenHistoryInMain = useCallback(() => {
    setHistoryOpen(false);
    void openFromPanel({ kind: "main", route: "/history" });
  }, []);

  const historyMenuItems: MenuProps["items"] = [
    {
      key: "search",
      label: (
        <Input.Search
          placeholder={t("chat.orb.search_conversation")}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          size="small"
          allowClear
          onClick={(e) => e.stopPropagation()}
        />
      ),
      disabled: true,
    },
    { type: "divider" },
    ...truncated.map((conv) => ({
      key: conv.id,
      label: (
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            gap: 8,
            minWidth: 160,
          }}
        >
          <span
            style={{
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
              fontWeight: conv.id === chat.currentConversationId ? 600 : 400,
            }}
          >
            {formatConversationTitle(conv)}
          </span>
          <span
            style={{
              fontSize: 11,
              color: "var(--color-text-tertiary)",
              flexShrink: 0,
            }}
          >
            {formatRelativeTime(conv.updatedAt)}
          </span>
        </div>
      ),
      onClick: () => {
        void chat.switchConversation(conv.id);
        setHistoryOpen(false);
      },
    })),
  ];

  if (isTruncated) {
    historyMenuItems.push(
      { type: "divider" },
      {
        key: "view-all",
        label: (
          <span style={{ color: "var(--color-primary)" }}>
            {search
              ? t("chat.orb.search_more_in_main")
              : t("chat.orb.view_all_in_main")}
          </span>
        ),
        onClick: handleOpenHistoryInMain,
      },
    );
  }

  if (filtered.length === 0 && chat.conversations.length === 0) {
    historyMenuItems.splice(1);
    historyMenuItems.push({
      key: "empty",
      label: t("history.no_conversations"),
      disabled: true,
    });
  }

  return (
    <div
      ref={panelRef}
      className="orb-panel orb-compact"
      onKeyDown={handleKeyDown}
      tabIndex={-1}
    >
      <div className="orb-chrome-top" data-tauri-drag-region>
        <Dropdown
          menu={{ items: historyMenuItems }}
          trigger={["click"]}
          placement="bottomLeft"
          open={historyOpen}
          onOpenChange={setHistoryOpen}
          classNames={{ root: "orb-dropdown" }}
        >
          <Tooltip
            title={t("chat.orb.conversation_list")}
            mouseEnterDelay={0.5}
            open={historyTip.open}
            onOpenChange={historyTip.onOpenChange}
          >
            <Button
              type="text"
              size="small"
              className="orb-chrome-btn"
              icon={<MenuOutlined />}
            />
          </Tooltip>
        </Dropdown>
        <div className="orb-chrome-center" />
        <Tooltip
          title={
            keepExpanded
              ? t("chat.orb.stop_keep_expanded")
              : t("chat.orb.keep_expanded")
          }
          mouseEnterDelay={0.5}
          open={keepExpandedTip.open}
          onOpenChange={keepExpandedTip.onOpenChange}
        >
          <Button
            type="text"
            size="small"
            className="orb-chrome-btn"
            icon={keepExpanded ? <PushpinFilled /> : <PushpinOutlined />}
            onClick={() => setKeepExpanded(!keepExpanded)}
          />
        </Tooltip>
        <Tooltip
          title={t("common.actions.collapse")}
          mouseEnterDelay={0.5}
          open={collapseTip.open}
          onOpenChange={collapseTip.onOpenChange}
        >
          <Button
            type="text"
            size="small"
            className="orb-chrome-btn"
            icon={<CollapseIcon />}
            onClick={() => {
              closeAllMenus();
              void collapse();
            }}
          />
        </Tooltip>
      </div>
      {messageContextHolder}
      <ConversationView modelOpenRef={modelOpenRef} />
    </div>
  );
};
