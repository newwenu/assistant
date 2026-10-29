import { useState, useMemo } from "react";
import { Input, Switch, Typography, Table, Alert } from "antd";
import cronstrue from "cronstrue/i18n";
import { isValidCron } from "cron-validator";
import { HELPERS, COLUMNS, getNextExecutions } from "./cron-parser.service";
import styles from "./cron-parser.module.css";

export default function CronParser() {
  const [cron, setCron] = useState("40 * * * *");
  const [verbose, setVerbose] = useState(true);
  const [use24Hour, setUse24Hour] = useState(true);
  const [dowStartZero, setDowStartZero] = useState(true);

  const valid = useMemo(() => {
    try {
      return isValidCron(cron, {
        allowBlankDay: true,
        alias: true,
        seconds: true,
      });
    } catch {
      return false;
    }
  }, [cron]);

  const humanReadable = useMemo(() => {
    if (!valid) return "";
    try {
      return cronstrue.toString(cron, {
        verbose,
        dayOfWeekStartIndexZero: dowStartZero,
        use24HourTimeFormat: use24Hour,
        throwExceptionOnParseError: true,
        locale: "zh_CN",
      });
    } catch {
      return "无法解析";
    }
  }, [cron, valid, verbose, use24Hour, dowStartZero]);

  const nextExecutions = useMemo(() => {
    if (!valid) return [];
    return getNextExecutions(cron, 5);
  }, [cron, valid]);

  return (
    <div>
      <Input
        value={cron}
        onChange={(e) => setCron(e.target.value)}
        placeholder="* * * * *"
        className={styles.cronInput}
        status={valid ? undefined : "error"}
        style={{ fontFamily: "monospace", fontSize: 30, textAlign: "center" }}
      />

      {valid && humanReadable && (
        <div className={styles.humanReadable}>{humanReadable}</div>
      )}
      {!valid && cron.trim() && (
        <Alert
          type="error"
          title="无效的 Cron 表达式"
          className={styles.alert}
        />
      )}

      <div className={styles.optionsRow}>
        <label className={styles.optionLabel}>
          <Switch size="small" checked={verbose} onChange={setVerbose} />
          <span>详细描述</span>
        </label>
        <label className={styles.optionLabel}>
          <Switch size="small" checked={use24Hour} onChange={setUse24Hour} />
          <span>24 小时制</span>
        </label>
        <label className={styles.optionLabel}>
          <Switch
            size="small"
            checked={dowStartZero}
            onChange={setDowStartZero}
          />
          <span>周日=0</span>
        </label>
      </div>

      {nextExecutions.length > 0 && (
        <div className={styles.nextSection}>
          <Typography.Text strong>下次执行时间</Typography.Text>
          <ul className={styles.nextList}>
            {nextExecutions.map((d, i) => (
              <li key={i}>{d.toLocaleString("zh-CN")}</li>
            ))}
          </ul>
        </div>
      )}

      <div className={styles.referenceSection}>
        <Typography.Text strong>Cron 表达式参考</Typography.Text>
        <pre className={styles.cronFormat}>
          {`┌──────────── [可选] 秒 (0 - 59)
│ ┌────────── 分钟 (0 - 59)
│ │ ┌──────── 小时 (0 - 23)
│ │ │ ┌────── 日期 (1 - 31)
│ │ │ │ ┌──── 月份 (1 - 12)
│ │ │ │ │ ┌── 星期 (0 - 6, 0=周日)
│ │ │ │ │ │
* * * * * *`}
        </pre>
        <Table
          dataSource={HELPERS}
          columns={COLUMNS}
          rowKey="symbol"
          size="small"
          pagination={false}
        />
      </div>
    </div>
  );
}
