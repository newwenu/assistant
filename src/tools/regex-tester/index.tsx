import { useState, useMemo } from "react";
import { Input, Checkbox, Typography, Tag, Table, Tabs } from "antd";
import {
  PATTERN_LIBRARY,
  matchRegex,
  buildHighlightHtml,
  buildFlags,
  type MatchResult,
} from "./regex-tester.service";
import styles from "./regex-tester.module.css";

const { TextArea } = Input;

export default function RegexTester() {
  const [pattern, setPattern] = useState("");
  const [testText, setTestText] = useState("");
  const [replaceText, setReplaceText] = useState("");
  const [globalFlag, setGlobalFlag] = useState(true);
  const [ignoreCase, setIgnoreCase] = useState(false);
  const [multiline, setMultiline] = useState(false);
  const [dotAll, setDotAll] = useState(false);
  const [unicodeFlag, setUnicodeFlag] = useState(false);
  const [unicodeSetsFlag, setUnicodeSetsFlag] = useState(false);

  const { matches, error, highlightedHtml, replacedText } = useMemo(() => {
    if (!pattern)
      return {
        matches: [],
        error: null,
        highlightedHtml: testText,
        replacedText: "",
      };

    const flags = buildFlags({
      global: globalFlag,
      ignoreCase,
      multiline,
      dotAll,
      unicode: unicodeFlag,
      unicodeSets: unicodeSetsFlag,
    });

    const { matches: results, error: err } = matchRegex(
      pattern,
      testText,
      flags,
      globalFlag,
    );

    if (err) {
      return {
        matches: [],
        error: err,
        highlightedHtml: testText,
        replacedText: "",
      };
    }

    const html =
      results.length > 0 ? buildHighlightHtml(testText, results) : testText;

    let replaced = "";
    if (replaceText && results.length > 0) {
      try {
        replaced = testText.replace(new RegExp(pattern, flags), replaceText);
      } catch {
        replaced = "";
      }
    }

    return {
      matches: results,
      error: null,
      highlightedHtml: html,
      replacedText: replaced,
    };
  }, [
    pattern,
    testText,
    replaceText,
    globalFlag,
    ignoreCase,
    multiline,
    dotAll,
    unicodeFlag,
    unicodeSetsFlag,
  ]);

  const applyPattern = (p: (typeof PATTERN_LIBRARY)[number]) => {
    setPattern(p.pattern);
    if (p.flags.includes("g")) setGlobalFlag(true);
    if (p.flags.includes("i")) setIgnoreCase(true);
    if (p.flags.includes("m")) setMultiline(true);
    if (p.flags.includes("s")) setDotAll(true);
    if (p.flags.includes("u")) setUnicodeFlag(true);
  };

  const matchColumns = [
    { title: "#", dataIndex: "idx", key: "idx", width: 40 },
    { title: "位置", dataIndex: "index", key: "index", width: 80 },
    { title: "长度", dataIndex: "length", key: "length", width: 60 },
    { title: "匹配值", dataIndex: "match", key: "match" },
    { title: "捕获组", dataIndex: "captures", key: "captures" },
    { title: "命名组", dataIndex: "namedGroups", key: "namedGroups" },
  ];

  const matchRows = matches.map((m: MatchResult, idx: number) => ({
    key: idx,
    idx: idx + 1,
    index: m.index,
    length: m.length,
    match: (
      <Typography.Text
        copyable
        className={styles.smallText}
        style={{ fontFamily: "monospace" }}
      >
        {m.match}
      </Typography.Text>
    ),
    captures:
      m.captures.length > 0 ? (
        <span className={styles.smallText}>
          {m.captures.map((c, i) => (
            <Tag key={i} style={{ marginBottom: 2 }}>
              {c.name}=&quot;{c.value}&quot;
            </Tag>
          ))}
        </span>
      ) : (
        "—"
      ),
    namedGroups:
      m.namedGroups.length > 0 ? (
        <span className={styles.smallText}>
          {m.namedGroups.map((g) => (
            <Tag key={g.name} color="blue" style={{ marginBottom: 2 }}>
              {g.name}=&quot;{g.value}&quot;
            </Tag>
          ))}
        </span>
      ) : (
        "—"
      ),
  }));

  return (
    <div className={styles.root}>
      <Typography.Text strong className={styles.sectionLabel}>
        正则表达式
      </Typography.Text>
      <Input
        value={pattern}
        onChange={(e) => setPattern(e.target.value)}
        placeholder="/pattern/flags"
        status={error ? "error" : undefined}
        style={{ fontFamily: "monospace", marginBottom: 4 }}
      />
      {error && (
        <Typography.Text type="danger" className={styles.errorText}>
          {error}
        </Typography.Text>
      )}

      <div className={styles.flagsRow}>
        <Checkbox
          checked={globalFlag}
          onChange={(e) => setGlobalFlag(e.target.checked)}
        >
          global (g)
        </Checkbox>
        <Checkbox
          checked={ignoreCase}
          onChange={(e) => setIgnoreCase(e.target.checked)}
        >
          ignoreCase (i)
        </Checkbox>
        <Checkbox
          checked={multiline}
          onChange={(e) => setMultiline(e.target.checked)}
        >
          multiline (m)
        </Checkbox>
        <Checkbox
          checked={dotAll}
          onChange={(e) => setDotAll(e.target.checked)}
        >
          dotAll (s)
        </Checkbox>
        <Checkbox
          checked={unicodeFlag}
          onChange={(e) => {
            setUnicodeFlag(e.target.checked);
            if (e.target.checked) setUnicodeSetsFlag(false);
          }}
        >
          unicode (u)
        </Checkbox>
        <Checkbox
          checked={unicodeSetsFlag}
          onChange={(e) => {
            setUnicodeSetsFlag(e.target.checked);
            if (e.target.checked) setUnicodeFlag(false);
          }}
        >
          unicodeSets (v)
        </Checkbox>
      </div>

      <Typography.Text strong className={styles.sectionLabel}>
        测试文本
      </Typography.Text>
      <TextArea
        value={testText}
        onChange={(e) => setTestText(e.target.value)}
        placeholder="输入测试文本..."
        rows={4}
        style={{ marginBottom: 12 }}
      />

      <Tabs
        items={[
          {
            key: "highlight",
            label: `匹配高亮 (${matches.length})`,
            children: (
              <div
                className={styles.highlightBox}
                dangerouslySetInnerHTML={{ __html: highlightedHtml }}
              />
            ),
          },
          {
            key: "details",
            label: "匹配明细",
            children:
              matches.length > 0 ? (
                <Table
                  columns={matchColumns}
                  dataSource={matchRows}
                  pagination={false}
                  size="small"
                  scroll={{ x: true }}
                />
              ) : (
                <Typography.Text type="secondary">
                  输入正则与文本，匹配结果将在这里展示。
                </Typography.Text>
              ),
          },
          {
            key: "replace",
            label: "替换预览",
            children: (
              <div>
                <Input
                  value={replaceText}
                  onChange={(e) => setReplaceText(e.target.value)}
                  placeholder="替换为…（支持 $1 $2 $<name> 反向引用）"
                  style={{ marginBottom: 8, fontFamily: "monospace" }}
                />
                {replacedText && (
                  <div className={styles.replaceResult}>{replacedText}</div>
                )}
              </div>
            ),
          },
          {
            key: "library",
            label: "常用模式",
            children: (
              <div className={styles.libraryTags}>
                {PATTERN_LIBRARY.map((p) => (
                  <Tag
                    key={p.name}
                    style={{ cursor: "pointer", marginBottom: 4 }}
                    color="processing"
                    onClick={() => applyPattern(p)}
                  >
                    {p.name}
                  </Tag>
                ))}
              </div>
            ),
          },
        ]}
      />
    </div>
  );
}
