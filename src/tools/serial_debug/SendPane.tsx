import { useCallback, useEffect, useRef, useState } from "react";
import {
  Button,
  Input,
  InputNumber,
  Select,
  Space,
  Switch,
  Typography,
} from "antd";
import { translateKey } from "@/i18n";
import type { LineEnding } from "./byteCodec";
import { parseHex } from "./byteCodec";
import type { SendHistoryEntry, SendMode } from "./localPrefs";

const MIN_INTERVAL_MS = 10;
const MAX_INTERVAL_MS = 60000;
const DEFAULT_INTERVAL_MS = 1000;

const LINE_ENDING_OPTIONS: { value: LineEnding; key: string }[] = [
  { value: "none", key: "tools.serial_debug.lineNone" },
  { value: "cr", key: "tools.serial_debug.lineCr" },
  { value: "lf", key: "tools.serial_debug.lineLf" },
  { value: "crlf", key: "tools.serial_debug.lineCrlf" },
];

export interface SendPayload {
  mode: SendMode;
  text: string;
  lineEnding: LineEnding;
}

interface SendPaneProps {
  open: boolean;
  history: SendHistoryEntry[];
  lineEnding: LineEnding;
  onLineEndingChange: (lineEnding: LineEnding) => void;
  onSend: (payload: SendPayload) => Promise<boolean>;
}

export function SendPane({
  open,
  history,
  lineEnding,
  onLineEndingChange,
  onSend,
}: SendPaneProps) {
  const [mode, setMode] = useState<SendMode>("text");
  const [text, setText] = useState("");
  const [repeating, setRepeating] = useState(false);
  const [intervalMs, setIntervalMs] = useState(DEFAULT_INTERVAL_MS);

  const sendRef = useRef(onSend);
  sendRef.current = onSend;
  const payloadRef = useRef<SendPayload>({ mode, text, lineEnding });
  payloadRef.current = { mode, text, lineEnding };

  const hexInvalid = mode === "hex" && text.length > 0 && !parseHex(text).ok;

  const canSend = open && text.length > 0 && !hexInvalid;

  useEffect(() => {
    if (!open) setRepeating(false);
  }, [open]);

  useEffect(() => {
    if (!repeating || !open) return;
    const timer = window.setInterval(() => {
      void sendRef.current(payloadRef.current).then((ok) => {
        if (!ok) setRepeating(false);
      });
    }, intervalMs);
    return () => window.clearInterval(timer);
  }, [repeating, open, intervalMs]);

  const handleSend = useCallback(() => {
    if (!canSend) return;
    void onSend({ mode, text, lineEnding });
  }, [canSend, lineEnding, mode, onSend, text]);

  const handleHistory = useCallback(
    (index: number | undefined) => {
      if (index === undefined) return;
      const entry = history[index];
      if (entry === undefined) return;
      setMode(entry.mode);
      setText(entry.text);
      onLineEndingChange(entry.lineEnding);
    },
    [history, onLineEndingChange],
  );

  return (
    <Space orientation="vertical" size={8} style={{ width: "100%" }}>
      <Space wrap size={8}>
        <Typography.Text type="secondary">
          {translateKey("tools.serial_debug.send")}
        </Typography.Text>
        <Space size={4}>
          <Typography.Text type="secondary">
            {translateKey("tools.serial_debug.viewHex")}
          </Typography.Text>
          <Switch
            size="small"
            checked={mode === "hex"}
            onChange={(checked) => setMode(checked ? "hex" : "text")}
          />
        </Space>
        <Typography.Text type="secondary">
          {translateKey("tools.serial_debug.lineEnding")}
        </Typography.Text>
        <Select
          size="small"
          style={{ width: 110 }}
          value={lineEnding}
          onChange={(value: LineEnding) => onLineEndingChange(value)}
          options={LINE_ENDING_OPTIONS.map((item) => ({
            value: item.value,
            label: translateKey(item.key),
          }))}
        />
        <Typography.Text type="secondary">
          {translateKey("tools.serial_debug.history")}
        </Typography.Text>
        <Select
          size="small"
          style={{ minWidth: 220 }}
          value={null}
          placeholder={translateKey(
            history.length === 0
              ? "tools.serial_debug.historyEmpty"
              : "tools.serial_debug.history",
          )}
          onChange={(value: number) => handleHistory(value)}
          options={history.map((entry, index) => ({
            value: index,
            label: `[${entry.mode}] ${entry.text}`,
          }))}
          disabled={history.length === 0}
        />
        <Space size={4}>
          <Typography.Text type="secondary">
            {translateKey("tools.serial_debug.repeat")}
          </Typography.Text>
          <Switch
            size="small"
            checked={repeating}
            onChange={setRepeating}
            disabled={!canSend}
          />
        </Space>
        <Typography.Text type="secondary">
          {translateKey("tools.serial_debug.repeatInterval")}
        </Typography.Text>
        <InputNumber
          size="small"
          style={{ width: 110 }}
          min={MIN_INTERVAL_MS}
          max={MAX_INTERVAL_MS}
          step={10}
          value={intervalMs}
          onChange={(value) => {
            if (typeof value === "number") setIntervalMs(value);
          }}
          suffix="ms"
        />
        <Button type="primary" onClick={handleSend} disabled={!canSend}>
          {translateKey("tools.serial_debug.send")}
        </Button>
      </Space>

      <Input.TextArea
        value={text}
        onChange={(event) => setText(event.target.value)}
        onPressEnter={(event) => {
          if (event.shiftKey) return;
          event.preventDefault();
          handleSend();
        }}
        autoSize={{ minRows: 4, maxRows: 8 }}
        placeholder={translateKey("tools.serial_debug.sendHint")}
        status={hexInvalid ? "error" : undefined}
        style={{ fontFamily: "monospace" }}
        disabled={!open}
      />
    </Space>
  );
}
