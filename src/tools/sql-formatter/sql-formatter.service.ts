import { format as formatSQL } from "sql-formatter";

export const DIALECT_OPTIONS = [
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

export const KEYWORD_CASE_OPTIONS = [
  { label: "UPPERCASE", value: "upper" },
  { label: "lowercase", value: "lower" },
  { label: "Preserve", value: "preserve" },
];

export const INDENT_STYLE_OPTIONS = [
  { label: "Standard", value: "standard" },
  { label: "Tabular Left", value: "tabularLeft" },
  { label: "Tabular Right", value: "tabularRight" },
];

export type SqlLanguage =
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
  | "spark";

export type KeywordCase = "upper" | "lower" | "preserve";
export type IndentStyle = "standard" | "tabularLeft" | "tabularRight";

export interface SqlFormatConfig {
  keywordCase: KeywordCase;
  language: SqlLanguage;
  indentStyle: IndentStyle;
}

export function formatSql(
  rawSQL: string,
  config: SqlFormatConfig,
): string | null {
  try {
    return formatSQL(rawSQL, {
      keywordCase: config.keywordCase,
      useTabs: false,
      language: config.language,
      indentStyle: config.indentStyle,
    });
  } catch {
    return null;
  }
}

export function minifySql(rawSQL: string): string {
  return rawSQL.replace(/\s+/g, " ").trim();
}
