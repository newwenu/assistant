const fs = require('fs');
const path = require('path');

const SIZE = 1024;
const CX = SIZE / 2;
const CY = SIZE / 2;
const OUTER_R = 460;
const INNER_R = 280;
const NUM_POINTS = 17;
const OUTER_CORNER_R = 42;
const INNER_CORNER_R = 16;
const FILL = '#00d992';

function starPoints(cx, cy, outerR, innerR, n) {
  const points = [];
  for (let i = 0; i < n * 2; i++) {
    const angle = (Math.PI * 2 * i) / (n * 2) - Math.PI / 2;
    const r = i % 2 === 0 ? outerR : innerR;
    points.push({
      x: cx + r * Math.cos(angle),
      y: cy + r * Math.sin(angle),
      isOuter: i % 2 === 0,
    });
  }
  return points;
}

function roundedStarPath(points, outerCornerR, innerCornerR) {
  const n = points.length;
  const corners = [];

  for (let i = 0; i < n; i++) {
    const prev = points[(i - 1 + n) % n];
    const curr = points[i];
    const next = points[(i + 1) % n];

    const r = curr.isOuter ? outerCornerR : innerCornerR;

    const d1x = prev.x - curr.x;
    const d1y = prev.y - curr.y;
    const len1 = Math.sqrt(d1x * d1x + d1y * d1y);
    const u1x = d1x / len1;
    const u1y = d1y / len1;

    const d2x = next.x - curr.x;
    const d2y = next.y - curr.y;
    const len2 = Math.sqrt(d2x * d2x + d2y * d2y);
    const u2x = d2x / len2;
    const u2y = d2y / len2;

    const dot = u1x * u2x + u1y * u2y;
    const alpha = Math.acos(Math.max(-1, Math.min(1, dot)));
    const halfAlpha = alpha / 2;

    const cross = u1x * u2y - u1y * u2x;

    const tanHalf = Math.tan(halfAlpha);
    let t = r / tanHalf;
    const maxT = Math.min(len1, len2) * 0.45;
    t = Math.min(t, maxT);
    const actualR = t * tanHalf;

    const t1x = curr.x + t * u1x;
    const t1y = curr.y + t * u1y;
    const t2x = curr.x + t * u2x;
    const t2y = curr.y + t * u2y;

    const sweepFlag = cross < 0 ? 1 : 0;

    corners.push({ t1x, t1y, t2x, t2y, r: actualR, sweepFlag });
  }

  let d = `M ${corners[0].t1x.toFixed(2)} ${corners[0].t1y.toFixed(2)}`;

  for (let i = 0; i < n; i++) {
    const c = corners[i];
    d += ` A ${c.r.toFixed(2)} ${c.r.toFixed(2)} 0 0 ${c.sweepFlag} ${c.t2x.toFixed(2)} ${c.t2y.toFixed(2)}`;
    const next = corners[(i + 1) % n];
    d += ` L ${next.t1x.toFixed(2)} ${next.t1y.toFixed(2)}`;
  }

  d += ' Z';
  return d;
}

const points = starPoints(CX, CY, OUTER_R, INNER_R, NUM_POINTS);
const pathD = roundedStarPath(points, OUTER_CORNER_R, INNER_CORNER_R);

const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${SIZE}" height="${SIZE}" viewBox="0 0 ${SIZE} ${SIZE}">
  <path d="${pathD}" fill="${FILL}"/>
  <text
    x="${CX}"
    y="${CY + 10}"
    text-anchor="middle"
    dominant-baseline="central"
    font-family="system-ui, -apple-system, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif"
    font-size="340"
    font-weight="700"
    fill="white"
    letter-spacing="-8"
  >A</text>
</svg>`;

const outPath = path.join(__dirname, 'app-icon.svg');
fs.writeFileSync(outPath, svg, 'utf8');
console.log(`Generated: ${outPath}`);