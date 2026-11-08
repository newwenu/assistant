import { describe, it, expect } from "vitest";
import {
  DEFAULT_TOOL_CONFIG,
  diffToolConfig,
  isEmptyToolConfigDiff,
  resolveEffectiveToolConfig,
} from "../toolConfig";
import type { ConversationToolConfigDto } from "../../../types/protocol.contract";

function config(
  partial: Partial<ConversationToolConfigDto> = {},
): ConversationToolConfigDto {
  return {
    maxToolTier: "sandbox",
    approvalLevel: "dangerous_only",
    enabledTools: [],
    disabledTools: [],
    ...partial,
  };
}

describe("resolveEffectiveToolConfig", () => {
  it("会话级优先于模式级", () => {
    const resolved = resolveEffectiveToolConfig(
      config({ enabledTools: ["shell"] }),
      config({ enabledTools: ["read"] }),
    );
    expect(resolved.enabledTools).toEqual(["shell"]);
  });

  it("会话级缺失时回退模式级", () => {
    expect(
      resolveEffectiveToolConfig(null, config({ enabledTools: ["read"] }))
        .enabledTools,
    ).toEqual(["read"]);
  });

  it("两级都缺失时回退缺省值", () => {
    expect(resolveEffectiveToolConfig(null, undefined)).toEqual(
      DEFAULT_TOOL_CONFIG,
    );
  });

  it("返回副本，调用方改动不污染来源", () => {
    const source = config({ enabledTools: ["read"] });
    const resolved = resolveEffectiveToolConfig(source, null);
    resolved.enabledTools.push("shell");
    expect(source.enabledTools).toEqual(["read"]);
  });
});

describe("diffToolConfig", () => {
  it("只报告新增与移除的工具", () => {
    const diff = diffToolConfig(
      config({ enabledTools: ["read", "shell"] }),
      config({ enabledTools: ["read", "write"] }),
    );
    expect(diff.addedTools).toEqual(["write"]);
    expect(diff.removedTools).toEqual(["shell"]);
    expect(diff.tierChanged).toBeNull();
    expect(diff.levelChanged).toBeNull();
  });

  it("层级上限与审批级别变更时给出新值", () => {
    const diff = diffToolConfig(
      config({ approvalLevel: "none" }),
      config({ maxToolTier: "os", approvalLevel: "all_os_tools" }),
    );
    expect(diff.tierChanged).toBe("os");
    expect(diff.levelChanged).toBe("all_os_tools");
  });

  it("无变化时差异为空", () => {
    expect(isEmptyToolConfigDiff(diffToolConfig(config(), config()))).toBe(
      true,
    );
  });
});
