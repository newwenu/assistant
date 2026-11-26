import { useCallback, useMemo, useRef, useState, useEffect } from "react";
import {
  Alert,
  App,
  Button,
  Segmented,
  Select,
  Space,
  Switch,
  Typography,
} from "antd";
import { translateKey } from "@/i18n";
import type { SerialEncoding } from "./byteCodec";
import { decodeText, formatHex, formatTimestamp } from "./byteCodec";
import type { SerialViewMode } from "./localPrefs";
import type { SerialLine } from "./useSerialSession";

export type ViewMode = SerialViewMode;

const ENCODING_OPTIONS: { value: SerialEncoding; key: string }[] = [
  { value: "utf8", key: "tools.serial_debug.encodingUtf8" },
  { value: "gbk", key: "tools.serial_debug.encodingGbk" },
  { value: "latin1", key: "tools.serial_debug.encodingLatin1" },
];

interface ReceivePaneProps {
  lines: SerialLine[];
  view: ViewMode;
  encoding: SerialEncoding;
  timestamp: boolean;
  paused: boolean;
  truncated: boolean;
  rxBytes: number;
  txBytes: number;
  onViewChange: (view: ViewMode) => void;
  onEncodingChange: (encoding: SerialEncoding) => void;
  onTimestampChange: (timestamp: boolean) => void;
  onPauseChange: (paused: boolean) => void;
  onClear: () => void;
}

export function ReceivePane({
  lines,
  view,
  encoding,
  timestamp,
  paused,
  truncated,
  rxBytes,
  txBytes,
  onViewChange,
  onEncodingChange,
  onTimestampChange,
  onPauseChange,
  onClear,
}: ReceivePaneProps) {
  const { message } = App.useApp();
  const containerRef = useRef<HTMLDivElement>(null);
  const [follow, setFollow] = useState(true);

  const rows = useMemo(() => {
    return lines.map((line) => {
      const body =
        view === "hex"
          ? formatHex(line.bytes)
          : decodeText(line.bytes, encoding);
      return timestamp ? `${formatTimestamp(line.atMs)} ${body}` : body;
    });
  }, [lines, view, encoding, timestamp]);

  useEffect(() => {
    if (!follow || paused) return;
    const element = containerRef.current;
    if (element === null) return;
    element.scrollTop = element.scrollHeight;
  }, [rows, follow, paused]);

  const handleScroll = useCallback(() => {
    const element = containerRef.current;
    if (element === null) return;
    const atBottom =
      element.scrollHeight - element.scrollTop - element.clientHeight <= 8;
    setFollow(atBottom);
  }, []);

  const handleCopy = useCallback(() => {
    void navigator.clipboard
      .writeText(rows.join("\n"))
      .then(() => message.success(translateKey("tools.serial_debug.copied")))
      .catch(() => undefined);
  }, [message, rows]);

  return (
    <Space orientation="vertical" size={8} style={{ width: "100%" }}>
      <Space wrap size={8}>
        <Typography.Text type="secondary">
          {translateKey("tools.serial_debug.receive")}
        </Typography.Text>
        <Segmented
          size="small"
          value={view}
          onChange={(value) => onViewChange(value as ViewMode)}
          options={[
            {
              value: "text",
              label: translateKey("tools.serial_debug.viewText"),
            },
            { value: "hex", label: translateKey("tools.serial_debug.viewHex") },
          ]}
        />
        <Typography.Text type="secondary">
          {translateKey("tools.serial_debug.encoding")}
        </Typography.Text>
        <Select
          size="small"
          style={{ width: 110 }}
          value={encoding}
          onChange={(value: SerialEncoding) => onEncodingChange(value)}
          options={ENCODING_OPTIONS.map((item) => ({
            value: item.value,
            label: translateKey(item.key),
          }))}
        />
        <Typography.Text type="secondary">
          {translateKey("tools.serial_debug.timestamp")}
        </Typography.Text>
        <Switch size="small" checked={timestamp} onChange={onTimestampChange} />
        <Typography.Text type="secondary">
          {translateKey("tools.serial_debug.autoscroll")}
        </Typography.Text>
        <Switch size="small" checked={follow} onChange={setFollow} />
        <Button size="small" onClick={() => onPauseChange(!paused)}>
          {translateKey(
            paused ? "tools.serial_debug.resume" : "tools.serial_debug.pause",
          )}
        </Button>
        <Button size="small" onClick={onClear}>
          {translateKey("tools.serial_debug.clear")}
        </Button>
        <Button size="small" onClick={handleCopy} disabled={rows.length === 0}>
          {translateKey("tools.serial_debug.copy")}
        </Button>
        <Typography.Text type="secondary">
          {`${translateKey("tools.serial_debug.rxBytes")}: ${rxBytes}`}
        </Typography.Text>
        <Typography.Text type="secondary">
          {`${translateKey("tools.serial_debug.txBytes")}: ${txBytes}`}
        </Typography.Text>
      </Space>

      {truncated && (
        <Alert
          type="warning"
          showIcon
          title={translateKey("tools.serial_debug.truncated")}
        />
      )}

      <div
        ref={containerRef}
        onScroll={handleScroll}
        style={{
          height: 320,
          overflowY: "auto",
          padding: 8,
          border: "1px solid var(--color-border-secondary)",
          borderRadius: 6,
          background: "var(--color-bg-container)",
          fontFamily: "monospace",
          fontSize: 12,
          whiteSpace: "pre-wrap",
          wordBreak: "break-all",
        }}
      >
        {rows.join("\n")}
      </div>
    </Space>
  );
}
