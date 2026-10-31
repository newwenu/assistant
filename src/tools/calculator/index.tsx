import { useState, useCallback } from "react";
import { Input, Button, Typography, message } from "antd";
import { CopyOutlined, DeleteOutlined } from "@ant-design/icons";
import { safeEvaluate } from "./calculator.service";
import styles from "./calculator.module.css";

interface HistoryEntry {
  expression: string;
  result: string;
}

export default function Calculator() {
  const [expression, setExpression] = useState("");
  const [history, setHistory] = useState<HistoryEntry[]>([]);

  const result = safeEvaluate(expression);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && result !== "") {
        setHistory((h) => [{ expression, result }, ...h.slice(0, 49)]);
        setExpression(result);
      }
    },
    [expression, result],
  );

  const copy = (text: string) => {
    void navigator.clipboard.writeText(text);
    message.success("已复制");
  };

  const clearHistory = () => setHistory([]);

  return (
    <div>
      <Input
        value={expression}
        onChange={(e) => setExpression(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="输入数学表达式，如 2*sqrt(6)、sin(pi/4)..."
        className={styles.input}
        style={{ fontFamily: "monospace" }}
      />

      {result !== "" && (
        <div className={styles.resultCard}>
          <div className={styles.resultLabel}>结果</div>
          <div className={styles.resultValue}>
            {result}
            <Button
              type="text"
              size="small"
              icon={<CopyOutlined />}
              onClick={() => copy(result)}
            />
          </div>
        </div>
      )}

      {history.length > 0 && (
        <div className={styles.historySection}>
          <div className={styles.historyHeader}>
            <Typography.Text strong>历史记录</Typography.Text>
            <Button
              size="small"
              icon={<DeleteOutlined />}
              onClick={clearHistory}
            >
              清空
            </Button>
          </div>
          <div className={styles.historyList}>
            {history.map((item, index) => (
              <div key={index} className={styles.historyItem}>
                <span
                  className={styles.historyExpr}
                  style={{ fontFamily: "monospace" }}
                >
                  {item.expression}
                </span>
                <div className={styles.historyResult}>
                  <span style={{ fontFamily: "monospace" }}>{item.result}</span>
                  <Button
                    type="text"
                    size="small"
                    icon={<CopyOutlined />}
                    onClick={() => copy(item.result)}
                  />
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
