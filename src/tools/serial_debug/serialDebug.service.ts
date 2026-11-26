import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { featureInvoke } from "@/services/featureIpc";
import { serialDebugModule } from "@/features/serial_debug/module";
import type {
  SerialDebugErrorCodeDto,
  SerialOpenInputDto,
  SerialPortInfoDto,
  SerialRxBatchDto,
  SerialSessionDto,
  SerialSetSignalsInputDto,
} from "@/types/protocol.contract";

const invoke = featureInvoke(serialDebugModule);

export const SERIAL_DEBUG_RX_EVENT = "serial_debug:rx";
export const SERIAL_DEBUG_STATUS_EVENT = "serial_debug:status";

const SERIAL_DEBUG_ERROR_CODE_FLAGS = {
  notOpen: true,
  alreadyOpen: true,
  portNotFound: true,
  permissionDenied: true,
  busy: true,
  io: true,
  unsupported: true,
  invalidInput: true,
} as const satisfies Record<SerialDebugErrorCodeDto, true>;

export function isSerialDebugErrorDto(
  value: unknown,
): value is { code: SerialDebugErrorCodeDto; message: string } {
  if (typeof value !== "object" || value === null) return false;
  const obj = value as Record<string, unknown>;
  return (
    typeof obj.code === "string" &&
    Object.prototype.hasOwnProperty.call(
      SERIAL_DEBUG_ERROR_CODE_FLAGS,
      obj.code,
    ) &&
    typeof obj.message === "string"
  );
}

export function serialDebugErrorCode(
  value: unknown,
): SerialDebugErrorCodeDto | null {
  return isSerialDebugErrorDto(value) ? value.code : null;
}

export const serialDebugService = {
  listPorts: (): Promise<SerialPortInfoDto[]> =>
    invoke<SerialPortInfoDto[]>("serial_debug_list_ports"),

  open: (input: SerialOpenInputDto): Promise<SerialSessionDto> =>
    invoke<SerialSessionDto>("serial_debug_open", { input }),

  close: (): Promise<SerialSessionDto> =>
    invoke<SerialSessionDto>("serial_debug_close"),

  status: (): Promise<SerialSessionDto> =>
    invoke<SerialSessionDto>("serial_debug_status"),

  write: (dataBase64: string): Promise<SerialSessionDto> =>
    invoke<SerialSessionDto>("serial_debug_write", { dataBase64 }),

  setSignals: (input: SerialSetSignalsInputDto): Promise<SerialSessionDto> =>
    invoke<SerialSessionDto>("serial_debug_set_signals", { input }),
};

export function onSerialRx(
  handler: (batch: SerialRxBatchDto) => void,
): Promise<UnlistenFn> {
  return listen<SerialRxBatchDto>(SERIAL_DEBUG_RX_EVENT, ({ payload }) => {
    handler(payload);
  });
}

export function onSerialStatus(
  handler: (session: SerialSessionDto) => void,
): Promise<UnlistenFn> {
  return listen<SerialSessionDto>(SERIAL_DEBUG_STATUS_EVENT, ({ payload }) => {
    handler(payload);
  });
}
