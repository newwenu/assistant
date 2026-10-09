import { useState, useMemo } from "react";
import { Input, Switch, Typography, Table, Alert } from "antd";
import cronstrue from "cronstrue/i18n";
import { isValidCron } from "cron-validator";
import styles from "./cron-parser.module.css";

const HELPERS = [
  { symbol: "*", meaning: "任意值", example: "* * * *", equivalent: "每分钟" },
  {
    symbol: "-",
    meaning: "范围",
    example: "1-10 * * *",
    equivalent: "第 1 到 10 分钟",
  },
  {
    symbol: ",",
    meaning: "列表",
    example: "1,10 * * *",
    equivalent: "第 1 和第 10 分钟",
  },
  {
    symbol: "/",
    meaning: "步长",
    example: "*/10 * * *",
    equivalent: "每 10 分钟",
  },
  {
    symbol: "@yearly",
    meaning: "每年一次",
    example: "@yearly",
    equivalent: "0 0 1 1 *",
  },
  {
    symbol: "@annually",
    meaning: "同 @yearly",
    example: "@annually",
    equivalent: "0 0 1 1 *",
  },
  {
    symbol: "@monthly",
    meaning: "每月一次",
    example: "@monthly",
    equivalent: "0 0 1 * *",
  },
  {
    symbol: "@weekly",
    meaning: "每周一次",
    example: "@weekly",
    equivalent: "0 0 * * 0",
  },
  {
    symbol: "@daily",
    meaning: "每天一次",
    example: "@daily",
    equivalent: "0 0 * * *",
  },
  {
    symbol: "@midnight",
    meaning: "同 @daily",
    example: "@midnight",
    equivalent: "0 0 * * *",
  },
  {
    symbol: "@hourly",
    meaning: "每小时一次",
    example: "@hourly",
    equivalent: "0 * * * *",
  },
  {
    symbol: "@reboot",
    meaning: "启动时运行",
    example: "@reboot",
    equivalent: "—",
  },
];

const COLUMNS = [
  { title: "符号", dataIndex: "symbol", key: "symbol", width: 100 },
  { title: "含义", dataIndex: "meaning", key: "meaning" },
  { title: "示例", dataIndex: "example", key: "example" },
  { title: "等价", dataIndex: "equivalent", key: "equivalent" },
];

function getNextExecutions(cronExpr: string, count: number): Date[] {
  const results: Date[] = [];
  try {
    const parts = cronExpr.trim().split(/\s+/);
    if (parts.length < 5) return results;

    const now = new Date();
    let current = new Date(now.getTime() + 60000);
    current.setSeconds(0, 0);

    const minuteField = parts[0];
    const hourField = parts[1];
    const domField = parts[2];
    const monthField = parts[3];

    const parseField = (field: string, min: number, max: number): number[] => {
      if (field === "*")
        return Array.from({ length: max - min + 1 }, (_, i) => min + i);
      const values = new Set<number>();
      for (const part of field.split(",")) {
        if (part.includes("/")) {
          const [range, step] = part.split("/");
          const s = Number(step);
          let start = min;
          let end = max;
          if (range !== "*") {
            [start, end] = range.split("-").map(Number);
          }
          for (let i = start; i <= end; i += s) values.add(i);
        } else if (part.includes("-")) {
          const [start, end] = part.split("-").map(Number);
          for (let i = start; i <= end; i++) values.add(i);
        } else {
          values.add(Number(part));
        }
      }
      return Array.from(values).sort((a, b) => a - b);
    };

    const minutes = parseField(minuteField, 0, 59);
    const hours = parseField(hourField, 0, 23);
    const doms = parseField(domField, 1, 31);
    const months = parseField(monthField, 1, 12);

    const maxIter = 525600;
    for (let i = 0; i < maxIter && results.length < count; i++) {
      if (
        minutes.includes(current.getMinutes()) &&
        hours.includes(current.getHours()) &&
        doms.includes(current.getDate()) &&
        months.includes(current.getMonth() + 1)
      ) {
        results.push(new Date(current.getTime()));
      }
      current = new Date(current.getTime() + 60000);
    }
  } catch {
    // ignore
  }
  return results;
}

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
          message="无效的 Cron 表达式"
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
