import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";

import {
  AWAY_THRESHOLD,
  LOAD_MORE_TRIGGER,
  distanceFromBottom,
  nextPinned,
  planEntry,
  planResize,
  type ScrollMetrics,
} from "./scrollIntent";
import { readPosition, savePosition } from "./scrollPositionMemory";
import type { Message } from "../../chat/core/chatTypes";

interface MessageScrollOptions {
  /** 当前会话 id：`entry` 意图的唯一判据，也是位置记忆的键（I48 §6.1）。 */
  conversationId: string | null;
  /** 已过滤掉 tool 行的可见消息，只用于条数判定与降级跟随的触发。 */
  visibleMessages: Message[];
  isStreaming: boolean;
  streamingContent?: string;
  hasMore: boolean;
  isLoadingMore: boolean;
  onLoadMore?: () => void;
}

/**
 * 消息列表的滚动编排：落位、跟随、前插补偿、未读计数与位置记忆。
 * 只做视口与内容高度的读写，不参与消息分组或渲染决策。
 */
export function useMessageScroll({
  conversationId,
  visibleMessages,
  isStreaming,
  streamingContent,
  hasMore,
  isLoadingMore,
  onLoadMore,
}: MessageScrollOptions) {
  const scrollRef = useRef<HTMLDivElement>(null);
  /** 内容容器：`ResizeObserver` 的观察对象，滚动容器不能兼任（I48 §6.4）。 */
  const contentRef = useRef<HTMLDivElement>(null);
  const [userScrolledUp, setUserScrolledUp] = useState(false);
  const [unreadCount, setUnreadCount] = useState(0);
  const rafIdRef = useRef<number | null>(null);
  const loadMoreTriggeredRef = useRef(false);
  const programScrollRef = useRef(false);
  const prevIsStreamingRef = useRef(isStreaming);
  /** 是否跟随新内容：唯一真相，`userScrolledUp` 只是它的展示投影（I48 §6.6 I4）。 */
  const pinnedRef = useRef(true);
  /** RO 基线：`entry` 之后必须重设，否则首个回调会把整页高度判成 `follow`。 */
  const baselineRef = useRef({ contentHeight: 0, viewportHeight: 0 });
  /** 前插分页发起前的快照，用于高度补偿；不能读 store 的加载标志（§6.2）。 */
  const pendingPrependRef = useRef<{ height: number; top: number } | null>(
    null,
  );
  const prependTimeoutRef = useRef<number | null>(null);
  /** 卸载时保存位置需要最新会话 id，不能依赖闭包快照。 */
  const conversationIdRef = useRef(conversationId);
  conversationIdRef.current = conversationId;
  /** 卸载清理时 `scrollRef` 已被置空，留一份元素实例用于读最后一次位置。 */
  const elementRef = useRef<HTMLDivElement | null>(null);

  const seenMessageCountRef = useRef(visibleMessages.length);
  /** 供 `entry` 读取当前可见条数，避免把条数写进依赖导致重复落位。 */
  const visibleCountRef = useRef(visibleMessages.length);
  visibleCountRef.current = visibleMessages.length;

  /** 一次性读取视口与内容的度量；容器滚动高度已包含 padding，无需再加。 */
  const readMetrics = useCallback((): ScrollMetrics | null => {
    const el = scrollRef.current;
    if (!el) return null;
    return {
      scrollTop: el.scrollTop,
      contentHeight: el.scrollHeight,
      viewportHeight: el.clientHeight,
    };
  }, []);

  const clearPendingPrepend = useCallback(() => {
    pendingPrependRef.current = null;
    if (prependTimeoutRef.current !== null) {
      window.clearTimeout(prependTimeoutRef.current);
      prependTimeoutRef.current = null;
    }
  }, []);

  /**
   * 程序性位移的唯一写入点（I48 §6.6 I1）：按 `maxTop` 钳制后与当前值比较，
   * 只有真正产生位移才置 `programScrollRef`——否则「写入无效」不会触发 scroll 事件，
   * 标志会滞留，把用户下一次真实滚动的判定吞掉。
   * 写入后同步位置记忆：程序性位移不会走滚动回调的保存分支（§7）。
   */
  const writeScrollTop = useCallback((top: number) => {
    const el = scrollRef.current;
    if (!el) return;
    const limit = Math.max(0, el.scrollHeight - el.clientHeight);
    const target = Math.min(Math.max(top, 0), limit);
    if (Math.abs(el.scrollTop - target) < 0.5) return;
    programScrollRef.current = true;
    el.scrollTop = target;
    savePosition(conversationIdRef.current, {
      top: target,
      pinned: pinnedRef.current,
    });
  }, []);

  /**
   * 发起前插分页的唯一入口：滚动触顶与「加载更早」按钮都走这里，
   * 保证补偿所需的基线快照一定被记录（§6.2）。
   */
  const requestLoadMore = useCallback(() => {
    const m = readMetrics();
    if (!m || !onLoadMore || pendingPrependRef.current) return;
    pendingPrependRef.current = { height: m.contentHeight, top: m.scrollTop };
    if (prependTimeoutRef.current !== null) {
      window.clearTimeout(prependTimeoutRef.current);
    }
    prependTimeoutRef.current = window.setTimeout(clearPendingPrepend, 3000);
    onLoadMore();
  }, [readMetrics, clearPendingPrepend, onLoadMore]);

  useEffect(() => {
    if (userScrolledUp) {
      const delta = visibleMessages.length - seenMessageCountRef.current;
      if (delta > 0) setUnreadCount((c) => c + delta);
    }
    seenMessageCountRef.current = visibleMessages.length;
  }, [visibleMessages.length, userScrolledUp]);

  /**
   * `entry` 意图：唯一的「进入会话」落位时机，在 paint 前同步写入（I48 §6.5）。
   * 依赖只有 `conversationId`——高度变化、流式状态、`link` 都不得触发落位（I3）。
   */
  useLayoutEffect(() => {
    const m = readMetrics();
    if (!m) return;
    const plan = planEntry(m, readPosition(conversationId));
    /** 先定意图再写入：`writeScrollTop` 会把当前位置连同 `pinned` 一起记入记忆。 */
    pinnedRef.current = plan.pinned;
    writeScrollTop(plan.top);
    baselineRef.current = {
      contentHeight: m.contentHeight,
      viewportHeight: m.viewportHeight,
    };
    setUserScrolledUp(!plan.pinned);
    setUnreadCount(0);
    seenMessageCountRef.current = visibleCountRef.current;
    clearPendingPrepend();
  }, [conversationId, readMetrics, writeScrollTop, clearPendingPrepend]);

  /**
   * `follow` / `compensate` 意图：一个 RO 同时观察内容与视口，
   * 回调在 layout 之后、paint 之前，回调内直接写入，不再叠 rAF（I48 §6.5）。
   */
  useEffect(() => {
    const el = scrollRef.current;
    const content = contentRef.current;
    if (!el || !content || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => {
      const m = readMetrics();
      if (!m) return;
      const prev = baselineRef.current;
      const delta = m.contentHeight - prev.contentHeight;
      const viewportDelta = m.viewportHeight - prev.viewportHeight;
      baselineRef.current = {
        contentHeight: m.contentHeight,
        viewportHeight: m.viewportHeight,
      };
      const plan = planResize({
        metrics: m,
        delta,
        viewportDelta,
        pinned: pinnedRef.current,
        pendingPrepend: pendingPrependRef.current,
      });
      if (plan.action === "compensate") {
        writeScrollTop(plan.top);
        clearPendingPrepend();
        return;
      }
      if (plan.action === "follow") {
        writeScrollTop(m.contentHeight);
      }
    });
    ro.observe(content);
    ro.observe(el);
    return () => ro.disconnect();
  }, [readMetrics, writeScrollTop, clearPendingPrepend]);

  /** 降级：`ResizeObserver` 不可用时按渲染后的高度比较，同样只发一次 `follow`。 */
  useEffect(() => {
    if (typeof ResizeObserver !== "undefined") return;
    if (!pinnedRef.current) return;
    const el = scrollRef.current;
    const m = readMetrics();
    if (!el || !m) return;
    if (m.contentHeight === baselineRef.current.contentHeight) return;
    baselineRef.current = {
      contentHeight: m.contentHeight,
      viewportHeight: m.viewportHeight,
    };
    writeScrollTop(m.contentHeight);
  }, [visibleMessages, streamingContent, readMetrics, writeScrollTop]);

  useEffect(() => {
    if (prevIsStreamingRef.current && !isStreaming) {
      prevIsStreamingRef.current = isStreaming;
      const m = readMetrics();
      if (!m) return;
      if (distanceFromBottom(m) <= AWAY_THRESHOLD) {
        pinnedRef.current = true;
        setUserScrolledUp(false);
        setUnreadCount(0);
      }
    } else {
      prevIsStreamingRef.current = isStreaming;
    }
  }, [isStreaming, readMetrics]);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const throttledCheck = () => {
      if (rafIdRef.current !== null) return;
      rafIdRef.current = requestAnimationFrame(() => {
        rafIdRef.current = null;
        /** I1：程序性写入保护期内跳过本轮全部判定（含加载更多）。 */
        if (programScrollRef.current) {
          programScrollRef.current = false;
          return;
        }
        const m = readMetrics();
        if (!m) return;
        const pinned = nextPinned(pinnedRef.current, distanceFromBottom(m));
        if (pinned !== pinnedRef.current) {
          pinnedRef.current = pinned;
          setUserScrolledUp(!pinned);
        }
        /** 位置记忆的保存点之一：覆盖同一会话内的移动（§7）。 */
        savePosition(conversationId, {
          top: m.scrollTop,
          pinned: pinnedRef.current,
        });

        if (
          hasMore &&
          !isLoadingMore &&
          m.scrollTop < LOAD_MORE_TRIGGER &&
          !pendingPrependRef.current &&
          !loadMoreTriggeredRef.current
        ) {
          loadMoreTriggeredRef.current = true;
          requestLoadMore();
          setTimeout(() => {
            loadMoreTriggeredRef.current = false;
          }, 1000);
        }
      });
    };
    el.addEventListener("scroll", throttledCheck, { passive: true });
    return () => {
      el.removeEventListener("scroll", throttledCheck);
      if (rafIdRef.current !== null) cancelAnimationFrame(rafIdRef.current);
    };
  }, [hasMore, isLoadingMore, conversationId, readMetrics, requestLoadMore]);

  /**
   * 位置记忆的另一个保存点：卸载时。
   * 卸载后 `scrollRef` 已被 React 置空，因此在每次 render 后把元素实例留在
   * `elementRef` 里（脱挂节点仍保留 `scrollTop`），清理时读它。
   * 不在 `conversationId` 变化的 cleanup 中保存——那时新内容已渲染，
   * 浏览器可能已按新内容 clamp 过 `scrollTop`，会把旧会话写成 0（§7）。
   */
  useLayoutEffect(() => {
    if (scrollRef.current) elementRef.current = scrollRef.current;
  });

  useEffect(
    () => () => {
      const el = elementRef.current;
      if (!el) return;
      savePosition(conversationIdRef.current, {
        top: el.scrollTop,
        pinned: pinnedRef.current,
      });
    },
    [],
  );

  /** 卸载时清掉前插定时器：只清资源，不写状态。 */
  useEffect(() => clearPendingPrepend, [clearPendingPrepend]);

  const scrollToBottom = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    const limit = Math.max(0, el.scrollHeight - el.clientHeight);
    if (Math.abs(el.scrollTop - limit) >= 0.5) {
      programScrollRef.current = true;
      el.scrollTo({ top: limit, behavior: "smooth" });
    }
    pinnedRef.current = true;
    setUserScrolledUp(false);
    setUnreadCount(0);
  }, []);

  return {
    scrollRef,
    contentRef,
    userScrolledUp,
    unreadCount,
    requestLoadMore,
    scrollToBottom,
  };
}
