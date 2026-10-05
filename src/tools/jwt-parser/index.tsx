import { useState, useMemo } from "react";
import { Input, Typography, Table, Tag } from "antd";

const { TextArea } = Input;

function base64UrlDecode(str: string): string {
  let base64 = str.replace(/-/g, "+").replace(/_/g, "/");
  while (base64.length % 4) base64 += "=";
  return atob(base64);
}

function decodeJwtPart(part: string): Record<string, unknown> {
  const json = base64UrlDecode(part);
  return JSON.parse(json);
}

const CLAIM_DESCRIPTIONS: Record<string, string> = {
  iss: "签发者",
  sub: "主题",
  aud: "受众",
  exp: "过期时间",
  nbf: "生效时间",
  iat: "签发时间",
  jti: "JWT ID",
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
      // invalid date, fall through
    }
  }
  if (typeof value === "object") {
    return { display: JSON.stringify(value, null, 2), friendly: null };
  }
  return { display: String(value), friendly: null };
}

export default function JwtParser() {
  const [rawJwt, setRawJwt] = useState(
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
  );

  const { header, payload, error } = useMemo(() => {
    try {
      const parts = rawJwt.trim().split(".");
      if (parts.length !== 3) throw new Error("JWT 必须包含 3 部分");
      const header = decodeJwtPart(parts[0]);
      const payload = decodeJwtPart(parts[1]);
      return { header, payload, error: null };
    } catch (e: any) {
      return { header: null, payload: null, error: e.message };
    }
  }, [rawJwt]);

  const renderSection = (
    title: string,
    data: Record<string, unknown> | null,
  ) => {
    if (!data) return null;
    const columns = [
      { title: "字段", dataIndex: "claim", key: "claim", width: 120 },
      { title: "值", dataIndex: "value", key: "value" },
      { title: "说明", dataIndex: "friendly", key: "friendly", width: 200 },
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
      <div style={{ marginBottom: 16 }}>
        <Typography.Text strong style={{ display: "block", marginBottom: 4 }}>
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
      <Typography.Text strong style={{ display: "block", marginBottom: 4 }}>
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
        <Typography.Text
          type="danger"
          style={{ display: "block", marginBottom: 8 }}
        >
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
