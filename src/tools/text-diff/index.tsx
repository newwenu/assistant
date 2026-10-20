import { useState, useMemo } from "react";
import { Input, Typography, Segmented } from "antd";
import {
  computeDiff,
  LINE_COLORS,
  PREFIX,
  type DiffLine,
} from "./text-diff.service";
import styles from "./text-diff.module.css";

const { TextArea } = Input;

type ViewMode = "unified" | "split";

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
            {diff.map((line: DiffLine, i: number) => (
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
