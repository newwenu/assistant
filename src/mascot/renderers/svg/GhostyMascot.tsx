import { type FC, useMemo, useRef } from "react";

import type {
  MascotFace,
  MascotRendererProps,
  SvgRendererConfig,
} from "../../core/types";
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
import { useMascotAnimation } from "./useMascotAnimation";
import {
  resolveSvgViseme,
  visemePath,
  VISEMES,
  REST_SMILE_PATH,
} from "./visemes";

const DEFAULT_BODY_COLOR = "#3DD87E";
const BLUSH_COLOR = "#f4a3a3";
const INK_COLOR = "#0a0a0a";
const HIGHLIGHT_COLOR = "#ffffff";

interface FacePreset {
  eyeScaleY: number;
  eyeScaleX: number;
  browTilt: number;
  browDy: number;
  showBrows: boolean;
  blushOpacity: number;
}

const FACE_PRESETS: Record<MascotFace, FacePreset> = {
  sleep: {
    eyeScaleY: 0.1,
    eyeScaleX: 1,
    browTilt: 0,
    browDy: 2,
    showBrows: false,
    blushOpacity: 0.5,
  },
  idle: {
    eyeScaleY: 1,
    eyeScaleX: 1,
    browTilt: 0,
    browDy: 0,
    showBrows: false,
    blushOpacity: 0.85,
  },
  listening: {
    eyeScaleY: 1.05,
    eyeScaleX: 1.05,
    browTilt: -8,
    browDy: -10,
    showBrows: true,
    blushOpacity: 0.9,
  },
  thinking: {
    eyeScaleY: 0.7,
    eyeScaleX: 1,
    browTilt: -4,
    browDy: -2,
    showBrows: true,
    blushOpacity: 0.6,
  },
  confused: {
    eyeScaleY: 0.85,
    eyeScaleX: 0.95,
    browTilt: 14,
    browDy: -4,
    showBrows: true,
    blushOpacity: 0.55,
  },
  speaking: {
    eyeScaleY: 1,
    eyeScaleX: 1,
    browTilt: 0,
    browDy: 0,
    showBrows: false,
    blushOpacity: 0.95,
  },
  happy: {
    eyeScaleY: 0.45,
    eyeScaleX: 1.1,
    browTilt: -6,
    browDy: -6,
    showBrows: false,
    blushOpacity: 1,
  },
  concerned: {
    eyeScaleY: 0.95,
    eyeScaleX: 0.95,
    browTilt: 22,
    browDy: -2,
    showBrows: true,
    blushOpacity: 0.5,
  },
  curious: {
    eyeScaleY: 1.1,
    eyeScaleX: 1.05,
    browTilt: -10,
    browDy: -8,
    showBrows: true,
    blushOpacity: 0.8,
  },
  proud: {
    eyeScaleY: 0.55,
    eyeScaleX: 1.15,
    browTilt: -4,
    browDy: -4,
    showBrows: false,
    blushOpacity: 1,
  },
  cautious: {
    eyeScaleY: 0.9,
    eyeScaleX: 0.95,
    browTilt: 10,
    browDy: -3,
    showBrows: true,
    blushOpacity: 0.65,
  },
  celebrating: {
    eyeScaleY: 0.4,
    eyeScaleX: 1.15,
    browTilt: -8,
    browDy: -8,
    showBrows: false,
    blushOpacity: 1,
  },
  writing: {
    eyeScaleY: 0.75,
    eyeScaleX: 1,
    browTilt: -2,
    browDy: -1,
    showBrows: false,
    blushOpacity: 0.7,
  },
  reading: {
    eyeScaleY: 0.85,
    eyeScaleX: 1.05,
    browTilt: -6,
    browDy: -4,
    showBrows: true,
    blushOpacity: 0.75,
  },
  waving: {
    eyeScaleY: 0.5,
    eyeScaleX: 1.1,
    browTilt: -6,
    browDy: -6,
    showBrows: false,
    blushOpacity: 1,
  },
  dancing: {
    eyeScaleY: 0.4,
    eyeScaleX: 1.15,
    browTilt: -8,
    browDy: -8,
    showBrows: false,
    blushOpacity: 1,
  },
  working: {
    eyeScaleY: 0.75,
    eyeScaleX: 1,
    browTilt: -2,
    browDy: -1,
    showBrows: false,
    blushOpacity: 0.7,
  },
};

