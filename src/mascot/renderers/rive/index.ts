import { registerRenderer } from "../../core/registry";
import { BUILTIN_RENDERER_IDS } from "../../core/types";
import { RiveMascot } from "./RiveMascot";

registerRenderer({
  meta: {
    id: BUILTIN_RENDERER_IDS.rive,
    label: "Rive",
    capabilities: {
      visemeSync: true,
      faceTransition: true,
      colorCustomization: true,
      idleAnimation: true,
    },
    defaultColors: { primary: "#71c7df", secondary: "#e9f65c" },
    settings: [
      {
        key: "rive_idle_pose_rotation",
        type: "boolean",
        default: false,
        label: "空闲轮转动画",
      },
      {
        key: "rive_src",
        type: "asset-picker",
        default: null,
        label: "自定义 Rive 文件",
        extra: "支持 .riv 文件，替换内置 mascot",
        saveMessage: "Rive 文件已更新",
        assetConfig: {
          assetType: "rive",
          fileFilter: { name: "Rive", extensions: ["riv"] },
        },
      },
    ],
  },
  component: RiveMascot,
  dispose: () => {},
});
