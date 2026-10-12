import { useState, useCallback, useMemo } from "react";
import {
  Input,
  InputNumber,
  Switch,
  Button,
  Typography,
  Slider,
  Progress,
  message,
} from "antd";
import { CopyOutlined, ReloadOutlined } from "@ant-design/icons";
import styles from "./password-generator.module.css";

const LOWERCASE = "abcdefghijklmnopqrstuvwxyz";
const UPPERCASE = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS = "0123456789";
const SPECIAL = "!@#$%^&*()_+-=[]{}|;:',.<>?/`~";

function getCharsetLength(password: string): number {
  let len = 0;
  if (/[a-z]/.test(password)) len += 26;
  if (/[A-Z]/.test(password)) len += 26;
  if (/\d/.test(password)) len += 10;
  if (/\W|_/.test(password)) len += 32;
  return len;
}

function getCrackTimeEstimation(password: string, guessesPerSecond = 1e9) {
  const charsetLength = getCharsetLength(password);
  const passwordLength = password.length;
  const entropy =
    passwordLength === 0 ? 0 : Math.log2(charsetLength) * passwordLength;
  const secondsToCrack = 2 ** entropy / guessesPerSecond;
  const score = Math.min(entropy / 128, 1);
  return { entropy, charsetLength, passwordLength, secondsToCrack, score };
}

function formatDuration(seconds: number): string {
  if (seconds <= 0.001) return "瞬间";
  if (seconds <= 1) return "不到 1 秒";

  const units = [
    { unit: "千年", s: 31536000000 },
    { unit: "世纪", s: 3153600000 },
    { unit: "十年", s: 315360000 },
    { unit: "年", s: 31536000 },
    { unit: "月", s: 2592000 },
    { unit: "周", s: 604800 },
    { unit: "天", s: 86400 },
    { unit: "小时", s: 3600 },
    { unit: "分钟", s: 60 },
    { unit: "秒", s: 1 },
  ];

  const parts: string[] = [];
  let remaining = seconds;
  for (const { unit, s } of units) {
    const qty = Math.floor(remaining / s);
    if (qty > 0) {
      parts.push(`${qty} ${unit}`);
      remaining %= s;
    }
    if (parts.length >= 2) break;
  }
  return parts.join(" ") || "不到 1 秒";
}

function getStrengthLevel(score: number): {
  label: string;
  color: string;
  percent: number;
} {
  if (score < 0.25)
    return { label: "极弱", color: "#ff4d4f", percent: score * 100 };
  if (score < 0.5)
    return { label: "弱", color: "#faad14", percent: score * 100 };
  if (score < 0.75)
    return { label: "中等", color: "#1677ff", percent: score * 100 };
  return { label: "强", color: "#52c41a", percent: score * 100 };
}

function generatePassword(
  length: number,
  useLower: boolean,
  useUpper: boolean,
  useDigits: boolean,
  useSpecial: boolean,
): string {
  let charset = "";
  if (useLower) charset += LOWERCASE;
  if (useUpper) charset += UPPERCASE;
  if (useDigits) charset += DIGITS;
  if (useSpecial) charset += SPECIAL;
  if (!charset) charset = LOWERCASE;

  const array = new Uint32Array(length);
  crypto.getRandomValues(array);
  return Array.from(array, (v) => charset[v % charset.length]).join("");
}

export default function PasswordGenerator() {
  const [length, setLength] = useState(16);
  const [useLower, setUseLower] = useState(true);
  const [useUpper, setUseUpper] = useState(true);
  const [useDigits, setUseDigits] = useState(true);
  const [useSpecial, setUseSpecial] = useState(true);
  const [password, setPassword] = useState(() =>
    generatePassword(16, true, true, true, true),
  );

  const regenerate = useCallback(() => {
    setPassword(
      generatePassword(length, useLower, useUpper, useDigits, useSpecial),
    );
  }, [length, useLower, useUpper, useDigits, useSpecial]);

  const estimation = useMemo(
    () => getCrackTimeEstimation(password),
    [password],
  );
  const strength = useMemo(
    () => getStrengthLevel(estimation.score),
    [estimation.score],
  );

  const copy = () => {
    void navigator.clipboard.writeText(password);
    message.success("已复制");
  };

  return (
    <div>
      <div className={styles.passwordRow}>
        <InputNumber
          min={4}
          max={128}
          value={length}
          onChange={(v) => v && setLength(v)}
          style={{ width: 80 }}
        />
        <Slider
          min={4}
          max={128}
          value={length}
          onChange={setLength}
          style={{ flex: 1 }}
        />
      </div>

      <div className={styles.switchRow}>
        <label className={styles.switchLabel}>
          <Switch size="small" checked={useLower} onChange={setUseLower} />
          <span>小写 a-z</span>
        </label>
        <label className={styles.switchLabel}>
          <Switch size="small" checked={useUpper} onChange={setUseUpper} />
          <span>大写 A-Z</span>
        </label>
        <label className={styles.switchLabel}>
          <Switch size="small" checked={useDigits} onChange={setUseDigits} />
          <span>数字 0-9</span>
        </label>
        <label className={styles.switchLabel}>
          <Switch size="small" checked={useSpecial} onChange={setUseSpecial} />
          <span>特殊字符</span>
        </label>
      </div>

      <div className={styles.resultRow}>
        <Input.Password
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder="输入或生成密码..."
          className={styles.passwordInput}
          style={{ fontFamily: "monospace" }}
        />
        <Button icon={<ReloadOutlined />} onClick={regenerate} size="small">
          生成
        </Button>
        <Button
          icon={<CopyOutlined />}
          onClick={copy}
          size="small"
          disabled={!password}
        >
          复制
        </Button>
      </div>

      {password && (
        <div className={styles.strengthCard}>
          <div className={styles.strengthHeader}>
            <Typography.Text strong>暴力破解耗时</Typography.Text>
            <Typography.Text
              strong
              style={{ color: strength.color, fontSize: 18 }}
            >
              {formatDuration(estimation.secondsToCrack)}
            </Typography.Text>
          </div>
          <Progress
            percent={strength.percent}
            strokeColor={strength.color}
            showInfo={false}
            size="small"
          />
          <div className={styles.strengthMeta}>
            <span>
              强度：<b style={{ color: strength.color }}>{strength.label}</b>
            </span>
            <span>熵：{estimation.entropy.toFixed(1)} bits</span>
            <span>字符集：{estimation.charsetLength}</span>
          </div>
        </div>
      )}

      <Typography.Text type="secondary" className={styles.note}>
        输入密码实时评估强度，或点击「生成」创建安全密码。基于暴力破解时间估算，未考虑字典攻击。
      </Typography.Text>
    </div>
  );
}
