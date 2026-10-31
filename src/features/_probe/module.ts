import { ExperimentOutlined } from "@ant-design/icons";
import type { FeatureModule } from "../types";
import type { SettingSectionDef } from "@/settings/core/field/fieldRegistry";
import type { ExtensionSettingDefinition } from "@/settings/core/types/extension";
import { probeConfSlot } from "./conf";

const PROBE_SECTIONS: SettingSectionDef[] = [
  {
    id: "probe-general",
    pageId: "probe",
    title: "探针设置",
    keywords: ["probe", "探针"],
  },
];

const PROBE_FIELDS: ExtensionSettingDefinition[] = [
  {
    key: "probeFlag",
    group: "probe",
    section: "probe-general",
    type: "boolean",
    default: false,
    label: "探针开关",
  },
  {
    key: "probeText",
    group: "probe",
    section: "probe-general",
    type: "string",
    default: "",
    label: "探针文本",
    placeholder: "任意文本",
  },
];

export const probeModule: FeatureModule = {
  id: "probe",
  order: 66,
  ipc: { namespace: "probe" },
  routes: [
    {
      path: "/probe",
      load: () => import("./ProbePage").then((m) => ({ default: m.ProbePage })),
      boundary: "探针页面",
      navParent: "probe",
      keywords: ["probe", "探针"],
    },
  ],
  sidebar: [
    {
      key: "probe",
      label: "探针",
      icon: ExperimentOutlined,
      slot: "after-tools",
    },
  ],
  categories: [{ id: "probe", label: "探针", order: 21 }],
  settings: {
    pages: [
      {
        id: "probe",
        categoryId: "probe",
        label: "探针设置",
        keywords: ["probe", "探针"],
        icon: ExperimentOutlined,
        order: 66,
        load: () =>
          import("./settings/ProbeSettingsPage").then((m) => ({
            default: m.ProbeSettingsPage,
          })),
      },
    ],
    sections: PROBE_SECTIONS,
    fields: PROBE_FIELDS,
  },
  confs: [probeConfSlot],
};
