import { useState } from "react";
import { Button, Empty, Select, Space, Switch, Typography } from "antd";
import { translateKey } from "@/i18n";
import type {
  SerialDataBitsDto,
  SerialFlowControlDto,
  SerialParityDto,
  SerialPortInfoDto,
  SerialStopBitsDto,
} from "@/types/protocol.contract";

export interface SerialConfigDraft {
  portName: string | null;
  baudRate: number;
  dataBits: SerialDataBitsDto;
  parity: SerialParityDto;
  stopBits: SerialStopBitsDto;
  flowControl: SerialFlowControlDto;
}

const BAUD_RATES = [9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600];

const DATA_BITS: SerialDataBitsDto[] = ["five", "six", "seven", "eight"];

const DATA_BITS_LABELS: Record<SerialDataBitsDto, string> = {
  five: "5",
  six: "6",
  seven: "7",
  eight: "8",
};

const PARITIES: SerialParityDto[] = ["none", "odd", "even", "mark", "space"];

const PARITY_LABELS: Record<SerialParityDto, string> = {
  none: "N",
  odd: "O",
  even: "E",
  mark: "M",
  space: "S",
};

const STOP_BITS: SerialStopBitsDto[] = ["one", "two"];

const STOP_BITS_LABELS: Record<SerialStopBitsDto, string> = {
  one: "1",
  two: "2",
};

const FLOW_CONTROLS: SerialFlowControlDto[] = ["none", "software", "hardware"];

const FLOW_CONTROL_LABELS: Record<SerialFlowControlDto, string> = {
  none: "None",
  software: "XON/XOFF",
  hardware: "RTS/CTS",
};

interface PortToolbarProps {
  ports: SerialPortInfoDto[];
  config: SerialConfigDraft;
  open: boolean;
  busy: boolean;
  dtr: boolean;
  rts: boolean;
  onRefresh: () => void;
  onConfigChange: (patch: Partial<SerialConfigDraft>) => void;
  onOpen: () => void;
  onClose: () => void;
  onSignalChange: (patch: { dtr?: boolean; rts?: boolean }) => void;
}

export function PortToolbar({
  ports,
  config,
  open,
  busy,
  dtr,
  rts,
  onRefresh,
  onConfigChange,
  onOpen,
  onClose,
  onSignalChange,
}: PortToolbarProps) {
  const [advanced, setAdvanced] = useState(false);

  return (
    <Space orientation="vertical" size={8} style={{ width: "100%" }}>
      <Space wrap size={8}>
        <Typography.Text type="secondary">
          {translateKey("tools.serial_debug.port")}
        </Typography.Text>
        <Select
          style={{ minWidth: 220 }}
          value={config.portName}
          placeholder={translateKey("tools.serial_debug.noPorts")}
          onChange={(value: string) => onConfigChange({ portName: value })}
          options={ports.map((item) => ({
            value: item.portName,
            label:
              item.product === null
                ? item.portName
                : `${item.portName} — ${item.product}`,
          }))}
          notFoundContent={
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={translateKey("tools.serial_debug.noPorts")}
            />
          }
          disabled={open || busy}
        />
        <Button onClick={onRefresh} disabled={open || busy}>
          {translateKey("tools.serial_debug.refresh")}
        </Button>
        <Typography.Text type="secondary">
          {translateKey("tools.serial_debug.baudRate")}
        </Typography.Text>
        <Select
          style={{ width: 120 }}
          value={config.baudRate}
          onChange={(value: number) => onConfigChange({ baudRate: value })}
          options={BAUD_RATES.map((value) => ({
            value,
            label: String(value),
          }))}
          disabled={open || busy}
        />
        <Space size={4}>
          <Typography.Text type="secondary">
            {translateKey("tools.serial_debug.advanced")}
          </Typography.Text>
          <Switch
            size="small"
            checked={advanced}
            onChange={setAdvanced}
            disabled={open}
          />
        </Space>
        {open ? (
          <Button danger onClick={onClose} disabled={busy}>
            {translateKey("tools.serial_debug.close")}
          </Button>
        ) : (
          <Button
            type="primary"
            onClick={onOpen}
            disabled={busy || config.portName === null}
          >
            {translateKey("tools.serial_debug.open")}
          </Button>
        )}
        <Typography.Text type={open ? "success" : "secondary"}>
          {translateKey(
            open
              ? "tools.serial_debug.statusOpen"
              : "tools.serial_debug.statusClosed",
          )}
        </Typography.Text>
      </Space>

      {advanced && (
        <Space wrap size={8}>
          <Typography.Text type="secondary">
            {translateKey("tools.serial_debug.dataBits")}
          </Typography.Text>
          <Select
            style={{ width: 80 }}
            value={config.dataBits}
            onChange={(value: SerialDataBitsDto) =>
              onConfigChange({ dataBits: value })
            }
            options={DATA_BITS.map((value) => ({
              value,
              label: DATA_BITS_LABELS[value],
            }))}
            disabled={open || busy}
          />
          <Typography.Text type="secondary">
            {translateKey("tools.serial_debug.parity")}
          </Typography.Text>
          <Select
            style={{ width: 80 }}
            value={config.parity}
            onChange={(value: SerialParityDto) =>
              onConfigChange({ parity: value })
            }
            options={PARITIES.map((value) => ({
              value,
              label: PARITY_LABELS[value],
            }))}
            disabled={open || busy}
          />
          <Typography.Text type="secondary">
            {translateKey("tools.serial_debug.stopBits")}
          </Typography.Text>
          <Select
            style={{ width: 80 }}
            value={config.stopBits}
            onChange={(value: SerialStopBitsDto) =>
              onConfigChange({ stopBits: value })
            }
            options={STOP_BITS.map((value) => ({
              value,
              label: STOP_BITS_LABELS[value],
            }))}
            disabled={open || busy}
          />
          <Typography.Text type="secondary">
            {translateKey("tools.serial_debug.flowControl")}
          </Typography.Text>
          <Select
            style={{ width: 120 }}
            value={config.flowControl}
            onChange={(value: SerialFlowControlDto) =>
              onConfigChange({ flowControl: value })
            }
            options={FLOW_CONTROLS.map((value) => ({
              value,
              label: FLOW_CONTROL_LABELS[value],
            }))}
            disabled={open || busy}
          />
          <Typography.Text type="secondary">
            {translateKey("tools.serial_debug.dtr")}
          </Typography.Text>
          <Switch
            size="small"
            checked={dtr}
            onChange={(value) => onSignalChange({ dtr: value })}
            disabled={!open || busy}
          />
          <Typography.Text type="secondary">
            {translateKey("tools.serial_debug.rts")}
          </Typography.Text>
          <Switch
            size="small"
            checked={rts}
            onChange={(value) => onSignalChange({ rts: value })}
            disabled={!open || busy}
          />
        </Space>
      )}
    </Space>
  );
}
