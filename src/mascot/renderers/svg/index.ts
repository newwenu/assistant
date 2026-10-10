import { registerRenderer } from "../../core/registry";
import { BUILTIN_RENDERER_IDS } from "../../core/types";
import { GhostyMascot } from "./GhostyMascot";

registerRenderer({
  meta: {
    id: BUILTIN_RENDERER_IDS.svg,
    label: "SVG",
    capabilities: {
      visemeSync: true,
      faceTransition: true,
      colorCustomization: true,
      idleAnimation: true,
    },
    defaultColors: { primary: "#3dd17f", secondary: null },
  },
  component: GhostyMascot,
});