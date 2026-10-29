import { useState, useMemo } from "react";
import { Input, Select, Switch, Button, Typography, message } from "antd";
import { CopyOutlined } from "@ant-design/icons";
import {
  DIALECT_OPTIONS,
  KEYWORD_CASE_OPTIONS,
  INDENT_STYLE_OPTIONS,
  formatSql,
  minifySql,
  type SqlLanguage,
  type KeywordCase,
  type IndentStyle,
} from "./sql-formatter.service";
import styles from "./sql-formatter.module.css";

const { TextArea } = Input;

export default function SqlFormatter() {
  const [rawSQL, setRawSQL] = useState(
    "select field1,field2,field3 from my_table where my_condition;",
  );
  const [language, setLanguage] = useState<SqlLanguage>("sql");
  const [keywordCase, setKeywordCase] = useState<KeywordCase>("upper");
  const [indentStyle, setIndentStyle] = useState<IndentStyle>("standard");
  const [tabulateAlias, setTabulateAlias] = useState(true);

  const formatted = useMemo(
    () =>
      formatSql(rawSQL, {
        keywordCase,
        language,
        indentStyle,
      }),
    [rawSQL, keywordCase, language, indentStyle],
  );

  const minified = useMemo(() => minifySql(rawSQL), [rawSQL]);

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
            onChange={(v) => setLanguage(v)}
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
            onChange={(v) => setKeywordCase(v)}
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
            onChange={(v) => setIndentStyle(v)}
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
