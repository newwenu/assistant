import { useState, useMemo } from "react";
import { Input, Typography, Segmented } from "antd";
import { diffLines, diffChars } from "diff";
import styles from "./text-diff.module.css";

const { TextArea } = Input;

type ViewMode = "unified" | "split";

interface DiffLine {
  type: "add" | "remove" | "same";
  content: string;
  lineNumOld?: number;
  lineNumNew?: number;
  charParts?: { text: string; added?: boolean; removed?: boolean }[];
}

function computeDiff(oldText: string, newText: string): DiffLine[] {
  const changes = diffLines(oldText, newText);
  const result: DiffLine[] = [];
  let lineNumOld = 1;
  let lineNumNew = 1;

  for (const change of changes) {
    const lines = change.value.replace(/\n$/, "").split("\n");
    for (const line of lines) {
      if (change.added) {
        let charParts: DiffLine["charParts"];
        if (result.length > 0) {
          const prev = result[result.length - 1];
          if (prev.type === "remove" && prev.content === line) {
            const charDiff = diffChars(prev.content, line);
            prev.charParts = charDiff
              .filter((c) => !c.added)
              .map((c) => ({
                text: c.value,
                removed: !c.added && !c.removed ? undefined : c.removed,
              }));
            prev.type = "same";
            prev.lineNumNew = lineNumNew;
            lineNumNew++;
            lineNumOld++;
            continue;
          }
        }
        result.push({
          type: "add",
          content: line,
          lineNumNew: lineNumNew++,
          charParts,
        });
      } else if (change.removed) {
        result.push({
          type: "remove",
          content: line,
          lineNumOld: lineNumOld++,
        });
      } else {
        result.push({
          type: "same",
          content: line,
          lineNumOld: lineNumOld++,
          lineNumNew: lineNumNew++,
        });
      }
    }
  }

  const final: DiffLine[] = [];
  let i = 0;
  while (i < result.length) {
    if (result[i].type === "remove") {
      const removeGroup: DiffLine[] = [];
      while (i < result.length && result[i].type === "remove") {
        removeGroup.push(result[i++]);
      }
      const addGroup: DiffLine[] = [];
      while (i < result.length && result[i].type === "add") {
        addGroup.push(result[i++]);
      }
      const pairLen = Math.min(removeGroup.length, addGroup.length);
      for (let p = 0; p < pairLen; p++) {
        const charDiff = diffChars(removeGroup[p].content, addGroup[p].content);
        final.push({
          type: "remove",
          content: removeGroup[p].content,
          lineNumOld: removeGroup[p].lineNumOld,
          charParts: charDiff
            .filter((c) => !c.added)
            .map((c) => ({
              text: c.value,
              removed: c.removed || undefined,
            })),
        });
        final.push({
          type: "add",
          content: addGroup[p].content,
          lineNumNew: addGroup[p].lineNumNew,
          charParts: charDiff
            .filter((c) => !c.removed)
            .map((c) => ({
              text: c.value,
              added: c.added || undefined,
            })),
        });
      }
      for (let p = pairLen; p < removeGroup.length; p++) {
        final.push(removeGroup[p]);
      }
      for (let p = pairLen; p < addGroup.length; p++) {
        final.push(addGroup[p]);
      }
    } else {
      final.push(result[i++]);
    }
  }

  return final;
}

const LINE_COLORS: Record<string, string> = {
  add: "var(--diff-add-bg)",
  remove: "var(--diff-remove-bg)",
  same: "transparent",
};

const PREFIX: Record<string, string> = {
  add: "+ ",
  remove: "- ",
  same: "  ",
};

function renderCharParts(
  parts: { text: string; added?: boolean; removed?: boolean }[],
  baseColor: string,
) {
  return parts.map((part, idx) => {
    if (part.added) {
      return (
        <span key={idx} className={styles.charAdd}>
          {part.text}
        </span>
      );
    }
    if (part.removed) {
      return (
        <span key={idx} className={styles.charRemove}>
          {part.text}
        </span>
      );
    }
    return (
      <span key={idx} style={{ color: baseColor }}>
        {part.text}
      </span>
    );
  });
}

export default function TextDiff() {
  const [oldText, setOldText] = useState("");
  const [newText, setNewText] = useState("");
  const [viewMode, setViewMode] = useState<ViewMode>("unified");

  const diff = useMemo(() => computeDiff(oldText, newText), [oldText, newText]);

  const addCount = diff.filter((d) => d.type === "add").length;
  const removeCount = diff.filter((d) => d.type === "remove").length;

  return (
    <div className={styles.container}>
      <div className={styles.headerRow}>
        <Segmented
          options={[
            { value: "unified", label: "统一视图" },
            { value: "split", label: "拆分视图" },
          ]}
          value={viewMode}
          onChange={(v) => setViewMode(v as ViewMode)}
        />
        <Typography.Text type="secondary" className={styles.countText}>
          <span style={{ color: "var(--diff-add-text)" }}>+{addCount}</span>{" "}
          <span style={{ color: "var(--diff-remove-text)" }}>
            -{removeCount}
          </span>
        </Typography.Text>
      </div>

      {viewMode === "split" ? (
        <div className={styles.sideBySide}>
          <div className={styles.panel}>
            <Typography.Text strong className={styles.sectionLabel}>
              原始文本
            </Typography.Text>
            <TextArea
              value={oldText}
              onChange={(e) => setOldText(e.target.value)}
              rows={12}
              style={{ fontFamily: "monospace" }}
            />
          </div>
          <div className={styles.panel}>
            <Typography.Text strong className={styles.sectionLabel}>
              修改文本
            </Typography.Text>
            <TextArea
              value={newText}
              onChange={(e) => setNewText(e.target.value)}
              rows={12}
              style={{ fontFamily: "monospace" }}
            />
          </div>
        </div>
      ) : (
        <>
          <div className={styles.sideBySide}>
            <div className={styles.panel}>
              <Typography.Text strong className={styles.sectionLabel}>
                原始文本
              </Typography.Text>
              <TextArea
                value={oldText}
                onChange={(e) => setOldText(e.target.value)}
                rows={6}
                style={{ fontFamily: "monospace" }}
              />
            </div>
            <div className={styles.panel}>
              <Typography.Text strong className={styles.sectionLabel}>
                修改文本
              </Typography.Text>
              <TextArea
                value={newText}
                onChange={(e) => setNewText(e.target.value)}
                rows={6}
                style={{ fontFamily: "monospace" }}
              />
            </div>
          </div>
          <div className={styles.diffContainer}>
            {diff.map((line, i) => (
              <div
                key={i}
                className={styles.diffLine}
                style={{ background: LINE_COLORS[line.type] }}
              >
                <span className={styles.lineNum}>{line.lineNumOld ?? ""}</span>
                <span className={styles.lineNum}>{line.lineNumNew ?? ""}</span>
                <span
                  style={{
                    color:
                      line.type === "add"
                        ? "var(--diff-add-text)"
                        : line.type === "remove"
                          ? "var(--diff-remove-text)"
                          : "inherit",
                  }}
                >
                  {PREFIX[line.type]}
                  {line.charParts
                    ? renderCharParts(
                        line.charParts,
                        line.type === "add"
                          ? "var(--diff-add-text)"
                          : line.type === "remove"
                            ? "var(--diff-remove-text)"
                            : "inherit",
                      )
                    : line.content}
                </span>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
