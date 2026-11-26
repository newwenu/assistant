import { useCallback, useEffect, useRef, useState } from "react";
import { Alert, Space } from "antd";
import type {
  SerialDebugErrorCodeDto,
  SerialPortInfoDto,
  SerialSessionDto,
} from "@/types/protocol.contract";
import type { LineEnding, SerialEncoding } from "./byteCodec";
import {
  appendLineEnding,
  bytesToBase64,
  parseHex,
  textToBase64,
} from "./byteCodec";
import {
  loadPrefs,
  pushSendHistory,
  savePrefs,
  type SendHistoryEntry,
} from "./localPrefs";
import { PortToolbar, type SerialConfigDraft } from "./PortToolbar";
import { ReceivePane, type ViewMode } from "./ReceivePane";
import { SendPane, type SendPayload } from "./SendPane";
import {
  isSerialDebugErrorDto,
  serialDebugErrorCode,
  serialDebugService,
} from "./serialDebug.service";
import { createLogger } from "@/services/system/logger";
import { serialDebugErrorText, useSerialSession } from "./useSerialSession";

const log = createLogger("serialDebug");

function logCommandError(value: unknown): void {
  if (isSerialDebugErrorDto(value)) {
    log.debug(`command failed ${value.code}`, value.message);
  }
}

