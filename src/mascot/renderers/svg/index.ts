import { registerRenderer } from "../../core/registry";
import { BUILTIN_RENDERER_IDS } from "../../core/types";
import { GhostyMascot } from "./GhostyMascot";

registerRenderer({
  meta: {
    id: BUILTIN_RENDERER_IDS.svg,
    label: "settings.mascot.renderer.svg.label",
    capabilities: {
      visemeSync: true,
      faceTransition: true,
      colorCustomization: true,
      idleAnimation: true,
    },
    defaultColors: { primary: "#3dd17f", secondary: null },
    settings: [
      {
        key: "svg_animated",
        type: "boolean",
        default: true,
        label: "settings.mascot.renderer.svg.animated",
      },
    ],
  },
  component: GhostyMascot,
});