function restMouthPath(face: MascotFace): string {
  switch (face) {
    case "sleep":
      return "M496,588 Q520,593 544,588 Q520,592 496,588 Z";
    case "happy":
    case "celebrating":
    case "dancing":
    case "waving":
      return "M460,565 Q520,635 580,565 Q520,605 460,565 Z";
    case "concerned":
      return "M478,605 Q520,560 562,605 Q520,590 478,605 Z";
    case "confused":
      return "M478,580 Q520,610 562,575 Q520,597 478,580 Z";
    case "thinking":
    case "writing":
    case "reading":
    case "working":
      return "M488,585 Q520,595 552,585 Q520,592 488,585 Z";
    case "listening":
      return "M495,580 Q520,600 545,580 Q520,615 495,580 Z";
    case "curious":
      return "M500,578 Q520,598 540,578 Q520,610 500,578 Z";
    case "proud":
      return "M468,570 Q520,625 572,570 Q520,600 468,570 Z";
    case "cautious":
      return "M482,600 Q520,568 558,600 Q520,585 482,600 Z";
    default:
      return REST_SMILE_PATH;
  }
}

export const GhostyMascot: FC<MascotRendererProps> = ({
  face = "idle",
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
    face === "waving" || face === "celebrating" || face === "dancing";
  const blinkMs = face === "thinking" ? 4200 : 2600;

  const groundRef = useRef<SVGGElement>(null);
  const bodyGroupRef = useRef<SVGGElement>(null);
  const dotGroupRef = useRef<SVGGElement>(null);
  const armGroupRef = useRef<SVGGElement>(null);
  const leftEyeRef = useRef<SVGEllipseElement>(null);
  const rightEyeRef = useRef<SVGEllipseElement>(null);
  const leftHighlightRef = useRef<SVGCircleElement>(null);
  const rightHighlightRef = useRef<SVGCircleElement>(null);

  useMascotAnimation(animated, (t) => {
    const bob = Math.sin(t * Math.PI * 1.2) * 14;

    const g = groundRef.current;
    if (g) g.setAttribute("transform", `translate(500, 970) scale(${1 - bob / 600}, 1)`);

    const bg = bodyGroupRef.current;
    if (bg) bg.setAttribute("transform", `translate(0, ${bob})`);

    const dotPhase = t * Math.PI;
    const dotDx = Math.sin(dotPhase * 0.7) * 6;
    const dotDy = Math.sin(dotPhase) * 9;
    const press = Math.max(0, Math.sin(dotPhase));
    const dotSquashY = 1 - 0.08 * press;
    const dotSquashX = 1 + 0.05 * press;
    const dg = dotGroupRef.current;
    if (dg) dg.setAttribute("transform", `translate(${dotDx}, ${dotDy}) translate(520 240) scale(${dotSquashX} ${dotSquashY}) translate(-520 -240)`);

    const wave = showArm ? Math.sin(t * Math.PI * 2.4) * 12 : 0;
    const ag = armGroupRef.current;
    if (ag) ag.setAttribute("transform", `rotate(${wave} 820 590)`);

    const blinkOffset = blinkMs / 2;
    const tMs = t * 1000;
    const inBlink = (tMs + blinkOffset) % blinkMs < 200;
    const blinkScale = inBlink ? 0.12 : 1;

    const le = leftEyeRef.current;
    if (le) le.setAttribute("ry", String(40 * preset.eyeScaleY * blinkScale));
    const re = rightEyeRef.current;
    if (re) re.setAttribute("ry", String(40 * preset.eyeScaleY * blinkScale));

    const lh = leftHighlightRef.current;
    if (lh) lh.style.display = inBlink ? "none" : "";
    const rh = rightHighlightRef.current;
    if (rh) rh.style.display = inBlink ? "none" : "";
  });

  const idPrefix = "ghosty";
  const id = (k: string) => `${idPrefix}-${k}`;
  const bodyFill = `url(#${id("body")})`;
  const dotFill = `url(#${id("dot")})`;

  const viseme: VisemeShape = useMemo(() => {
    if (!visemeCode || face === "idle" || face === "sleep") return VISEMES.REST;
    return resolveSvgViseme(visemeCode);
  }, [visemeCode, face]);

  const mouthPath =
    face === "speaking" ? visemePath(viseme) : restMouthPath(face);
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
        width="100%"
        height="100%"
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

        <g ref={bodyGroupRef} transform="translate(0, 0)" filter={`url(#${id("drop")})`}>
          <g ref={dotGroupRef} transform="translate(0, 0) translate(520 240) scale(1 1) translate(-520 -240)">
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
            <circle ref={leftHighlightRef} cx={425} cy={501} r={7} fill={HIGHLIGHT_COLOR} />
            <circle ref={rightHighlightRef} cx={635} cy={501} r={7} fill={HIGHLIGHT_COLOR} />
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