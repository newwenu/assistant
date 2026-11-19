import { describe, expect, it } from "vitest";
import type {
  AgentNodeDto,
  NodeStateDto,
} from "../../../../types/protocol.contract";
import {
  AGENT_NODE_IDS,
  AGENT_NODE_TEMPLATE,
  isAgentNodeId,
  isNodeState,
  NODE_STATES,
  nodesAfterInIteration,
  templateOrderOf,
  TOOL_BRANCH_NODE_IDS,
} from "../template";

describe("template — 12 节点静态模板（G2 / I55 §3.1）", () => {
  it("G2：节点集合与模板一一对应，顺序 = order，无重复", () => {
    expect(AGENT_NODE_TEMPLATE).toHaveLength(12);
    expect(AGENT_NODE_IDS).toHaveLength(12);
    expect(new Set(AGENT_NODE_IDS).size).toBe(12);
    AGENT_NODE_TEMPLATE.forEach((entry, index) => {
      expect(entry.order).toBe(index);
      expect(AGENT_NODE_IDS[index]).toBe(entry.id);
    });
  });

  it("G2：模板 id 集合与契约生成类型的 key 集合相等（编译期锁 + 运行时核对）", () => {
    // 真正的锁在 template.ts 的 `satisfies Record<AgentNodeDto, …>`：缺项 / 多项都编译失败。
    // 运行时这一条防的是「有人绕过该约束用 as 断言改了常量」。
    const contractKeys: AgentNodeDto[] = [
      "admit",
      "prepare",
      "brake",
      "ladder",
      "model",
      "persist-assistant",
      "branch",
      "budget-split",
      "pipeline",
      "after-tool",
      "persist-tools",
      "after-iteration",
    ];
    expect(new Set(AGENT_NODE_IDS)).toEqual(new Set(contractKeys));
    expect(contractKeys.every(isAgentNodeId)).toBe(true);
  });

  it("7 值状态闭集与守卫（未知取值不建格子，I55 §4.8 第 3 条）", () => {
    expect(NODE_STATES).toHaveLength(7);
    const allStates: NodeStateDto[] = [
      "pending",
      "running",
      "waiting",
      "finished",
      "error",
      "skipped",
      "cancelled",
    ];
    expect(new Set(NODE_STATES)).toEqual(new Set(allStates));
    expect(isNodeState("orchestrating")).toBe(false);
    expect(isAgentNodeId("orchestrator")).toBe(false);
  });

  it("迭代外节点只有 admit / prepare（iteration 恒 0，I55 §5.4 第 1 条）", () => {
    const outside = AGENT_NODE_TEMPLATE.filter((entry) => !entry.inIteration);
    expect(outside.map((entry) => entry.id)).toEqual(["admit", "prepare"]);
  });

  it("pending 推导目标 = 同迭代中排在后面的节点；branch 之后恰是 ⑧–⑫", () => {
    expect(nodesAfterInIteration("branch")).toEqual([...TOOL_BRANCH_NODE_IDS]);
    expect(nodesAfterInIteration("after-iteration")).toEqual([]);
    expect(nodesAfterInIteration("admit")).toEqual(
      AGENT_NODE_TEMPLATE.filter((entry) => entry.inIteration).map(
        (entry) => entry.id,
      ),
    );
    expect(templateOrderOf("orchestrator")).toBeNull();
  });
});
