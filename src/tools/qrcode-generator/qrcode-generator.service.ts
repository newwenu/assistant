import QRCode from "qrcode";
import type { QRCodeErrorCorrectionLevel } from "qrcode";

export const ERROR_LEVELS: {
  value: QRCodeErrorCorrectionLevel;
  label: string;
}[] = [
  { value: "L", label: "低 (L ~7%)" },
  { value: "M", label: "中 (M ~15%)" },
  { value: "Q", label: "较高 (Q ~25%)" },
  { value: "H", label: "高 (H ~30%)" },
];

export async function generateQrDataUrl(
  text: string,
  foreground: string,
  background: string,
  errorLevel: QRCodeErrorCorrectionLevel,
): Promise<string> {
  return QRCode.toDataURL(text.trim(), {
    width: 512,
    margin: 2,
    color: {
      dark: foreground,
      light: background,
    },
    errorCorrectionLevel: errorLevel,
  });
}
