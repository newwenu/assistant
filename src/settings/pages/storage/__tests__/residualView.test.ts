import { describe, expect, it } from "vitest";
import { residualView } from "../residualView";
import {
  makeChild,
  makeInventory,
  makeNode,
  makeScopeTotals,
  makeTotals,
} from "./fixtures";

describe("对账余量视图模型", () => {
  it("未登记优先，其余按类别序，同类别按余量降序且 null 置末", () => {
    const view = residualView(
      makeInventory({
        nodes: [
          makeNode({
            name: "declared-big",
            class: "declared",
            residualBytes: 900,
          }),
          makeNode({
            name: "ignored-small",
            class: "ignored",
            residualBytes: 10,
          }),
          makeNode({
            name: "companion-null",
            class: "companion",
            residualBytes: null,
          }),
          makeNode({
            name: "unregistered-small",
            class: "unregistered",
            residualBytes: 20,
          }),
          makeNode({
            name: "unregistered-big",
            class: "unregistered",
            residualBytes: 800,
          }),
          makeNode({
            name: "optional",
            class: "optionalNotMounted",
            residualBytes: 40,
          }),
          makeNode({
            name: "companion",
            class: "companion",
            residualBytes: 30,
          }),
        ],
      }),
    );

    expect(view.rows.map((row) => row.name)).toEqual([
      "unregistered-big",
      "unregistered-small",
      "optional",
      "companion",
      "companion-null",
      "ignored-small",
      "declared-big",
    ]);
  });

  it("同类别同余量时按名称稳定排序", () => {
    const view = residualView(
      makeInventory({
        nodes: [
          makeNode({ name: "b", residualBytes: 10 }),
          makeNode({ name: "a", residualBytes: 10 }),
        ],
      }),
    );
    expect(view.rows.map((row) => row.name)).toEqual(["a", "b"]);
  });

  it("declared 节点仅在余量大于 0 时出现", () => {
    const view = residualView(
      makeInventory({
        nodes: [
          makeNode({ name: "zero", class: "declared", residualBytes: 0 }),
          makeNode({ name: "positive", class: "declared", residualBytes: 5 }),
          makeNode({ name: "unknown", class: "declared", residualBytes: null }),
          makeNode({
            name: "not-declared-zero",
            class: "ignored",
            residualBytes: 0,
          }),
        ],
      }),
    );
    expect(view.rows.map((row) => row.name)).toEqual([
      "not-declared-zero",
      "positive",
    ]);
  });

  it("declared 之外的类别登记列显示占位符", () => {
    const view = residualView(
      makeInventory({
        nodes: [
          makeNode({ name: "stray", declaredBytes: 999 }),
          makeNode({
            name: "declared",
            class: "declared",
            residualBytes: 5,
            declaredBytes: 100,
          }),
        ],
      }),
    );
    const stray = view.rows.find((row) => row.name === "stray");
    const declared = view.rows.find((row) => row.name === "declared");
    expect(stray?.declaredText).toBe("—");
    expect(declared?.declaredText).toBe("100 B");
  });

  it("partial 生成警示，未覆盖节点最多列三个并附余下数量", () => {
    const view = residualView(
      makeInventory({
        partial: true,
        truncatedNodes: ["a", "b", "c", "d", "e"],
      }),
    );
    const notice = view.notices.find((item) => item.level === "warning");
    expect(notice).toBeDefined();
    expect(notice?.detail?.startsWith("a, b, c")).toBe(true);
    expect(notice?.detail).toContain("2");
  });

  it("partial 仅由 overRegistered 触发时不冒充「只扫描了一部分目录」", () => {
    const view = residualView(
      makeInventory({
        partial: true,
        truncatedNodes: [],
        nodes: [makeNode({ name: "EBWebView", overRegistered: true })],
      }),
    );
    expect(
      view.notices.filter((item) => item.level === "warning"),
    ).toHaveLength(0);
    const error = view.notices.find((item) => item.level === "error");
    expect(error).toBeDefined();
  });

  it("任一节点超登记时整段标红并保留该行标记", () => {
    const view = residualView(
      makeInventory({
        nodes: [
          makeNode({ name: "ok" }),
          makeNode({ name: "over", overRegistered: true }),
        ],
      }),
    );
    const error = view.notices.find((item) => item.level === "error");
    expect(error).toBeDefined();
    expect(view.rows.find((row) => row.name === "over")?.overRegistered).toBe(
      true,
    );
    expect(view.rows.find((row) => row.name === "ok")?.overRegistered).toBe(
      false,
    );
  });

  it("扫描失败保留上次数字并追加提示", () => {
    const view = residualView(
      makeInventory({
        status: "failed",
        scannedAt: 1000,
        failedAt: 2000,
        nodes: [makeNode({ name: "kept" })],
        unregisteredBytes: 512,
        unregisteredCount: 2,
      }),
    );
    expect(view.rows).toHaveLength(1);
    expect(view.notices.some((item) => item.level === "warning")).toBe(true);
    expect(view.totalsLines[0]?.text).not.toBe("—");
  });

  it("扫描失败且没有任何数字时不提示「保留上次」", () => {
    const view = residualView(
      makeInventory({
        status: "failed",
        nodes: [],
        unregisteredBytes: null,
        unregisteredCount: null,
      }),
    );
    expect(view.notices).toEqual([]);
    expect(view.empty).toBe(true);
  });

  it("合计为空时不填 0", () => {
    const view = residualView(
      makeInventory({
        unregisteredBytes: null,
        unregisteredCount: 3,
        totals: makeTotals({
          scanBytes: null,
          scanCount: null,
          residualBytes: null,
          residualCount: null,
          data: makeScopeTotals({
            scanBytes: null,
            residualBytes: null,
            unnameableBytes: null,
          }),
          local: makeScopeTotals({
            scanBytes: null,
            residualBytes: null,
            unnameableBytes: null,
          }),
        }),
      }),
    );
    expect(view.totalsLines.every((line) => line.text.includes("—"))).toBe(
      true,
    );
    expect(view.totalsHint.length).toBeGreaterThan(0);
  });

  it("合计三段只读 totals，且 partial 时三段统一加下界标注", () => {
    const view = residualView(
      makeInventory({
        unregisteredBytes: 512,
        unregisteredCount: 2,
        totals: makeTotals({
          scanBytes: 2048,
          residualBytes: 1024,
          partial: true,
        }),
      }),
    );
    expect(view.totalsLines.map((line) => line.key)).toEqual([
      "total",
      "residual",
      "unregistered",
    ]);
    expect(view.totalsLines.every((line) => line.text.includes("≥"))).toBe(
      true,
    );
    expect(view.totalsHint).toContain("≥");
  });

  it("不可命名部分只在合计区追加说明，不进入节点列表", () => {
    const view = residualView(
      makeInventory({
        totals: makeTotals({
          data: makeScopeTotals({
            unnameableBytes: 64,
            unnameableCount: 1,
          }),
          local: makeScopeTotals({
            unnameableBytes: 32,
            unnameableCount: 2,
          }),
        }),
      }),
    );
    expect(view.unnameableText).not.toBeNull();
    expect(view.unnameableText).toContain("3");
    expect(view.rows).toHaveLength(0);
  });

  it("没有不可命名条目时不追加说明", () => {
    const view = residualView(makeInventory());
    expect(view.unnameableText).toBeNull();
  });

  it("五态文案随状态切换", () => {
    const neverRun = residualView(makeInventory({ status: "neverRun" }));
    const running = residualView(makeInventory({ status: "running" }));
    const stale = residualView(makeInventory({ status: "stale" }));
    const failed = residualView(
      makeInventory({ status: "failed", failedAt: 2000 }),
    );

    expect(neverRun.statusText).not.toBe(running.statusText);
    expect(stale.statusTone).toBe("warning");
    expect(failed.statusTone).toBe("error");
    expect(neverRun.statusTone).toBe("neutral");
  });

  it("children 最多展开 20 项，超出提示数量", () => {
    const children = Array.from({ length: 23 }, (_, i) =>
      makeChild({ name: `c${i}`, bytes: i * 10, count: i }),
    );
    const view = residualView(
      makeInventory({ nodes: [makeNode({ name: "many", children })] }),
    );
    const row = view.rows[0];
    expect(row.children).toHaveLength(20);
    expect(row.childrenMoreText ?? "").toContain("3");
    expect(row.expandable).toBe(true);
  });

  it("children 的条数不可得时不拼「— 项」", () => {
    const view = residualView(
      makeInventory({
        nodes: [
          makeNode({ name: "one", children: [makeChild({ count: null })] }),
        ],
      }),
    );
    expect(view.rows[0].children[0].text).toBe("128 B");
  });
});
