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
import { useOrbExpand } from "./OrbExpand";
import { ChatView } from "../chat/ChatView";
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
import { orbService } from "../services/orbService";
import { useErrorNotifyStore } from "../services/errorNotifier";
import { translateKey } from "../i18n";

export const OrbPanel: React.FC = () => {
  const { expand, collapse } = useOrbExpand();
  const expandDirection = useOrbStore((s) => s.expandDirection);
  const isPinned = useOrbStore((s) => s.isPinned);
  const setPinned = useOrbStore((s) => s.setPinned);
  const chat = useChatStore();
  const panelRef = useRef<HTMLDivElement>(null);
  const [messageApi, messageContextHolder] = message.useMessage({
    getContainer: () => panelRef.current ?? document.body,
  });
  const errorNotifyPending = useErrorNotifyStore((s) => s.pending);
  const errorNotifyDequeue = useErrorNotifyStore((s) => s.dequeue);

  React.useEffect(() => {
    while (errorNotifyPending.length > 0) {
      const item = errorNotifyDequeue();
      if (!item) break;
      messageApi.error({
        content: translateKey(item.titleKey, item.options),
        duration: 3,
      });
    }
  }, [errorNotifyPending, errorNotifyDequeue, messageApi]);
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
      if (useOrbStore.getState().isPinned) return;
      if (timer !== null) clearTimeout(timer);
      timer = window.setTimeout(() => {
        void (async () => {
          const state = useOrbStore.getState();
          if (state.isDragging) return;
          try {
            const [mouse, winRect] = await Promise.all([
              getMouseLogicalPos(),
              getWindowLogicalRect(),
            ]);
            if (isPointInRect(mouse.x, mouse.y, winRect)) return;
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
    void orbService.openMainWindowAndNavigate("/history");
    setHistoryOpen(false);
    void collapse();
  }, [collapse]);

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
            {conv.title}
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
          title={isPinned ? t("common.actions.unpin") : t("common.actions.pin")}
          mouseEnterDelay={0.5}
        >
          <Button
            type="text"
            size="small"
            className="orb-chrome-btn"
            icon={isPinned ? <PushpinFilled /> : <PushpinOutlined />}
            onClick={() => setPinned(!isPinned)}
          />
        </Tooltip>
        <Tooltip title={t("common.actions.collapse")} mouseEnterDelay={0.5}>
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
      <ChatView modelOpenRef={modelOpenRef} />
    </div>
  );
};
