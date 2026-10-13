import { registerRenderer } from "../../core/registry";
import { BUILTIN_RENDERER_IDS } from "../../core/types";
import { RiveMascot } from "./RiveMascot";

registerRenderer({
  meta: {
    id: BUILTIN_RENDERER_IDS.rive,
    label: "mascot_renderer.rive.label",
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
        label: "mascot_renderer.rive.idle_pose_rotation",
      },
      {
        key: "rive_src",
        type: "asset-picker",
        default: null,
        label: "mascot_renderer.rive.custom_rive.label",
        extra: "mascot_renderer.rive.custom_rive.extra",
        saveMessage: "mascot_renderer.rive.custom_rive.save_message",
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
