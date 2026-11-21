import { useState, useCallback, useMemo } from "react";
import {
  Input,
  InputNumber,
  Switch,
  Button,
  Typography,
  Slider,
  Progress,
  App,
} from "antd";
import { CopyOutlined, ReloadOutlined } from "@ant-design/icons";
import styles from "./password-generator.module.css";
import {
  generatePassword,
  getCrackTimeEstimation,
  formatDuration,
  getStrengthLevel,
} from "./password-generator.service";

export default function PasswordGenerator() {
  const { message } = App.useApp();
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
