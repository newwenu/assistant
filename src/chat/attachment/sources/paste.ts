import type { ClipboardEvent } from "react";

export function extractPastedImages(e: ClipboardEvent): File[] {
  const files: File[] = [];
  for (const item of Array.from(e.clipboardData.items)) {
    if (item.type.startsWith("image/")) {
      const file = item.getAsFile();
      if (file) files.push(file);
    }
  }
  if (files.length > 0) return files;

  for (const file of Array.from(e.clipboardData.files)) {
    if (file.type.startsWith("image/")) files.push(file);
  }
  return files;
}
