export interface TextStats {
  charCount: number;
  wordCount: number;
  lineCount: number;
  byteSize: number;
  cjkCount: number;
  sentenceCount: number;
  paragraphCount: number;
}

export function computeTextStats(text: string): TextStats {
  const charCount = text.length;
  const wordCount = text.trim() === "" ? 0 : text.trim().split(/\s+/).length;
  const lineCount = text === "" ? 0 : text.split(/\r\n|\r|\n/).length;
  const byteSize = new TextEncoder().encode(text).length;
  const cjkCount = (text.match(/[\u4e00-\u9fff\u3400-\u4dbf]/g) || []).length;
  const sentenceCount =
    text === "" ? 0 : (text.match(/[.!?。！？]+/g) || []).length;
  const paragraphCount =
    text === ""
      ? 0
      : text.split(/\n\s*\n/).filter((p) => p.trim()).length ||
        (text.trim() ? 1 : 0);
  return {
    charCount,
    wordCount,
    lineCount,
    byteSize,
    cjkCount,
    sentenceCount,
    paragraphCount,
  };
}

export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
}
