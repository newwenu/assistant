import { useState, useMemo } from "react";
import { Input, Statistic, Row, Col } from "antd";

const { TextArea } = Input;

export default function TextStatistics() {
  const [text, setText] = useState("");

  const stats = useMemo(() => {
    const charCount = text.length;
    const wordCount = text.trim() === "" ? 0 : text.trim().split(/\s+/).length;
    const lineCount = text === "" ? 0 : text.split(/\r\n|\r|\n/).length;
    const byteSize = new TextEncoder().encode(text).length;
    return { charCount, wordCount, lineCount, byteSize };
  }, [text]);

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  };

  return (
    <div>
      <TextArea
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder="输入文本..."
        rows={6}
        style={{ marginBottom: 16 }}
      />
      <Row gutter={16}>
        <Col span={6}>
          <Statistic title="字符数" value={stats.charCount} />
        </Col>
        <Col span={6}>
          <Statistic title="单词数" value={stats.wordCount} />
        </Col>
        <Col span={6}>
          <Statistic title="行数" value={stats.lineCount} />
        </Col>
        <Col span={6}>
          <Statistic title="字节大小" value={formatBytes(stats.byteSize)} />
        </Col>
      </Row>
    </div>
  );
}
