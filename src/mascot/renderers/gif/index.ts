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
    settings: [
      {
        key: "gif_border_radius",
        type: "slider",
        default: 50,
        label: "圆角",
        min: 0,
        max: 50,
        extra: "0 = 方形，50 = 圆形",
      },
      {
        key: "gif_src",
        type: "asset-picker",
        default: null,
        label: "GIF 来源",
        extra: "支持 GIF/PNG/WebP 文件上传，或 URL、本地路径、data URI",
        saveMessage: "GIF 来源已更新",
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
