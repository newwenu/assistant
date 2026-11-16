const REACT_ERRORS_BASE_URL = "https://react.dev/errors";

const REACT_HINTS: Record<number, string> = {
  300: "Rendered more hooks than during the previous render.",
  301: "Too many re-renders. React limits the number of renders to prevent an infinite loop.",
  310: "Rendered fewer hooks than expected. This may be caused by an early return statement.",
};

interface ParsedReactError {
  code: number;
  decoderUrl: string;
  hint?: string;
}

export function isMinifiedReactErrorMessage(message: string): boolean {
  return /^Minified React error #\d+/.test(message);
}

export function parseMinifiedReactError(
  message: string,
): ParsedReactError | null {
  const match = message.match(/^Minified React error #(\d+)/);
  if (!match) return null;

  const code = parseInt(match[1], 10);
  return {
    code,
    decoderUrl: `${REACT_ERRORS_BASE_URL}/${code}`,
    hint: REACT_HINTS[code],
  };
}

interface ReactCrashLogPayload {
  name: string;
  message: string;
  stack?: string;
  componentStack?: string;
  reactCode?: number;
  reactDecoderUrl?: string;
  reactHint?: string;
}

export function safeReactErrorInfo(
  errorInfo: React.ErrorInfo | undefined,
): string | undefined {
  if (!errorInfo) return undefined;
  return errorInfo.componentStack ?? undefined;
}

export function buildReactCrashLogPayload(
  error: Error,
  errorInfo?: React.ErrorInfo,
): ReactCrashLogPayload {
  const payload: ReactCrashLogPayload = {
    name: error.name,
    message: error.message,
    stack: error.stack,
    componentStack: safeReactErrorInfo(errorInfo),
  };

  const parsed = parseMinifiedReactError(error.message);
  if (parsed) {
    payload.reactCode = parsed.code;
    payload.reactDecoderUrl = parsed.decoderUrl;
    payload.reactHint = parsed.hint;
  }

  return payload;
}
