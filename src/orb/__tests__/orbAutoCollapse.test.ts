import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  collapseOrbPanel: vi.fn(async () => {}),
  setOrbClickThrough: vi.fn(async () => {}),
}));

vi.mock("../../services/desktop/orbService", () => ({
  orbService: {
    collapseOrbPanel: mocks.collapseOrbPanel,
    setOrbClickThrough: mocks.setOrbClickThrough,
  },
}));

vi.mock("@/settings/core/runtime/useConfGroup", () => ({
  getConfValue: () => ({ clickThrough: true }),
}));

import { autoCollapseOrb, collapseOrbPanel } from "../OrbExpand";
import { useOrbStore } from "../../stores/orbStore";
import { useChatStore } from "../../stores/chat/chatStore";
import { useChatStreamStore } from "../../stores/chat/chatStreamStore";
import type { Message, ToolCallView } from "../../chat/core/chatTypes";

function awaitingMessage(): Message {
  const view: ToolCallView = {
    callId: "a",
    name: "shell",
    status: "awaiting_user",
  };
  return {
    id: "m1",
    conversationId: "c1",
    role: "assistant",
    content: "",
    tokens: null,
    timestamp: 0,
    parentId: null,
    isPinned: false,
    reasoning: null,
    siblingIndex: 0,
    siblingCount: null,
    providerId: null,
    modelId: null,
    modeId: null,
    turnIndex: null,
    iteration: 0,
    toolCalls: [view],
  };
}

beforeEach(() => {
  useOrbStore.setState({
    isExpanded: false,
    isDragging: false,
    keepExpanded: false,
    expandDirection: "right",
    expandVertical: "down",
  });
  useChatStreamStore.setState({ isStreaming: false });
  useChatStore.setState({ messages: [] });
  mocks.collapseOrbPanel.mockClear();
  mocks.setOrbClickThrough.mockClear();
});

describe("autoCollapseOrb", () => {
  it("流式进行中拦截自动收起，且不触碰任何 store 与 service", async () => {
    useOrbStore.setState({ isExpanded: true });
    useChatStreamStore.setState({ isStreaming: true });

    await autoCollapseOrb();

    expect(useOrbStore.getState().isExpanded).toBe(true);
    expect(mocks.collapseOrbPanel).not.toHaveBeenCalled();
    expect(mocks.setOrbClickThrough).not.toHaveBeenCalled();
  });

  it("存在等待用户的工具调用时同样拦截", async () => {
    useOrbStore.setState({ isExpanded: true });
    useChatStore.setState({ messages: [awaitingMessage()] });

    await autoCollapseOrb();

    expect(useOrbStore.getState().isExpanded).toBe(true);
    expect(mocks.collapseOrbPanel).not.toHaveBeenCalled();
  });

  it("空闲时正常收起并按配置恢复穿透", async () => {
    useOrbStore.setState({ isExpanded: true });

    await autoCollapseOrb();

    expect(useOrbStore.getState().isExpanded).toBe(false);
    expect(mocks.collapseOrbPanel).toHaveBeenCalledWith("right", "down");
    expect(mocks.setOrbClickThrough).toHaveBeenCalledWith(true);
  });

  it("已收起时直接返回，不调用任何 service 方法", async () => {
    await autoCollapseOrb();

    expect(mocks.collapseOrbPanel).not.toHaveBeenCalled();
    expect(mocks.setOrbClickThrough).not.toHaveBeenCalled();
  });
});

describe("显式收起不受门控", () => {
  it("busy 时 collapseOrbPanel 仍正常收起", async () => {
    useOrbStore.setState({ isExpanded: true });
    useChatStreamStore.setState({ isStreaming: true });

    await collapseOrbPanel();

    expect(useOrbStore.getState().isExpanded).toBe(false);
    expect(mocks.collapseOrbPanel).toHaveBeenCalledTimes(1);
  });
});
