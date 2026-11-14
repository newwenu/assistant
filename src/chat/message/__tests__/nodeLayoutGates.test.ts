import { describe, expect, it } from "vitest";
import { readRepoFile } from "@/test-utils/paths";

/**
 * I46 门禁的源码级断言（规格 §9.2 门禁 6、8、9、11、16）：
 * 这些约束是「不许做什么」，只能靠静态检查守住，运行期无法覆盖。
 */

const GROUP_PROJECTION = "src/chat/message/groupNodeRows.ts";
const TURN_GROUP = "src/chat/message/TurnGroup.tsx";
const TURN_TAIL = "src/chat/message/TurnTail.tsx";
const TOOL_RENDERER = "src/chat/tool/ToolCallRenderer.tsx";
const MESSAGE_LIST = "src/chat/message/MessageList.tsx";

function code(relative: string): string {
  return readRepoFile(relative)
    .split("\n")
    .filter((line) => !line.trimStart().startsWith("*"))
    .filter((line) => !line.trimStart().startsWith("//"))
    .join("\n");
}

describe("I46 门禁 16：link 只读", () => {
  it("分组投影不对 link 做任何条件分支", () => {
    const source = code(GROUP_PROJECTION);
    expect(source).not.toMatch(/link\s*===/);
    expect(source).not.toMatch(/link\s*!==/);
    expect(source).not.toMatch(/switch\s*\(\s*link/);
  });
});

describe("I46 门禁 11：工具选路不看身份", () => {
  it("ToolCallRenderer 不读 role / modeId / TurnKind / 工具名分支", () => {
    const source = code(TOOL_RENDERER);
    expect(source).not.toMatch(/\brole\b/);
    expect(source).not.toMatch(/\bmodeId\b/);
    expect(source).not.toMatch(/\bTurnKind\b/);
    expect(source).not.toMatch(/view\.name\s*===/);
    expect(source).not.toMatch(/switch\s*\(\s*view\.name/);
  });
});

describe("I46 门禁 8、9：密度降级不读正文", () => {
  it("ToolCallRenderer 不解析结果正文、不现场统计", () => {
    const source = code(TOOL_RENDERER);
    expect(source).not.toMatch(/parsePersistedOutput/);
    expect(source).not.toMatch(/resultStats/);
    expect(source).not.toMatch(/shouldCollapseResult/);
    expect(source).not.toMatch(/resultText/);
    expect(source).not.toMatch(/ReactMarkdown/);
  });

  it("分组投影只读状态与时间，不读工具结果正文", () => {
    const source = code(GROUP_PROJECTION);
    expect(source).not.toMatch(/resultText/);
    expect(source).not.toMatch(/parsePersistedOutput/);
  });

  it("折叠判据只读段数与 status，不读 modeId（compact 门禁 14）", () => {
    const source = code(GROUP_PROJECTION);
    expect(source).not.toMatch(/\bmodeId\b/);
  });
});

describe("I46 门禁 6：折叠态不由计时器或后置标志驱动", () => {
  it("分组投影与容器组件都不使用定时器", () => {
    for (const file of [GROUP_PROJECTION, TURN_GROUP, TURN_TAIL]) {
      const source = code(file);
      expect(source).not.toMatch(
        /setTimeout|setInterval|requestAnimationFrame/,
      );
    }
  });
});

describe("I46 分页常量单一来源", () => {
  it("前端不重复定义 TURNS_PER_PAGE", () => {
    const source = readRepoFile(MESSAGE_LIST);
    expect(source).not.toMatch(/TURNS_PER_PAGE/);
  });
});
