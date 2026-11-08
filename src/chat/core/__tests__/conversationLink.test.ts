import { describe, it, expect } from "vitest";
import {
  linkFromModeKind,
  resolveConversationRoute,
  type ModeLinkFields,
} from "../conversationLink";

function mode(partial: Partial<ModeLinkFields> & { id: string }) {
  return {
    kind: "agent",
    toolConfig: {
      maxToolTier: "os" as const,
      approvalLevel: "dangerous_only" as const,
      enabledTools: ["read"],
      disabledTools: [],
    },
    ...partial,
  };
}

describe("linkFromModeKind", () => {
  it("agent 归 agent 链路", () => {
    expect(linkFromModeKind("agent")).toBe("agent");
  });

  it("chat 与旧值、未知值、空值都归 chat 链路", () => {
    for (const kind of [
      "chat",
      "default",
      "quick",
      "deep",
      "custom",
      "unknown",
      null,
      undefined,
    ]) {
      expect(linkFromModeKind(kind)).toBe("chat");
    }
  });
});

describe("resolveConversationRoute", () => {
  it("modeId 为 null 时按 chat 处理且无工具配置", () => {
    const route = resolveConversationRoute(null, []);
    expect(route).toEqual({ link: "chat", modeId: null, toolConfig: null });
  });

  it("modeId 查不到 mode 行时按 chat 处理，但仍保留 modeId 供后端读取", () => {
    const route = resolveConversationRoute("mode-missing", []);
    expect(route.link).toBe("chat");
    expect(route.modeId).toBe("mode-missing");
    expect(route.toolConfig).toBeNull();
  });

  it("chat 模式即便工具名单非空也不下发工具配置", () => {
    const route = resolveConversationRoute("builtin-default", [
      mode({
        id: "builtin-default",
        kind: "chat",
        toolConfig: {
          maxToolTier: "os",
          approvalLevel: "dangerous_only",
          enabledTools: ["read"],
          disabledTools: [],
        },
      }),
    ]);
    expect(route.link).toBe("chat");
    expect(route.toolConfig).toBeNull();
  });

  it("旧 kind 值退化为标签，仍走 chat 链路", () => {
    const route = resolveConversationRoute("mode-legacy", [
      mode({
        id: "mode-legacy",
        kind: "deep",
        toolConfig: {
          maxToolTier: "os",
          approvalLevel: "dangerous_only",
          enabledTools: ["read", "write"],
          disabledTools: [],
        },
      }),
    ]);
    expect(route.link).toBe("chat");
    expect(route.toolConfig).toBeNull();
  });

  it("agent 模式下发模式级工具配置原值", () => {
    const route = resolveConversationRoute("mode-agent", [
      mode({
        id: "mode-agent",
        toolConfig: {
          maxToolTier: "os",
          approvalLevel: "dangerous_only",
          enabledTools: ["read", "write"],
          disabledTools: [],
        },
      }),
    ]);
    expect(route.link).toBe("agent");
    expect(route.toolConfig).toEqual({
      maxToolTier: "os",
      approvalLevel: "dangerous_only",
      enabledTools: ["read", "write"],
      disabledTools: [],
    });
  });

  it("agent 模式优先取会话持久工具配置", () => {
    const route = resolveConversationRoute(
      "mode-agent",
      [
        mode({
          id: "mode-agent",
          toolConfig: {
            maxToolTier: "sandbox",
            approvalLevel: "none",
            enabledTools: [],
            disabledTools: [],
          },
        }),
      ],
      {
        maxToolTier: "os",
        approvalLevel: "all_os_tools",
        enabledTools: ["read"],
        disabledTools: ["write"],
      },
    );
    expect(route.link).toBe("agent");
    expect(route.toolConfig).toEqual({
      maxToolTier: "os",
      approvalLevel: "all_os_tools",
      enabledTools: ["read"],
      disabledTools: ["write"],
    });
  });

  it("agent 模式在会话未配置时回退到模式初始值", () => {
    const route = resolveConversationRoute(
      "mode-agent",
      [
        mode({
          id: "mode-agent",
          toolConfig: {
            maxToolTier: "sandbox",
            approvalLevel: "none",
            enabledTools: [],
            disabledTools: [],
          },
        }),
      ],
      null,
    );
    expect(route.toolConfig).toEqual({
      maxToolTier: "sandbox",
      approvalLevel: "none",
      enabledTools: [],
      disabledTools: [],
    });
  });

  it("chat 模式即便会话有工具配置也不下发", () => {
    const route = resolveConversationRoute(
      "builtin-default",
      [mode({ id: "builtin-default", kind: "chat" })],
      {
        maxToolTier: "os",
        approvalLevel: "all_os_tools",
        enabledTools: ["read"],
        disabledTools: [],
      },
    );
    expect(route.link).toBe("chat");
    expect(route.toolConfig).toBeNull();
  });
});
