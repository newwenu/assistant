import { useState, useMemo } from "react";
import { Input, Checkbox, Typography, Tag } from "antd";

const { TextArea } = Input;

interface MatchResult {
  match: string;
  index: number;
  groups: Record<string, string> | undefined;
}

export default function RegexTester() {
  const [pattern, setPattern] = useState("");
  const [testText, setTestText] = useState("");
  const [globalFlag, setGlobalFlag] = useState(true);
  const [ignoreCase, setIgnoreCase] = useState(false);
  const [multiline, setMultiline] = useState(false);
  const [dotAll, setDotAll] = useState(false);

  const { matches, error, highlightedHtml } = useMemo(() => {
    if (!pattern)
      return { matches: [], error: null, highlightedHtml: testText };

    let flags = "d";
    if (globalFlag) flags += "g";
    if (ignoreCase) flags += "i";
    if (multiline) flags += "m";
    if (dotAll) flags += "s";

    try {
      const regex = new RegExp(pattern, flags);
      const results: MatchResult[] = [];
      let m: RegExpExecArray | null;

      if (globalFlag) {
        while ((m = regex.exec(testText)) !== null) {
          results.push({
            match: m[0],
            index: m.index,
            groups: m.groups,
          });
          if (m[0].length === 0) regex.lastIndex++;
        }
      } else {
        m = regex.exec(testText);
        if (m) {
          results.push({
            match: m[0],
            index: m.index,
            groups: m.groups,
          });
        }
      }

      let html = testText;
      if (results.length > 0) {
        const parts: { text: string; isMatch: boolean }[] = [];
        let lastIdx = 0;
        const sorted = [...results].sort((a, b) => a.index - b.index);
        for (const r of sorted) {
          if (r.index > lastIdx) {
            parts.push({
              text: testText.slice(lastIdx, r.index),
              isMatch: false,
            });
          }
          parts.push({ text: r.match, isMatch: true });
          lastIdx = r.index + r.match.length;
        }
        if (lastIdx < testText.length) {
          parts.push({ text: testText.slice(lastIdx), isMatch: false });
        }
        html = parts
          .map((p) =>
            p.isMatch
              ? `<mark style="background:#ffe58f;padding:0 2px;border-radius:2px">${p.text}</mark>`
              : p.text.replace(/</g, "&lt;").replace(/>/g, "&gt;"),
          )
          .join("");
      }

      return { matches: results, error: null, highlightedHtml: html };
    } catch (e: any) {
      return { matches: [], error: e.message, highlightedHtml: testText };
    }
  }, [pattern, testText, globalFlag, ignoreCase, multiline, dotAll]);

  return (
    <div>
      <Typography.Text strong style={{ display: "block", marginBottom: 4 }}>
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
        <Typography.Text
          type="danger"
          style={{ display: "block", marginBottom: 8, fontSize: 12 }}
        >
          {error}
        </Typography.Text>
      )}

      <div
        style={{ display: "flex", gap: 12, marginBottom: 12, flexWrap: "wrap" }}
      >
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
      </div>

      <Typography.Text strong style={{ display: "block", marginBottom: 4 }}>
        测试文本
      </Typography.Text>
      <TextArea
        value={testText}
        onChange={(e) => setTestText(e.target.value)}
        placeholder="输入测试文本..."
        rows={4}
        style={{ marginBottom: 12 }}
      />

      <Typography.Text strong style={{ display: "block", marginBottom: 4 }}>
        匹配结果 ({matches.length})
      </Typography.Text>
      <div
        style={{
          padding: 8,
          border: "1px solid var(--color-border)",
          borderRadius: 6,
          marginBottom: 12,
          minHeight: 40,
          whiteSpace: "pre-wrap",
          wordBreak: "break-all",
          fontSize: 13,
        }}
        dangerouslySetInnerHTML={{ __html: highlightedHtml }}
      />

      {matches.length > 0 && matches[0].groups && (
        <>
          <Typography.Text strong style={{ display: "block", marginBottom: 4 }}>
            捕获组
          </Typography.Text>
          <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
            {Object.entries(matches[0].groups).map(([key, val]) => (
              <Tag key={key} color="blue">
                {key}: {val}
              </Tag>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
