import { registerRenderer } from "../../core/registry";
import { BUILTIN_RENDERER_IDS } from "../../core/types";
import { GifMascot } from "./GifMascot";

registerRenderer({
  meta: {
    id: BUILTIN_RENDERER_IDS.gif,
    label: "mascot_renderer.gif.label",
    capabilities: {
      visemeSync: false,
      faceTransition: false,
      colorCustomization: false,
      idleAnimation: false,
    },
    limitations: "mascot_renderer.gif.limitations",
    defaultColors: null,
    settings: [
      {
        key: "gif_border_radius",
        type: "slider",
        default: 50,
        label: "mascot_renderer.gif.border_radius.label",
        min: 0,
        max: 50,
        extra: "mascot_renderer.gif.border_radius.extra",
      },
      {
        key: "gif_src",
        type: "asset-picker",
        default: null,
        label: "mascot_renderer.gif.gif_src.label",
        extra: "mascot_renderer.gif.gif_src.extra",
        saveMessage: "mascot_renderer.gif.gif_src.save_message",
        assetConfig: {
          assetType: "gif",
          fileFilter: { name: "Image", extensions: ["gif", "png", "webp"] },
          urlInput: true,
        },
      },
    ],
  },
  component: GifMascot,
});
