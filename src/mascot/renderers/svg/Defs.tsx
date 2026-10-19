import { type FC } from "react";

import { shadeHex } from "../../core/colorUtils";
import { BODY_PATH } from "./paths";

export type GhostyVariant = "shaded" | "flat";

interface GhostyDefsProps {
  idPrefix: string;
  bodyColor: string;
  variant?: GhostyVariant;
}

export const GhostyDefs: FC<GhostyDefsProps> = ({
  idPrefix,
  bodyColor,
  variant = "shaded",
}) => {
  const id = (k: string) => `${idPrefix}-${k}`;
  const flat = variant === "flat";

  const hl = shadeHex(bodyColor, 0.45);
  const hi = shadeHex(bodyColor, 0.28);
  const mid = bodyColor;
  const lo = shadeHex(bodyColor, -0.18);
  const edge = shadeHex(bodyColor, -0.32);
  const deep = shadeHex(bodyColor, -0.45);

  return (
    <defs>
      {flat ? (
        <radialGradient id={id("body")} cx="0.35" cy="0.3" r="0.95">
          <stop offset="0%" stopColor={hi} />
          <stop offset="45%" stopColor={mid} />
          <stop offset="100%" stopColor={lo} />
        </radialGradient>
      ) : (
        <radialGradient id={id("body")} cx="0.32" cy="0.28" r="1.05">
          <stop offset="0%" stopColor={hl} />
          <stop offset="18%" stopColor={hi} />
          <stop offset="38%" stopColor={mid} />
          <stop offset="58%" stopColor={lo} />
          <stop offset="78%" stopColor={edge} />
          <stop offset="100%" stopColor={deep} />
        </radialGradient>
      )}
      {flat ? (
        <radialGradient id={id("dot")} cx="0.35" cy="0.3" r="1">
          <stop offset="0%" stopColor={hi} />
          <stop offset="55%" stopColor={mid} />
          <stop offset="100%" stopColor={lo} />
        </radialGradient>
      ) : (
        <radialGradient id={id("dot")} cx="0.35" cy="0.3" r="1">
          <stop offset="0%" stopColor={hl} />
          <stop offset="25%" stopColor={hi} />
          <stop offset="50%" stopColor={mid} />
          <stop offset="75%" stopColor={lo} />
          <stop offset="100%" stopColor={edge} />
        </radialGradient>
      )}
      <filter id={id("grain")} x="0%" y="0%" width="100%" height="100%">
        <feTurbulence
          type="fractalNoise"
          baseFrequency="0.9"
          numOctaves={2}
          stitchTiles="stitch"
          result="noise"
        />
        <feColorMatrix
          in="noise"
          type="matrix"
          values="0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 0.06 0"
        />
      </filter>
      <filter id={id("soft")} x="-30%" y="-30%" width="160%" height="160%">
        <feGaussianBlur stdDeviation={30} />
      </filter>
      <filter id={id("drop")} x="-20%" y="-20%" width="140%" height="160%">
        <feGaussianBlur in="SourceAlpha" stdDeviation={14} />
        <feOffset dx={0} dy={22} result="off" />
        <feComponentTransfer>
          <feFuncA type="linear" slope={0.45} />
        </feComponentTransfer>
        <feMerge>
          <feMergeNode />
          <feMergeNode in="SourceGraphic" />
        </feMerge>
      </filter>
      <radialGradient id={id("ground")} cx="0.5" cy="0.5" r="0.5">
        <stop offset="0%" stopColor="#000000" stopOpacity={0.35} />
        <stop offset="100%" stopColor="#000000" stopOpacity={0} />
      </radialGradient>
      <clipPath id={id("body-clip")}>
        <path d={BODY_PATH} />
      </clipPath>
    </defs>
  );
};
