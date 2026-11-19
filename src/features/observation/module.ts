import { NodeIndexOutlined } from "@ant-design/icons";
import type { FeatureModule } from "../types";

export const observationModule: FeatureModule = {
  id: "observation",
  order: 70,
  routes: [
    {
      path: "/agent-observation",
      load: () =>
        import("./ObservationPage").then((m) => ({
          default: m.ObservationPage,
        })),
      boundary: "Agent 观测",
      navParent: "observation",
      keywords: ["agent", "observation", "turn", "node", "观测", "节点"],
    },
  ],
  sidebar: [
    {
      key: "observation",
      label: "common.navigation.agent_observation",
      icon: NodeIndexOutlined,
      slot: "before-history",
      order: 70,
    },
  ],
};
