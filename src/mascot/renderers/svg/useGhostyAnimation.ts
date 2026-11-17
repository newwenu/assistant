import type { RefObject } from "react";

import { useMascotAnimation } from "./useMascotAnimation";

import type { FacePreset } from "./facePresets";

interface GhostyAnimationRefs {
  groundRef: RefObject<SVGGElement | null>;
  bodyGroupRef: RefObject<SVGGElement | null>;
  dotGroupRef: RefObject<SVGGElement | null>;
  armGroupRef: RefObject<SVGGElement | null>;
  leftEyeRef: RefObject<SVGEllipseElement | null>;
  rightEyeRef: RefObject<SVGEllipseElement | null>;
  leftHighlightRef: RefObject<SVGCircleElement | null>;
  rightHighlightRef: RefObject<SVGCircleElement | null>;
}

interface GhostyAnimationOptions {
  animated: boolean;
  refs: GhostyAnimationRefs;
  preset: FacePreset;
  showArm: boolean;
  blinkMs: number;
}

/**
 * 逐帧写入浮动、光点挤压、手臂摆动与眨眼。
 * 所有目标都是 SVG 属性/样式，不经过 React 状态，避免每帧重渲染。
 */
export function useGhostyAnimation({
  animated,
  refs,
  preset,
  showArm,
  blinkMs,
}: GhostyAnimationOptions) {
  useMascotAnimation(animated, (t) => {
    const bob = Math.sin(t * Math.PI * 1.2) * 14;

    const g = refs.groundRef.current;
    if (g)
      g.setAttribute(
        "transform",
        `translate(500, 970) scale(${1 - bob / 600}, 1)`,
      );

    const bg = refs.bodyGroupRef.current;
    if (bg) bg.setAttribute("transform", `translate(0, ${bob})`);

    const dotPhase = t * Math.PI;
    const dotDx = Math.sin(dotPhase * 0.7) * 6;
    const dotDy = Math.sin(dotPhase) * 9;
    const press = Math.max(0, Math.sin(dotPhase));
    const dotSquashY = 1 - 0.08 * press;
    const dotSquashX = 1 + 0.05 * press;
    const dg = refs.dotGroupRef.current;
    if (dg)
      dg.setAttribute(
        "transform",
        `translate(${dotDx}, ${dotDy}) translate(520 240) scale(${dotSquashX} ${dotSquashY}) translate(-520 -240)`,
      );

    const wave = showArm ? Math.sin(t * Math.PI * 2.4) * 12 : 0;
    const ag = refs.armGroupRef.current;
    if (ag) ag.setAttribute("transform", `rotate(${wave} 820 590)`);

    const blinkOffset = blinkMs / 2;
    const tMs = t * 1000;
    const inBlink = (tMs + blinkOffset) % blinkMs < 200;
    const blinkScale = inBlink ? 0.12 : 1;

    const le = refs.leftEyeRef.current;
    if (le) le.setAttribute("ry", String(40 * preset.eyeScaleY * blinkScale));
    const re = refs.rightEyeRef.current;
    if (re) re.setAttribute("ry", String(40 * preset.eyeScaleY * blinkScale));

    const lh = refs.leftHighlightRef.current;
    if (lh) lh.style.display = inBlink ? "none" : "";
    const rh = refs.rightHighlightRef.current;
    if (rh) rh.style.display = inBlink ? "none" : "";
  });
}
