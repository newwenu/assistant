import { isTauri as detectTauri } from "@tauri-apps/api/core";
import * as tauriLog from "@tauri-apps/plugin-log";

export enum LogLevel {
  TRACE = 0,
  DEBUG = 1,
  INFO = 2,
  WARN = 3,
  ERROR = 4,
  NONE = 5,
}

const isDev = import.meta.env.DEV;

let currentLevel: LogLevel = isDev ? LogLevel.DEBUG : LogLevel.WARN;
let consoleForwardMinLevel: LogLevel = LogLevel.INFO;

const TAURI_LOG_FNS: Record<number, (message: string) => Promise<void>> = {
  [LogLevel.TRACE]: tauriLog.trace,
  [LogLevel.DEBUG]: tauriLog.debug,
  [LogLevel.INFO]: tauriLog.info,
  [LogLevel.WARN]: tauriLog.warn,
  [LogLevel.ERROR]: tauriLog.error,
};

function formatError(error: unknown): string {
  if (!(error instanceof Error)) return String(error);
  const { name, message, stack } = error;
  if (stack && !stack.includes(message)) {
    return `${name}: ${message}\n${stack}`;
  }
  return stack ?? `${name}: ${message}`;
}

function formatData(data: unknown): string {
  if (data instanceof Error) return formatError(data);
  if (data === undefined) return "undefined";
  if (data === null) return "null";
  if (typeof data === "string") return data;
  if (typeof data === "number" || typeof data === "boolean")
    return String(data);
  try {
    return JSON.stringify(data);
  } catch {
    return Object.prototype.toString.call(data);
  }
}

function formatConsoleArg(arg: unknown): string {
  if (arg instanceof Error) return formatError(arg);
  if (typeof arg === "string") return arg;
  if (typeof arg === "number" || typeof arg === "boolean") return String(arg);
  try {
    return JSON.stringify(arg);
  } catch {
    return String(arg);
  }
}

function formatConsoleArgs(args: unknown[]): string {
  return args.map(formatConsoleArg).join(" ");
}

async function writeTauri(level: LogLevel, message: string): Promise<void> {
  if (!detectTauri()) return;
  const fn = TAURI_LOG_FNS[level];
  if (!fn) return;
  try {
    await fn(message);
  } catch {
    // fire-and-forget
  }
}

export interface Logger {
  trace: (msg: string, data?: unknown) => void;
  debug: (msg: string, data?: unknown) => void;
  info: (msg: string, data?: unknown) => void;
  warn: (msg: string, data?: unknown) => void;
  error: (msg: string, data?: unknown) => void;
}

function createLogMethod(level: LogLevel, context: string) {
  const prefix = `[${context}]`;
  const consoleFn =
    level === LogLevel.ERROR
      ? console.error
      : level === LogLevel.WARN
        ? console.warn
        : level === LogLevel.INFO
          ? console.info
          : level === LogLevel.DEBUG
            ? console.debug
            : console.log;

  return (msg: string, data?: unknown): void => {
    if (level < currentLevel) return;

    const dataStr = data !== undefined ? ` ${formatData(data)}` : "";
    const fullMessage = `${prefix} ${msg}${dataStr}`;

    consoleFn(fullMessage);
    if (level >= consoleForwardMinLevel) {
      void writeTauri(level, fullMessage);
    }
  };
}

export function createLogger(context: string): Logger {
  return {
    trace: createLogMethod(LogLevel.TRACE, context),
    debug: createLogMethod(LogLevel.DEBUG, context),
    info: createLogMethod(LogLevel.INFO, context),
    warn: createLogMethod(LogLevel.WARN, context),
    error: createLogMethod(LogLevel.ERROR, context),
  };
}

export function setLogLevel(level: LogLevel): void {
  currentLevel = level;
}

export function setConsoleForwardMinLevel(level: LogLevel): void {
  consoleForwardMinLevel = level;
}

export function bootstrapLogger(): void {
  if (!detectTauri()) return;
  if (
    (globalThis as Record<string, unknown>)
      .__assistant_console_forward_installed__
  )
    return;

  const originals = {
    error: console.error.bind(console),
    warn: console.warn.bind(console),
    info: console.info.bind(console),
    debug: console.debug.bind(console),
    trace: console.trace.bind(console),
    log: console.log.bind(console),
  };

  const levelMap: Record<string, LogLevel> = {
    error: LogLevel.ERROR,
    warn: LogLevel.WARN,
    info: LogLevel.INFO,
    debug: LogLevel.DEBUG,
    trace: LogLevel.TRACE,
    log: LogLevel.INFO,
  };

  const install = (method: keyof typeof originals): void => {
    const level = levelMap[method] ?? LogLevel.INFO;
    const original = originals[method];
    Object.defineProperty(console, method, {
      value: (...args: unknown[]): void => {
        original(...args);
        if (level >= consoleForwardMinLevel) {
          const formatted = formatConsoleArgs(args);
          void writeTauri(level, formatted);
        }
      },
      writable: true,
      configurable: true,
    });
  };

  install("error");
  install("warn");
  install("info");
  install("debug");
  install("trace");
  install("log");

  (
    globalThis as Record<string, unknown>
  ).__assistant_console_forward_installed__ = true;
}
