import { describe, expect, it } from "vitest";
import { resolveNavigateMode } from "@/tools/framework/miniNavigate";

describe("mini 路由形态判断", () => {
  it("启动器与 miniFriendly 工具允许保持 mini", () => {
    expect(resolveNavigateMode("/")).toBe("mini");
    expect(resolveNavigateMode("/tools/clock")).toBe("mini");
  });

  it("不兼容工具与非工具路由要求 normal", () => {
    expect(resolveNavigateMode("/tools/json-viewer")).toBe("normal");
    expect(resolveNavigateMode("/tools/not-exist")).toBe("normal");
    expect(resolveNavigateMode("/settings/general")).toBe("normal");
    expect(resolveNavigateMode("/history")).toBe("normal");
  });
});
