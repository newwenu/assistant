const MAX_GIF_URL_LEN = 2048;
const MAX_DATA_URL_LEN = 2_200_000;

const RASTER_EXTENSIONS = [".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp"];

const DATA_URL_RE =
  /^data:image\/(?:png|jpe?g|gif|webp|bmp);base64,(?:(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)|(?:[A-Za-z0-9+/]{4})+)$/;

function hasImagePath(value: string): boolean {
  const [path = ""] = value.split(/[?#]/, 1);
  const lower = path.toLowerCase();
  return RASTER_EXTENSIONS.some((ext) => lower.endsWith(ext));
}

export function isValidGifSrc(value: unknown): value is string {
  if (typeof value !== "string") return false;
  const trimmed = value.trim();
  if (trimmed.length === 0) return false;

  if (trimmed.startsWith("data:")) {
    return trimmed.length <= MAX_DATA_URL_LEN && DATA_URL_RE.test(trimmed);
  }

  if (trimmed.length > MAX_GIF_URL_LEN) return false;

  try {
    const parsed = new URL(trimmed);
    if (!hasImagePath(parsed.pathname)) return false;
    if (parsed.protocol === "https:" || parsed.protocol === "file:")
      return true;
    if (parsed.protocol !== "http:") return false;
    return ["localhost", "127.0.0.1", "::1", "[::1]"].includes(parsed.hostname);
  } catch {
    return (
      hasImagePath(trimmed) &&
      (trimmed.startsWith("/") || trimmed.startsWith("~/"))
    );
  }
}
