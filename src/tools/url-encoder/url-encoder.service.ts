export function encodeUrl(input: string): string {
  return encodeURIComponent(input);
}

export function decodeUrl(input: string): string {
  return decodeURIComponent(input);
}

export function parseUrl(input: string): string | null {
  try {
    const url = new URL(input);
    const parts = [
      `协议: ${url.protocol}`,
      `主机: ${url.hostname}`,
      url.port ? `端口: ${url.port}` : "",
      `路径: ${url.pathname}`,
      url.search ? `查询: ${url.search}` : "",
      url.hash ? `片段: ${url.hash}` : "",
    ]
      .filter(Boolean)
      .join("\n");
    return parts;
  } catch {
    return null;
  }
}
