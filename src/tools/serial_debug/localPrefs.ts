import type { LineEnding, SerialEncoding } from "./byteCodec";

export type SerialViewMode = "text" | "hex";

export type SendMode = SerialViewMode;

export interface SendHistoryEntry {
  mode: SendMode;
  text: string;
  lineEnding: LineEnding;
}

export interface SerialPrefs {
  portName: string | null;
  baudRate: number;
  encoding: SerialEncoding;
  lineEnding: LineEnding;
  timestamp: boolean;
  sendMode: SendMode;
  receiveView: SerialViewMode;
  history: SendHistoryEntry[];
}

const STORAGE_KEY = "serial_debug.prefs";
const HISTORY_LIMIT = 20;

const DEFAULTS: SerialPrefs = {
  portName: null,
  baudRate: 115200,
  encoding: "utf8",
  lineEnding: "none",
  timestamp: false,
  sendMode: "text",
  receiveView: "text",
  history: [],
};

const ENCODINGS: SerialEncoding[] = ["utf8", "gbk", "latin1"];
const LINE_ENDINGS: LineEnding[] = ["none", "cr", "lf", "crlf"];
const VIEW_MODES: SerialViewMode[] = ["text", "hex"];

function readRaw(): Record<string, unknown> {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (raw === null) return {};
    const parsed: unknown = JSON.parse(raw);
    return typeof parsed === "object" && parsed !== null
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

function isHistoryEntry(value: unknown): value is SendHistoryEntry {
  if (typeof value !== "object" || value === null) return false;
  const obj = value as Record<string, unknown>;
  return (
    (obj.mode === "text" || obj.mode === "hex") &&
    typeof obj.text === "string" &&
    typeof obj.lineEnding === "string" &&
    LINE_ENDINGS.includes(obj.lineEnding as LineEnding)
  );
}

export function loadPrefs(): SerialPrefs {
  const raw = readRaw();
  const history = Array.isArray(raw.history)
    ? raw.history.filter(isHistoryEntry).slice(-HISTORY_LIMIT)
    : [];
  return {
    portName: typeof raw.portName === "string" ? raw.portName : null,
    baudRate:
      typeof raw.baudRate === "number" && Number.isFinite(raw.baudRate)
        ? raw.baudRate
        : DEFAULTS.baudRate,
    encoding: ENCODINGS.includes(raw.encoding as SerialEncoding)
      ? (raw.encoding as SerialEncoding)
      : DEFAULTS.encoding,
    lineEnding: LINE_ENDINGS.includes(raw.lineEnding as LineEnding)
      ? (raw.lineEnding as LineEnding)
      : DEFAULTS.lineEnding,
    timestamp: raw.timestamp === true,
    sendMode: VIEW_MODES.includes(raw.sendMode as SerialViewMode)
      ? (raw.sendMode as SerialViewMode)
      : DEFAULTS.sendMode,
    receiveView: VIEW_MODES.includes(raw.receiveView as SerialViewMode)
      ? (raw.receiveView as SerialViewMode)
      : DEFAULTS.receiveView,
    history,
  };
}

export function savePrefs(patch: Partial<SerialPrefs>): SerialPrefs {
  const next = { ...loadPrefs(), ...patch };
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  } catch {
    return next;
  }
  return next;
}

export function pushSendHistory(entry: SendHistoryEntry): SerialPrefs {
  const current = loadPrefs();
  const history = [...current.history, entry].slice(-HISTORY_LIMIT);
  return savePrefs({ history });
}
