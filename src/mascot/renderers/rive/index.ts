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
  },
  component: RiveMascot,
  dispose: () => {},
});
