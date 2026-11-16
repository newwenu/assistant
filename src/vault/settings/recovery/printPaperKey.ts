export interface PaperKeyPrintContent {
  title: string;
  warning: string;
  groups: string[];
  meta: string;
  footer: string;
}

const PRINT_CSS = `
@page { size: A4; margin: 18mm; }
body { font-family: system-ui, sans-serif; color: #111; }
h1 { font-size: 18px; margin: 0 0 12px; }
.warning { border: 2px solid #111; padding: 10px 12px; font-size: 12px; line-height: 1.6; }
.groups { margin: 20px 0; font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 22px; letter-spacing: 3px; }
.groups div { margin: 8px 0; }
.no { display: inline-block; width: 24px; opacity: 0.55; font-size: 13px; }
.meta, .footer { font-size: 12px; line-height: 1.6; }
`;

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export function buildPaperKeyPrintBody(content: PaperKeyPrintContent): string {
  const rows = content.groups
    .map(
      (group, index) =>
        `<div><span class="no">${index + 1}</span>${escapeHtml(group)}</div>`,
    )
    .join("");

  return `<h1>${escapeHtml(content.title)}</h1>
<div class="warning">${escapeHtml(content.warning)}</div>
<div class="groups">${rows}</div>
<div class="meta">${escapeHtml(content.meta)}</div>
<div class="footer">${escapeHtml(content.footer)}</div>`;
}

export function buildPaperKeyPrintHtml(content: PaperKeyPrintContent): string {
  return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<title>${escapeHtml(content.title)}</title>
<style>${PRINT_CSS}</style>
</head>
<body>
${buildPaperKeyPrintBody(content)}
</body>
</html>`;
}

export function printPaperKey(content: PaperKeyPrintContent): void {
  const frame = document.createElement("iframe");
  frame.setAttribute("aria-hidden", "true");
  frame.setAttribute("tabindex", "-1");
  frame.style.cssText =
    "position:fixed;right:0;bottom:0;width:0;height:0;border:0;visibility:hidden;";

  const cleanup = () => {
    if (frame.parentNode) frame.parentNode.removeChild(frame);
  };

  frame.srcdoc = buildPaperKeyPrintHtml(content);
  frame.addEventListener("load", () => {
    const win = frame.contentWindow;
    if (!win) {
      cleanup();
      return;
    }
    win.onafterprint = cleanup;
    win.focus();
    win.print();
    window.setTimeout(cleanup, 10_000);
  });

  document.body.appendChild(frame);
}
