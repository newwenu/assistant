import type { MascotConf } from "@/settings/core/types/confTypes";
import type { RendererConfig } from "./types";

export function buildRendererConfig(conf: MascotConf): RendererConfig {
  const renderer = conf.renderer ?? "svg";
  return {
    idlePoseRotation: conf.riveIdlePoseRotation,
    src: renderer === "rive" ? conf.riveSrc : conf.gifSrc,
    animated: conf.svgAnimated,
    borderRadius: conf.gifBorderRadius,
  };
}

export function buildMascotPatch(
  renderer: string,
  rendererConfig?: RendererConfig,
): Partial<MascotConf> {
  const patch: Partial<MascotConf> = {
    renderer: renderer,
  };
  if (rendererConfig) {
    if (
      "idlePoseRotation" in rendererConfig &&
      rendererConfig.idlePoseRotation !== undefined
    )
      patch.riveIdlePoseRotation = rendererConfig.idlePoseRotation;
    if ("animated" in rendererConfig && rendererConfig.animated !== undefined)
      patch.svgAnimated = rendererConfig.animated;
    if ("src" in rendererConfig && rendererConfig.src !== undefined) {
      if (renderer === "rive") patch.riveSrc = rendererConfig.src;
      else patch.gifSrc = rendererConfig.src;
    }
    if (
      "borderRadius" in rendererConfig &&
      rendererConfig.borderRadius !== undefined
    )
      patch.gifBorderRadius = rendererConfig.borderRadius;
  }
  return patch;
}
