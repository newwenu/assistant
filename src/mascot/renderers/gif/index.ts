import { registerRenderer } from "../../core/registry";
import { BUILTIN_RENDERER_IDS } from "../../core/types";
import { GifMascot } from "./GifMascot";

registerRenderer({
  meta: {
    id: BUILTIN_RENDERER_IDS.gif,
    label: "GIF",
    capabilities: {
      visemeSync: false,
      faceTransition: false,
      colorCustomization: false,
      idleAnimation: false,
    },
    limitations: "GIF 模式下助手不会响应对话状态变化",
    defaultColors: null,
  },
  component: GifMascot,
});
