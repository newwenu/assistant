import { PartitionOutlined } from "@ant-design/icons";
import type { FeatureModule } from "../types";

/**
 * 画布示例模块（§11.2）：示例内容挂侧栏入口，便于直接查看画布设施。
 * W2 落地时与 `demo/` 一并删除，并去掉 `registry.ts` 的装载段。
 */
export const canvasModule: FeatureModule = {
  id: "canvas-demo",
  order: 80,
  routes: [
    {
      path: "/canvas-demo",
      load: () =>
        import("./demo/CanvasDemoPage").then((m) => ({
          default: m.CanvasDemoPage,
        })),
      boundary: "画布示例",
      navParent: "canvas-demo",
      keywords: ["canvas", "画布", "demo"],
    },
  ],
  sidebar: [
    {
      key: "canvas-demo",
      label: "common.navigation.canvas_demo",
      icon: PartitionOutlined,
      slot: "after-history",
      order: 80,
    },
  ],
};