export default function SerialDebug() {
  const [prefs, setPrefs] = useState(loadPrefs);
  const [config, setConfig] = useState<SerialConfigDraft>(() => ({
    portName: prefs.portName,
    baudRate: prefs.baudRate,
    dataBits: "eight",
    parity: "none",
    stopBits: "one",
    flowControl: "none",
  }));
  const [ports, setPorts] = useState<SerialPortInfoDto[]>([]);
  const [view, setView] = useState<ViewMode>("text");
  const [errorCode, setErrorCode] = useState<SerialDebugErrorCodeDto | null>(
    null,
  );
  const [busy, setBusy] = useState(false);
  const [dtr, setDtr] = useState(false);
  const [rts, setRts] = useState(false);

  const { session, lines, truncated, paused, applySession, setPaused, clear } =
    useSerialSession();

  const lastHistoryRef = useRef<SendHistoryEntry | null>(null);

  const patchPrefs = useCallback((patch: Parameters<typeof savePrefs>[0]) => {
    setPrefs(savePrefs(patch));
  }, []);

  const refreshPorts = useCallback(async () => {
    try {
      const list = await serialDebugService.listPorts();
      setPorts(list);
      setConfig((current) => {
        const found =
          current.portName !== null &&
          list.some((item) => item.portName === current.portName);
        if (found) return current;
        const fallback = list[0]?.portName ?? null;
        if (fallback !== current.portName) patchPrefs({ portName: fallback });
        return { ...current, portName: fallback };
      });
    } catch (e) {
      logCommandError(e);
      setErrorCode(serialDebugErrorCode(e) ?? "io");
    }
  }, [patchPrefs]);

  useEffect(() => {
    void refreshPorts();
  }, [refreshPorts]);

  const handleConfigChange = useCallback(
    (patch: Partial<SerialConfigDraft>) => {
      setConfig((current) => ({ ...current, ...patch }));
      if (patch.portName !== undefined)
        patchPrefs({ portName: patch.portName });
      if (patch.baudRate !== undefined)
        patchPrefs({ baudRate: patch.baudRate });
    },
    [patchPrefs],
  );

  const runSessionCommand = useCallback(
    async (command: () => Promise<SerialSessionDto>): Promise<boolean> => {
      setBusy(true);
      try {
        applySession(await command());
        setErrorCode(null);
        return true;
      } catch (e) {
        logCommandError(e);
        setErrorCode(serialDebugErrorCode(e) ?? "io");
        return false;
      } finally {
        setBusy(false);
      }
    },
    [applySession],
  );

  const handleOpen = useCallback(() => {
    void (async () => {
      const ok = await runSessionCommand(() =>
        serialDebugService.open({
          portName: config.portName ?? "",
          baudRate: config.baudRate,
          dataBits: config.dataBits,
          parity: config.parity,
          stopBits: config.stopBits,
          flowControl: config.flowControl,
        }),
      );
      if (!ok) return;
      setDtr(true);
      await runSessionCommand(() =>
        serialDebugService.setSignals({ dtr: true }),
      );
    })();
  }, [config, runSessionCommand]);

  const handleClose = useCallback(() => {
    void runSessionCommand(() => serialDebugService.close());
  }, [runSessionCommand]);

  const handleSignalChange = useCallback(
    (patch: { dtr?: boolean; rts?: boolean }) => {
      if (patch.dtr !== undefined) setDtr(patch.dtr);
      if (patch.rts !== undefined) setRts(patch.rts);
      void runSessionCommand(() => serialDebugService.setSignals(patch));
    },
    [runSessionCommand],
  );

  const handleSend = useCallback(
    async (payload: SendPayload): Promise<boolean> => {
      let dataBase64: string;
      if (payload.mode === "hex") {
        const parsed = parseHex(payload.text);
        if (!parsed.ok) return false;
        dataBase64 = bytesToBase64(parsed.bytes);
      } else {
        dataBase64 = textToBase64(
          appendLineEnding(payload.text, payload.lineEnding),
        );
      }
      const ok = await runSessionCommand(() =>
        serialDebugService.write(dataBase64),
      );
      if (!ok) return false;
      const last = lastHistoryRef.current;
      if (
        last === null ||
        last.mode !== payload.mode ||
        last.text !== payload.text ||
        last.lineEnding !== payload.lineEnding
      ) {
        lastHistoryRef.current = payload;
        setPrefs(pushSendHistory(payload));
      }
      return true;
    },
    [runSessionCommand],
  );

  const handleEncodingChange = useCallback(
    (encoding: SerialEncoding) => patchPrefs({ encoding }),
    [patchPrefs],
  );

  const handleTimestampChange = useCallback(
    (timestamp: boolean) => patchPrefs({ timestamp }),
    [patchPrefs],
  );

  const handleLineEndingChange = useCallback(
    (lineEnding: LineEnding) => patchPrefs({ lineEnding }),
    [patchPrefs],
  );

  const isOpen = session?.open === true;

  useEffect(() => {
    if (isOpen) return;
    setDtr(false);
    setRts(false);
  }, [isOpen]);

  const activeError = session?.error?.code ?? errorCode;

  return (
    <Space orientation="vertical" size={12} style={{ width: "100%" }}>
      {activeError !== null && (
        <Alert
          type="error"
          showIcon
          title={serialDebugErrorText(activeError)}
          closable={{ onClose: () => setErrorCode(null) }}
        />
      )}

      <PortToolbar
        ports={ports}
        config={config}
        open={isOpen}
        busy={busy}
        dtr={dtr}
        rts={rts}
        onRefresh={() => void refreshPorts()}
        onConfigChange={handleConfigChange}
        onOpen={handleOpen}
        onClose={handleClose}
        onSignalChange={handleSignalChange}
      />

      <ReceivePane
        lines={lines}
        view={view}
        encoding={prefs.encoding}
        timestamp={prefs.timestamp}
        paused={paused}
        truncated={truncated}
        rxBytes={session?.rxBytes ?? 0}
        txBytes={session?.txBytes ?? 0}
        onViewChange={setView}
        onEncodingChange={handleEncodingChange}
        onTimestampChange={handleTimestampChange}
        onPauseChange={setPaused}
        onClear={clear}
      />

      <SendPane
        open={isOpen}
        history={prefs.history}
        lineEnding={prefs.lineEnding}
        onLineEndingChange={handleLineEndingChange}
        onSend={handleSend}
      />
    </Space>
  );
}
