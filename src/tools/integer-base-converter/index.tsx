import { useState, useMemo } from "react";
import { Input, InputNumber, Typography, Button, message } from "antd";
import { CopyOutlined } from "@ant-design/icons";
import styles from "./integer-base-converter.module.css";

const CHARSET =
  "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ+/";

function convertBase({
  value,
  fromBase,
  toBase,
}: {
  value: string;
  fromBase: number;
  toBase: number;
}): string {
  if (!value) return "";
  const fromRange = CHARSET.slice(0, fromBase).split("");
  const toRange = CHARSET.slice(0, toBase).split("");

  let decValue = value
    .split("")
    .reverse()
    .reduce((carry: bigint, digit: string, index: number) => {
      const digitLower = digit.toLowerCase();
      const idx = fromRange.indexOf(digitLower);
      if (idx === -1 && digit !== digitLower) {
        const idxUpper = fromRange.indexOf(digit);
        if (idxUpper === -1) {
          throw new Error(`无效数字 "${digit}" (基 ${fromBase})`);
        }
        return (carry += BigInt(idxUpper) * BigInt(fromBase) ** BigInt(index));
      }
      if (idx === -1) {
        throw new Error(`无效数字 "${digit}" (基 ${fromBase})`);
      }
      return (carry += BigInt(idx) * BigInt(fromBase) ** BigInt(index));
    }, 0n);

  if (decValue === 0n) return "0";

  let result = "";
  while (decValue > 0n) {
    result = toRange[Number(decValue % BigInt(toBase))] + result;
    decValue = (decValue - (decValue % BigInt(toBase))) / BigInt(toBase);
  }
  return result;
}

const PRESET_BASES = [
  { label: "二进制 (2)", base: 2 },
  { label: "八进制 (8)", base: 8 },
  { label: "十进制 (10)", base: 10 },
  { label: "十六进制 (16)", base: 16 },
  { label: "Base64 (64)", base: 64 },
];

export default function IntegerBaseConverter() {
  const [inputValue, setInputValue] = useState("42");
  const [inputBase, setInputBase] = useState(10);
  const [customBase, setCustomBase] = useState(36);

  const { results, error } = useMemo((): {
    results: Record<string, string>;
    error: string | null;
  } => {
    if (!inputValue) return { results: {}, error: null };
    try {
      const r: Record<string, string> = {};
      for (const b of [...PRESET_BASES, { label: "", base: customBase }]) {
        if (b.base < 2 || b.base > 64) continue;
        r[String(b.base)] = convertBase({
          value: inputValue,
          fromBase: inputBase,
          toBase: b.base,
        });
      }
      return { results: r, error: null };
    } catch (e: unknown) {
      return { results: {}, error: (e as Error).message };
    }
  }, [inputValue, inputBase, customBase]);

  const copy = (value: string) => {
    void navigator.clipboard.writeText(value);
    message.success("已复制");
  };

  return (
    <div>
      <div className={styles.inputRow}>
        <Typography.Text strong className={styles.inputLabel}>
          输入
        </Typography.Text>
        <Input
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          placeholder="输入数值..."
          status={error ? "error" : undefined}
          style={{ flex: 1, fontFamily: "monospace" }}
        />
      </div>
      <div className={styles.inputRow}>
        <Typography.Text strong className={styles.inputLabel}>
          输入基
        </Typography.Text>
        <InputNumber
          min={2}
          max={64}
          value={inputBase}
          onChange={(v) => v && setInputBase(v)}
          style={{ width: 100 }}
        />
      </div>
      {error && (
        <Typography.Text type="danger" className={styles.errorText}>
          {error}
        </Typography.Text>
      )}
      {PRESET_BASES.map((b) => (
        <div key={b.base} className={styles.resultRow}>
          <Typography.Text strong className={styles.resultLabel}>
            {b.label}
          </Typography.Text>
          <Input
            value={results[String(b.base)] ?? ""}
            readOnly
            size="small"
            style={{ flex: 1, fontFamily: "monospace" }}
          />
          <Button
            icon={<CopyOutlined />}
            size="small"
            onClick={() =>
              results[String(b.base)] && copy(results[String(b.base)])
            }
          />
        </div>
      ))}
      <div className={styles.customRow}>
        <Typography.Text strong className={styles.customLabel}>
          自定义基
        </Typography.Text>
        <InputNumber
          min={2}
          max={64}
          value={customBase}
          onChange={(v) => v && setCustomBase(v)}
          size="small"
          style={{ width: 80 }}
        />
        <Input
          value={results[String(customBase)] ?? ""}
          readOnly
          size="small"
          style={{ flex: 1, fontFamily: "monospace" }}
        />
        <Button
          icon={<CopyOutlined />}
          size="small"
          onClick={() =>
            results[String(customBase)] && copy(results[String(customBase)])
          }
        />
      </div>
    </div>
  );
}
