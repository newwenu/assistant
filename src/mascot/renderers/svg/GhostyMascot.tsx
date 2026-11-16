import { type FC, useMemo, useRef } from "react";

import type { MascotRendererProps, SvgRendererConfig } from "../../core/types";
import { asFace } from "../../core/types";
import { argbToHex } from "../../core/colorUtils";
import type { VisemeShape } from "../../core/resolve";
import { GhostyDefs, type GhostyVariant } from "./Defs";
import {
  ARM_PATH,
  BODY_PATH,
  LEFT_LEG_PATH,
  RIGHT_LEG_PATH,
  VIEWBOX,
} from "./paths";
import { resolveSvgViseme, visemePath, VISEMES } from "./visemes";
import { FACE_PRESETS } from "./ghosty/facePresets";
import { restMouthPath } from "./ghosty/paths";
import { useGhostyAnimation } from "./ghosty/useGhostyAnimation";

const DEFAULT_BODY_COLOR = "#3DD87E";
const BLUSH_COLOR = "#f4a3a3";
const INK_COLOR = "#0a0a0a";
const HIGHLIGHT_COLOR = "#ffffff";

export const GhostyMascot: FC<MascotRendererProps> = ({
  face = asFace("idle"),
  size = "100%",
  visemeCode,
  primaryColor,
  rendererConfig,
}) => {
  const config = rendererConfig as SvgRendererConfig | undefined;
  const animated = config?.animated ?? true;
  const variant: GhostyVariant = config?.variant ?? "shaded";

  const bodyColor = argbToHex(primaryColor) ?? DEFAULT_BODY_COLOR;

  const preset = FACE_PRESETS[face] ?? FACE_PRESETS.idle;
  const showArm =
    face === asFace("waving") ||
    face === asFace("celebrating") ||
    face === asFace("dancing");
  const blinkMs = face === asFace("thinking") ? 4200 : 2600;

  const groundRef = useRef<SVGGElement>(null);
  const bodyGroupRef = useRef<SVGGElement>(null);
  const dotGroupRef = useRef<SVGGElement>(null);
  const armGroupRef = useRef<SVGGElement>(null);
  const leftEyeRef = useRef<SVGEllipseElement>(null);
  const rightEyeRef = useRef<SVGEllipseElement>(null);
  const leftHighlightRef = useRef<SVGCircleElement>(null);
  const rightHighlightRef = useRef<SVGCircleElement>(null);

  useGhostyAnimation({
    animated,
    refs: {
      groundRef,
      bodyGroupRef,
      dotGroupRef,
      armGroupRef,
      leftEyeRef,
      rightEyeRef,
      leftHighlightRef,
      rightHighlightRef,
    },
    preset,
    showArm,
    blinkMs,
  });

  const idPrefix = "ghosty";
  const id = (k: string) => `${idPrefix}-${k}`;
  const bodyFill = `url(#${id("body")})`;
  const dotFill = `url(#${id("dot")})`;

  const viseme: VisemeShape = useMemo(() => {
    if (!visemeCode || face === asFace("idle") || face === asFace("sleep"))
      return VISEMES.REST;
    return resolveSvgViseme(visemeCode);
  }, [visemeCode, face]);

  const mouthPath =
    face === asFace("speaking") ? visemePath(viseme) : restMouthPath(face);
  const sizeStyle = typeof size === "number" ? `${size}px` : size;

  return (
    <div
      style={{
        width: sizeStyle,
        height: sizeStyle,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
      data-face={face}
    >
      <svg
        width="97%"
        height="97%"
        viewBox={`0 0 ${VIEWBOX} ${VIEWBOX}`}
        style={{ overflow: "visible", display: "block" }}
      >
        <GhostyDefs
          idPrefix={idPrefix}
          bodyColor={bodyColor}
          variant={variant}
        />

        <g
          ref={groundRef}
          transform="translate(500, 970) scale(1, 1)"
          style={{ transformOrigin: "500px 970px" }}
        >
          <ellipse
            cx={0}
            cy={0}
            rx={260}
            ry={28}
            fill={`url(#${id("ground")})`}
          />
        </g>

        <g filter={`url(#${id("drop")})`}>
          <path d={LEFT_LEG_PATH} fill={bodyFill} />
          <path d={RIGHT_LEG_PATH} fill={bodyFill} />
        </g>

        <g
          ref={bodyGroupRef}
          transform="translate(0, 0)"
          filter={`url(#${id("drop")})`}
        >
          <g
            ref={dotGroupRef}
            transform="translate(0, 0) translate(520 240) scale(1 1) translate(-520 -240)"
          >
            <ellipse cx={520} cy={155} rx={92} ry={88} fill={dotFill} />
            <ellipse
              cx={490}
              cy={120}
              rx={24}
              ry={14}
              fill={HIGHLIGHT_COLOR}
              opacity={0.18}
            />
          </g>

          <g
            ref={armGroupRef}
            transform="rotate(0 820 590)"
            style={{ display: showArm ? "" : "none" }}
          >
            <path d={ARM_PATH} fill={bodyFill} />
          </g>

          <path d={BODY_PATH} fill={bodyFill} />

          <g clipPath={`url(#${id("body-clip")})`}>
            <g filter={`url(#${id("soft")})`}>
              <ellipse
                cx={340}
                cy={380}
                rx={220}
                ry={160}
                fill={HIGHLIGHT_COLOR}
                opacity={0.09}
              />
              <ellipse
                cx={720}
                cy={800}
                rx={280}
                ry={170}
                fill={INK_COLOR}
                opacity={variant === "flat" ? 0.12 : 0.22}
              />
            </g>
            <rect
              x={0}
              y={0}
              width={1000}
              height={1000}
              filter={`url(#${id("grain")})`}
            />
          </g>

          <ellipse
            cx={360}
            cy={545}
            rx={48}
            ry={22}
            fill={BLUSH_COLOR}
            opacity={0.85 * preset.blushOpacity}
          />
          <ellipse
            cx={680}
            cy={545}
            rx={48}
            ry={22}
            fill={BLUSH_COLOR}
            opacity={0.85 * preset.blushOpacity}
          />

          {preset.showBrows && (
            <g fill={INK_COLOR} data-face-brows={face}>
              <rect
                x={385}
                y={455 + preset.browDy}
                width={60}
                height={9}
                rx={4}
                transform={`rotate(${-preset.browTilt} 415 ${460 + preset.browDy})`}
              />
              <rect
                x={595}
                y={455 + preset.browDy}
                width={60}
                height={9}
                rx={4}
                transform={`rotate(${preset.browTilt} 625 ${460 + preset.browDy})`}
              />
            </g>
          )}

          <g>
            <ellipse
              ref={leftEyeRef}
              cx={415}
              cy={515}
              rx={30 * preset.eyeScaleX}
              ry={40 * preset.eyeScaleY}
              fill={INK_COLOR}
            />
            <ellipse
              ref={rightEyeRef}
              cx={625}
              cy={515}
              rx={30 * preset.eyeScaleX}
              ry={40 * preset.eyeScaleY}
              fill={INK_COLOR}
            />
            <circle
              ref={leftHighlightRef}
              cx={425}
              cy={501}
              r={7}
              fill={HIGHLIGHT_COLOR}
            />
            <circle
              ref={rightHighlightRef}
              cx={635}
              cy={501}
              r={7}
              fill={HIGHLIGHT_COLOR}
            />
          </g>

          <path d={mouthPath} fill={INK_COLOR} data-face={face} />

          <ellipse
            cx={360}
            cy={545}
            rx={56}
            ry={26}
            fill={BLUSH_COLOR}
            opacity={0.18 * preset.blushOpacity}
          />
          <ellipse
            cx={680}
            cy={545}
            rx={56}
            ry={26}
            fill={BLUSH_COLOR}
            opacity={0.18 * preset.blushOpacity}
          />
        </g>
      </svg>
    </div>
  );
};
