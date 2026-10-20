import { useState, useMemo } from "react";
import { Input, Statistic, Row, Col } from "antd";
import { computeTextStats, formatBytes } from "./text-statistics.service";
import styles from "./text-statistics.module.css";

const { TextArea } = Input;

export default function TextStatistics() {
  const [text, setText] = useState("");

  const stats = useMemo(() => computeTextStats(text), [text]);

  return (
    <div>
      <TextArea
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder="输入文本..."
        rows={6}
        className={styles.textArea}
      />
      <Row gutter={[16, 16]}>
        <Col span={6}>
          <Statistic title="字符数" value={stats.charCount} />
        </Col>
        <Col span={6}>
          <Statistic title="单词数" value={stats.wordCount} />
        </Col>
        <Col span={6}>
          <Statistic title="中文字数" value={stats.cjkCount} />
        </Col>
        <Col span={6}>
          <Statistic title="行数" value={stats.lineCount} />
        </Col>
        <Col span={6}>
          <Statistic title="句子数" value={stats.sentenceCount} />
        </Col>
        <Col span={6}>
          <Statistic title="段落数" value={stats.paragraphCount} />
        </Col>
        <Col span={6}>
          <Statistic title="字节大小" value={formatBytes(stats.byteSize)} />
        </Col>
      </Row>
    </div>
  );
}
