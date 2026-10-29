export const CLAIM_DESCRIPTIONS: Record<string, string> = {
  iss: "签发者 (Issuer)",
  sub: "主题 (Subject)",
  aud: "受众 (Audience)",
  exp: "过期时间 (Expiration)",
  nbf: "生效时间 (Not Before)",
  iat: "签发时间 (Issued At)",
  jti: "JWT ID",
};

export const ALGORITHM_DESCRIPTIONS: Record<string, string> = {
  HS256: "HMAC using SHA-256",
  HS384: "HMAC using SHA-384",
  HS512: "HMAC using SHA-512",
  RS256: "RSASSA-PKCS1-v1_5 using SHA-256",
  RS384: "RSASSA-PKCS1-v1_5 using SHA-384",
  RS512: "RSASSA-PKCS1-v1_5 using SHA-512",
  ES256: "ECDSA using P-256 curve and SHA-256",
  ES384: "ECDSA using P-384 curve and SHA-384",
  ES512: "ECDSA using P-521 curve and SHA-512",
  PS256: "RSASSA-PSS using SHA-256",
  PS384: "RSASSA-PSS using SHA-384",
  PS512: "RSASSA-PSS using SHA-512",
  none: "无签名 (Unsecured)",
};

export function formatValue(
  claim: string,
  value: unknown,
): { display: string; friendly: string | null } {
  if (typeof value === "number" && ["exp", "nbf", "iat"].includes(claim)) {
    try {
      const date = new Date(value * 1000).toLocaleString();
      return { display: String(value), friendly: date };
    } catch {
      // fall through
    }
  }
  if (claim === "alg" && typeof value === "string") {
    const desc = ALGORITHM_DESCRIPTIONS[value];
    if (desc) return { display: String(value), friendly: desc };
  }
  if (typeof value === "object") {
    return { display: JSON.stringify(value, null, 2), friendly: null };
  }
  if (
    typeof value === "string" ||
    typeof value === "number" ||
    typeof value === "boolean"
  ) {
    return { display: String(value), friendly: null };
  }
  return { display: JSON.stringify(value), friendly: null };
}
