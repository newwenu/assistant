import { registerRenderer } from "../../core/registry";
import { BUILTIN_RENDERER_IDS, BUILTIN_FACES, asFace } from "../../core/types";
import { RiveManifestEditor } from "@/settings/shared/renderers/RiveManifestEditor";
import { RiveMascot } from "./RiveMascot";

registerRenderer({
  meta: {
    id: BUILTIN_RENDERER_IDS.rive,
    label: "settings.mascot.renderer.rive.label",
    capabilities: {
      visemeSync: true,
      faceTransition: true,
      colorCustomization: true,
      idleAnimation: true,
    },
    defaultColors: { primary: "#71c7df", secondary: "#e9f65c" },
    supportedFaces: BUILTIN_FACES.map(asFace),
    settings: [
      {
        key: "riveIdlePoseRotation",
        type: "boolean",
        default: false,
        label: "settings.mascot.renderer.rive.idle_pose_rotation",
      },
      {
        key: "riveSrc",
        type: "asset-picker",
        default: null,
        label: "settings.mascot.renderer.rive.custom_rive.label",
        extra: "settings.mascot.renderer.rive.custom_rive.extra",
        saveMessage: "settings.mascot.renderer.rive.custom_rive.save_message",
        assetConfig: {
          assetType: "rive",
          fileFilter: { name: "Rive", extensions: ["riv"] },
          manifestEditor: RiveManifestEditor,
        },
      },
    ],
  },
  component: RiveMascot,
  dispose: () => {},
});
