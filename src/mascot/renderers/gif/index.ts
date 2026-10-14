import { registerRenderer } from "../../core/registry";
import { BUILTIN_RENDERER_IDS, asFace } from "../../core/types";
import { GifMascot } from "./GifMascot";

registerRenderer({
  meta: {
    id: BUILTIN_RENDERER_IDS.gif,
    label: "settings.mascot.renderer.gif.label",
    capabilities: {
      visemeSync: false,
      faceTransition: false,
      colorCustomization: false,
      idleAnimation: false,
    },
    limitations: "settings.mascot.renderer.gif.limitations",
    defaultColors: null,
    supportedFaces: [asFace("idle")],
    settings: [
      {
        key: "gif_border_radius",
        type: "slider",
        default: 50,
        label: "settings.mascot.renderer.gif.border_radius.label",
        min: 0,
        max: 50,
        extra: "settings.mascot.renderer.gif.border_radius.extra",
      },
      {
        key: "gif_src",
        type: "asset-picker",
        default: null,
        label: "settings.mascot.renderer.gif.gif_src.label",
        extra: "settings.mascot.renderer.gif.gif_src.extra",
        saveMessage: "settings.mascot.renderer.gif.gif_src.save_message",
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
