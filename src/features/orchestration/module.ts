import { DeploymentUnitOutlined } from "@ant-design/icons";
import type { FeatureModule } from "../types";

export const orchestrationModule: FeatureModule = {
  id: "orchestration",
  order: 75,
  routes: [
    {
      path: "/agent-orchestration",
      load: () =>
        import("./OrchestrationPage").then((m) => ({
          default: m.OrchestrationPage,
        })),
      boundary: "Agent 编排",
      navParent: "orchestration",
      keywords: ["agent", "orchestration", "canvas", "编排", "画布"],
    },
  ],
  sidebar: [
    {
      key: "orchestration",
      label: "common.navigation.agent_orchestration",
      icon: DeploymentUnitOutlined,
      slot: "before-history",
      order: 75,
    },
  ],
};
