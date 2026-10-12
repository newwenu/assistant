import { useState, useMemo } from "react";
import { Input, Typography, Table, Tag } from "antd";
import { jwtDecode } from "jwt-decode";
import styles from "./jwt-parser.module.css";

const { TextArea } = Input;

const CLAIM_DESCRIPTIONS: Record<string, string> = {
  iss: "签发者 (Issuer)",
  sub: "主题 (Subject)",
  aud: "受众 (Audience)",
  exp: "过期时间 (Expiration)",
  nbf: "生效时间 (Not Before)",
  iat: "签发时间 (Issued At)",
  jti: "JWT ID",
};

const ALGORITHM_DESCRIPTIONS: Record<string, string> = {
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

function formatValue(
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

export default function JwtParser() {
  const [rawJwt, setRawJwt] = useState(
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
  );

  const { header, payload, error } = useMemo(() => {
    try {
      const header = jwtDecode<Record<string, unknown>>(rawJwt, {
        header: true,
      });
      const payload = jwtDecode<Record<string, unknown>>(rawJwt);
      return { header, payload, error: null };
    } catch {
      return { header: null, payload: null, error: "无效的 JWT Token" };
    }
  }, [rawJwt]);

  const renderSection = (
    title: string,
    data: Record<string, unknown> | null,
  ) => {
    if (!data) return null;
    const columns = [
      { title: "字段", dataIndex: "claim", key: "claim", width: 100 },
      { title: "值", dataIndex: "value", key: "value" },
      { title: "说明", dataIndex: "friendly", key: "friendly", width: 220 },
    ];
    const rows = Object.entries(data).map(([claim, value]) => {
      const { display, friendly } = formatValue(claim, value);
      return {
        key: claim,
        claim,
        value: (
          <Typography.Text
            copyable
            style={{ fontFamily: "monospace", fontSize: 12 }}
          >
            {display}
          </Typography.Text>
        ),
        friendly: friendly ? (
          <Tag color="blue">{friendly}</Tag>
        ) : (
          (CLAIM_DESCRIPTIONS[claim] ?? "")
        ),
      };
    });
    return (
      <div className={styles.section}>
        <Typography.Text strong className={styles.sectionLabel}>
          {title}
        </Typography.Text>
        <Table
          columns={columns}
          dataSource={rows}
          pagination={false}
          size="small"
        />
      </div>
    );
  };

  return (
    <div>
      <Typography.Text strong className={styles.sectionLabel}>
        JWT Token
      </Typography.Text>
      <TextArea
        value={rawJwt}
        onChange={(e) => setRawJwt(e.target.value)}
        placeholder="粘贴 JWT Token..."
        rows={3}
        style={{ fontFamily: "monospace", marginBottom: 8 }}
        status={error ? "error" : undefined}
      />
      {error && (
        <Typography.Text type="danger" className={styles.errorText}>
          {error}
        </Typography.Text>
      )}
      {!error && header && payload && (
        <>
          {renderSection("Header", header)}
          {renderSection("Payload", payload)}
          {payload.exp && (
            <div>
              {Number(payload.exp) * 1000 > Date.now() ? (
                <Tag color="green">未过期</Tag>
              ) : (
                <Tag color="red">已过期</Tag>
              )}
            </div>
          )}
        </>
      )}
    </div>
  );
}
