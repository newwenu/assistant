import { type FC, useEffect, useRef } from "react";

import type { MascotRendererId, MascotRendererProps } from "./core/types";
import { getRenderer } from "./core/registry";
import { BUILTIN_RENDERER_IDS, asFace } from "./core/types";
import { createLogger } from "@/services/system/logger";

const log = createLogger("MascotRenderer");

import "./renderers/rive";
import "./renderers/svg";
import "./renderers/gif";

interface Props extends MascotRendererProps {
  renderer: MascotRendererId;
}

export const MascotRenderer: FC<Props> = (props) => {
  const prevRendererRef = useRef<MascotRendererId | null>(null);

  useEffect(() => {
    if (prevRendererRef.current && prevRendererRef.current !== props.renderer) {
      const oldEntry = getRenderer(prevRendererRef.current);
      oldEntry?.dispose?.();
    }
    prevRendererRef.current = props.renderer;
  }, [props.renderer]);

  if (props.renderer === BUILTIN_RENDERER_IDS.disabled) return null;

  const entry = getRenderer(props.renderer);
  if (!entry) {
    log.warn(`unknown renderer "${props.renderer}", falling back to svg`);
    const fallback = getRenderer(BUILTIN_RENDERER_IDS.svg);
    if (!fallback) return null;
    const Component = fallback.component;
    return (
      <Component
        face={props.face}
        size={props.size}
        visemeCode={props.visemeCode}
        primaryColor={props.primaryColor}
        secondaryColor={props.secondaryColor}
        rendererConfig={props.rendererConfig}
      />
    );
  }

  const Component = entry.component;
  const { capabilities, supportedFaces } = entry.meta;

  const effectiveFace =
    !supportedFaces || supportedFaces.includes(props.face)
      ? props.face
      : asFace("idle");

  return (
    <Component
      face={effectiveFace}
      size={props.size}
      visemeCode={capabilities.visemeSync ? props.visemeCode : undefined}
      primaryColor={
        capabilities.colorCustomization ? props.primaryColor : undefined
      }
      secondaryColor={
        capabilities.colorCustomization ? props.secondaryColor : undefined
      }
      rendererConfig={props.rendererConfig}
    />
  );
};
