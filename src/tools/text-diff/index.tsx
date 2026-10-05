import { useState, useMemo } from "react";
import { Input, Typography, Segmented } from "antd";

const { TextArea } = Input;

type ViewMode = "unified" | "split";

interface DiffLine {
  type: "add" | "remove" | "same";
  content: string;
  lineNumOld?: number;
  lineNumNew?: number;
}

function computeDiff(oldText: string, newText: string): DiffLine[] {
  const oldLines = oldText.split("\n");
  const newLines = newText.split("\n");
  const result: DiffLine[] = [];

  let oi = 0,
    ni = 0;

  while (oi < oldLines.length || ni < newLines.length) {
    if (
      oi < oldLines.length &&
      ni < newLines.length &&
      oldLines[oi] === newLines[ni]
    ) {
      result.push({
        type: "same",
        content: oldLines[oi],
        lineNumOld: oi + 1,
        lineNumNew: ni + 1,
      });
      oi++;
      ni++;
    } else {
      let found = false;
      for (let look = 1; look <= 3 && !found; look++) {
        if (
          ni + look < newLines.length &&
          oldLines[oi] === newLines[ni + look]
        ) {
          for (let k = 0; k < look; k++) {
            result.push({
              type: "add",
              content: newLines[ni + k],
              lineNumNew: ni + k + 1,
            });
          }
          ni += look;
          found = true;
        }
        if (
          oi + look < oldLines.length &&
          newLines[ni] === oldLines[oi + look]
        ) {
          for (let k = 0; k < look; k++) {
            result.push({
              type: "remove",
              content: oldLines[oi + k],
              lineNumOld: oi + k + 1,
            });
          }
          oi += look;
          found = true;
        }
      }
      if (!found) {
        if (oi < oldLines.length) {
          result.push({
            type: "remove",
            content: oldLines[oi],
            lineNumOld: oi + 1,
          });
          oi++;
        }
        if (ni < newLines.length) {
          result.push({
            type: "add",
            content: newLines[ni],
            lineNumNew: ni + 1,
          });
          ni++;
        }
      }
    }
  }
  return result;
}

const LINE_COLORS: Record<string, string> = {
  add: "#e6ffed",
  remove: "#ffeef0",
  same: "transparent",
};

const PREFIX: Record<string, string> = {
  add: "+ ",
  remove: "- ",
  same: "  ",
};

export default function TextDiff() {
  const [oldText, setOldText] = useState("");
  const [newText, setNewText] = useState("");
  const [viewMode, setViewMode] = useState<ViewMode>("unified");

  const diff = useMemo(() => computeDiff(oldText, newText), [oldText, newText]);

  const addCount = diff.filter((d) => d.type === "add").length;
  const removeCount = diff.filter((d) => d.type === "remove").length;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
        <Segmented
          options={[
            { value: "unified", label: "统一视图" },
            { value: "split", label: "拆分视图" },
          ]}
          value={viewMode}
          onChange={(v) => setViewMode(v as ViewMode)}
        />
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          <span style={{ color: "#52c41a" }}>+{addCount}</span>{" "}
          <span style={{ color: "#ff4d4f" }}>-{removeCount}</span>
        </Typography.Text>
      </div>

      {viewMode === "split" ? (
        <div style={{ display: "flex", gap: 8 }}>
          <div style={{ flex: 1 }}>
            <Typography.Text
              strong
              style={{ display: "block", marginBottom: 4 }}
            >
              原始文本
            </Typography.Text>
            <TextArea
              value={oldText}
              onChange={(e) => setOldText(e.target.value)}
              rows={12}
              style={{ fontFamily: "monospace" }}
            />
          </div>
          <div style={{ flex: 1 }}>
            <Typography.Text
              strong
              style={{ display: "block", marginBottom: 4 }}
            >
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
          <div style={{ display: "flex", gap: 8 }}>
            <div style={{ flex: 1 }}>
              <Typography.Text
                strong
                style={{ display: "block", marginBottom: 4 }}
              >
                原始文本
              </Typography.Text>
              <TextArea
                value={oldText}
                onChange={(e) => setOldText(e.target.value)}
                rows={6}
                style={{ fontFamily: "monospace" }}
              />
            </div>
            <div style={{ flex: 1 }}>
              <Typography.Text
                strong
                style={{ display: "block", marginBottom: 4 }}
              >
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
          <div
            style={{
              border: "1px solid var(--color-border)",
              borderRadius: 6,
              padding: 8,
              fontFamily: "monospace",
              fontSize: 13,
              maxHeight: 400,
              overflow: "auto",
              whiteSpace: "pre",
            }}
          >
            {diff.map((line, i) => (
              <div
                key={i}
                style={{
                  background: LINE_COLORS[line.type],
                  padding: "0 4px",
                  minHeight: 20,
                }}
              >
                <span
                  style={{
                    opacity: 0.5,
                    marginRight: 8,
                    display: "inline-block",
                    width: 30,
                    textAlign: "right",
                  }}
                >
                  {line.lineNumOld ?? ""}
                </span>
                <span
                  style={{
                    opacity: 0.5,
                    marginRight: 8,
                    display: "inline-block",
                    width: 30,
                    textAlign: "right",
                  }}
                >
                  {line.lineNumNew ?? ""}
                </span>
                <span
                  style={{
                    color:
                      line.type === "add"
                        ? "#52c41a"
                        : line.type === "remove"
                          ? "#ff4d4f"
                          : "inherit",
                  }}
                >
                  {PREFIX[line.type]}
                  {line.content}
                </span>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
