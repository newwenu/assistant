import { useState, useMemo } from "react";
import { Input, Typography, Table, Tag } from "antd";
import { jwtDecode } from "jwt-decode";
import { CLAIM_DESCRIPTIONS, formatValue } from "./jwt-parser.service";
import styles from "./jwt-parser.module.css";

const { TextArea } = Input;

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
