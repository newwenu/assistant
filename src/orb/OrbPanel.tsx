import React, { useEffect, useCallback, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { Button, Tooltip, Dropdown, Input } from "antd";
import { LeftOutlined, RightOutlined, MenuOutlined } from "@ant-design/icons";
import type { MenuProps } from "antd";
import { ORB_BLUR_COLLAPSE_DELAY_MS } from "../config/constants";
import { useOrbStore } from "../stores/orbStore";
import { useOrbExpand } from "./OrbExpand";
import { ChatView } from "../chat/ChatView";
import { useChatStore } from "../stores/chatStore";
import { initChatDeltaListener } from "../stores/chatStore";
import {
  getMouseLogicalPos,
  getWindowLogicalRect,
  isPointInRect,
} from "./OrbCoord";

function relativeTime(timestamp: number): string {
  const now = Date.now();
  const diff = now - timestamp;
  if (diff < 60000) return "刚刚";
  if (diff < 3600000) return `${Math.floor(diff / 60000)}分钟前`;
  if (diff < 86400000) return `${Math.floor(diff / 3600000)}小时前`;
  if (diff < 172800000) return "昨天";
  return new Date(timestamp).toLocaleDateString();
}

export const OrbPanel: React.FC = () => {
  const { expand, collapse } = useOrbExpand();
  const expandDirection = useOrbStore((s) => s.expandDirection);
  const chat = useChatStore();
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
    const unlistenBlur = listen("orb-blur", () => {
      closeAllMenus();
      if (timer !== null) clearTimeout(timer);
      timer = window.setTimeout(async () => {
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
        collapse();
      }, ORB_BLUR_COLLAPSE_DELAY_MS);
    });
    const unlistenNewChat = listen("orb-new-conversation", async () => {
      if (!useOrbStore.getState().isExpanded) {
        await expand();
      }
      window.dispatchEvent(new CustomEvent("chat-new-conversation"));
    });
    initChatDeltaListener();
    return () => {
      unlistenBlur.then((fn) => fn());
      unlistenNewChat.then((fn) => fn());
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
        collapse();
      }
    },
    [collapse, historyOpen, closeAllMenus],
  );

  const CollapseIcon =
    expandDirection === "left" ? RightOutlined : LeftOutlined;

  const filtered = chat.conversations.filter((c) =>
    c.title.toLowerCase().includes(search.toLowerCase()),
  );

  const historyMenuItems: MenuProps["items"] = [
    {
      key: "search",
      label: (
        <Input.Search
          placeholder="搜索对话"
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
    ...filtered.map((conv) => ({
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
            {relativeTime(conv.updated_at)}
          </span>
        </div>
      ),
      onClick: () => {
        chat.switchConversation(conv.id);
        setHistoryOpen(false);
      },
    })),
  ];

  if (filtered.length === 0 && chat.conversations.length === 0) {
    historyMenuItems.splice(1);
    historyMenuItems.push({
      key: "empty",
      label: "暂无对话",
      disabled: true,
    });
  }

  return (
    <div
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
          overlayClassName="orb-dropdown"
        >
          <Tooltip title="对话列表" mouseEnterDelay={0.5}>
            <Button
              type="text"
              size="small"
              className="orb-chrome-btn"
              icon={<MenuOutlined />}
            />
          </Tooltip>
        </Dropdown>
        <div className="orb-chrome-center" />
        <Tooltip title="折叠" mouseEnterDelay={0.5}>
          <Button
            type="text"
            size="small"
            className="orb-chrome-btn"
            icon={<CollapseIcon />}
            onClick={() => {
              closeAllMenus();
              collapse();
            }}
          />
        </Tooltip>
      </div>
      <ChatView compact modelOpenRef={modelOpenRef} />
    </div>
  );
};
