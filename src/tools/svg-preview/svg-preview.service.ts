const PARSERERROR_NS = "http://www.mozilla.org/newlayout/xml/parsererror.xml";

export const DEFAULT_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 200 200">
  <circle cx="100" cy="100" r="80" fill="#4f46e5" opacity="0.8"/>
  <circle cx="100" cy="100" r="50" fill="#818cf8" opacity="0.8"/>
  <circle cx="100" cy="100" r="20" fill="#c7d2fe"/>
</svg>`;

export function optimizeSvg(svg: string): string {
  return svg
    .replace(/<!--[\s\S]*?-->/g, "")
    .replace(/\s+/g, " ")
    .replace(/>\s+</g, "><")
    .replace(/\s+\/>/g, "/>")
    .trim();
}

export function svgToDataUrl(svg: string): string {
  const bytes = new TextEncoder().encode(svg);
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  const encoded = btoa(binary);
  return `data:image/svg+xml;base64,${encoded}`;
}

export function isValidSvg(svg: string): boolean {
  try {
    const parser = new DOMParser();
    const doc = parser.parseFromString(svg, "image/svg+xml");
    return (
      doc.getElementsByTagNameNS(PARSERERROR_NS, "parsererror").length === 0
    );
  } catch {
    return false;
  }
}
