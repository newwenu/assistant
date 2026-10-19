import { useState, useMemo } from "react";
import { Input, Select, Switch, Button, Typography, message } from "antd";
import { CopyOutlined } from "@ant-design/icons";
import { format as formatSQL } from "sql-formatter";
import styles from "./sql-formatter.module.css";

const { TextArea } = Input;

const DIALECT_OPTIONS = [
  { label: "Standard SQL", value: "sql" },
  { label: "MySQL", value: "mysql" },
  { label: "PostgreSQL", value: "postgresql" },
  { label: "MariaDB", value: "mariadb" },
  { label: "SQLite", value: "sqlite" },
  { label: "Oracle PL/SQL", value: "plsql" },
  { label: "SQL Server T-SQL", value: "tsql" },
  { label: "GCP BigQuery", value: "bigquery" },
  { label: "IBM DB2", value: "db2" },
  { label: "Apache Hive", value: "hive" },
  { label: "Couchbase N1QL", value: "n1ql" },
  { label: "Amazon Redshift", value: "redshift" },
  { label: "Spark", value: "spark" },
];

const KEYWORD_CASE_OPTIONS = [
  { label: "UPPERCASE", value: "upper" },
  { label: "lowercase", value: "lower" },
  { label: "Preserve", value: "preserve" },
];

const INDENT_STYLE_OPTIONS = [
  { label: "Standard", value: "standard" },
  { label: "Tabular Left", value: "tabularLeft" },
  { label: "Tabular Right", value: "tabularRight" },
];

export default function SqlFormatter() {
  const [rawSQL, setRawSQL] = useState(
    "select field1,field2,field3 from my_table where my_condition;",
  );
  const [language, setLanguage] = useState("sql");
  const [keywordCase, setKeywordCase] = useState("upper");
  const [indentStyle, setIndentStyle] = useState("standard");
  const [tabulateAlias, setTabulateAlias] = useState(true);

  const config = useMemo(
    () => ({
      keywordCase: keywordCase as "upper" | "lower" | "preserve",
      useTabs: false,
      language: language as
        | "sql"
        | "mysql"
        | "postgresql"
        | "mariadb"
        | "sqlite"
        | "plsql"
        | "tsql"
        | "bigquery"
        | "db2"
        | "hive"
        | "n1ql"
        | "redshift"
        | "spark",
      indentStyle: indentStyle as "standard" | "tabularLeft" | "tabularRight",
      tabulateAlias,
    }),
    [keywordCase, language, indentStyle, tabulateAlias],
  );

  const formatted = useMemo(() => {
    try {
      return formatSQL(rawSQL, config);
    } catch {
      return null;
    }
  }, [rawSQL, config]);

  const minified = useMemo(() => {
    try {
      return rawSQL.replace(/\s+/g, " ").trim();
    } catch {
      return null;
    }
  }, [rawSQL]);

  const copy = (text: string) => {
    void navigator.clipboard.writeText(text);
    message.success("已复制");
  };

  return (
    <div>
      <div className={styles.configRow}>
        <div className={styles.configItem}>
          <Typography.Text strong className={styles.configLabel}>
            方言
          </Typography.Text>
          <Select
            value={language}
            onChange={setLanguage}
            options={DIALECT_OPTIONS}
            style={{ flex: 1 }}
          />
        </div>
        <div className={styles.configItem}>
          <Typography.Text strong className={styles.configLabel}>
            关键字大小写
          </Typography.Text>
          <Select
            value={keywordCase}
            onChange={setKeywordCase}
            options={KEYWORD_CASE_OPTIONS}
            style={{ flex: 1 }}
          />
        </div>
        <div className={styles.configItem}>
          <Typography.Text strong className={styles.configLabel}>
            缩进风格
          </Typography.Text>
          <Select
            value={indentStyle}
            onChange={setIndentStyle}
            options={INDENT_STYLE_OPTIONS}
            style={{ flex: 1 }}
          />
        </div>
      </div>

      <div className={styles.switchRow}>
        <label className={styles.switchLabel}>
          <Switch
            size="small"
            checked={tabulateAlias}
            onChange={setTabulateAlias}
          />
          <span>别名对齐</span>
        </label>
      </div>

      <Typography.Text strong>输入 SQL</Typography.Text>
      <TextArea
        value={rawSQL}
        onChange={(e) => setRawSQL(e.target.value)}
        placeholder="输入 SQL 查询..."
        rows={20}
        className={styles.textArea}
        style={{ fontFamily: "monospace" }}
      />

      <div className={styles.resultHeader}>
        <Typography.Text strong>格式化结果</Typography.Text>
        <div className={styles.resultActions}>
          <Button
            size="small"
            icon={<CopyOutlined />}
            disabled={!formatted}
            onClick={() => formatted && copy(formatted)}
          >
            复制
          </Button>
        </div>
      </div>
      <TextArea
        value={formatted ?? ""}
        readOnly
        rows={12}
        className={styles.textArea}
        style={{ fontFamily: "monospace" }}
      />

      {minified && (
        <>
          <div className={styles.resultHeader}>
            <Typography.Text strong>压缩结果</Typography.Text>
            <Button
              size="small"
              icon={<CopyOutlined />}
              onClick={() => copy(minified)}
            >
              复制
            </Button>
          </div>
          <TextArea
            value={minified}
            readOnly
            rows={3}
            className={styles.textArea}
            style={{ fontFamily: "monospace" }}
          />
        </>
      )}
    </div>
  );
}
