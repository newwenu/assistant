export function textToBase64(text: string, urlSafe: boolean): string {
  const bytes = new TextEncoder().encode(text);
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  const encoded = btoa(binary);
  if (urlSafe) {
    return encoded.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  }
  return encoded;
}

export function base64ToText(base64: string, urlSafe: boolean): string {
  let b64 = base64;
  if (urlSafe) {
    b64 = b64.replace(/-/g, "+").replace(/_/g, "/");
    const pad = (4 - (b64.length % 4)) % 4;
    b64 += "=".repeat(pad);
  }
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return new TextDecoder().decode(bytes);
}

export function isValidBase64(base64: string, urlSafe: boolean): boolean {
  try {
    base64ToText(base64, urlSafe);
    return true;
  } catch {
    return false;
  }
}
