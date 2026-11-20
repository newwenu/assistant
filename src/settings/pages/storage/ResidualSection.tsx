import React, { useEffect, useMemo, useRef, useState } from "react";
import { App, Button } from "antd";
import { ScanOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type {
  StorageInventoryDto,
  StorageInventorySnapshotStatusDto,
  StorageInventoryStatusDto,
} from "@/types/protocol.contract";
import { residualView } from "./residualView";
import { ResidualTable } from "./ResidualTable";

interface ResidualSectionProps {
  inventory: StorageInventoryDto;
  onScan: () => Promise<StorageInventorySnapshotStatusDto>;
}

function errorText(raw: unknown): string {
  return raw instanceof Error ? raw.message : String(raw);
}

function scanLabelKey(
  status: StorageInventoryStatusDto,
): "scan" | "scanning" | "rescan" | "retry_scan" {
  switch (status) {
    case "neverRun":
      return "scan";
    case "running":
      return "scanning";
    case "failed":
      return "retry_scan";
    default:
      return "rescan";
  }
}

export const ResidualSection: React.FC<ResidualSectionProps> = ({
  inventory,
  onScan,
}) => {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [scanning, setScanning] = useState(false);
  const [statusOverride, setStatusOverride] =
    useState<StorageInventoryStatusDto | null>(null);
  const inventoryRef = useRef(inventory);

  useEffect(() => {
    if (inventoryRef.current !== inventory) {
      inventoryRef.current = inventory;
      setStatusOverride(null);
    }
  }, [inventory]);

  const status = statusOverride ?? inventory.status;
  const view = useMemo(
    () => residualView(inventory, statusOverride ?? undefined),
    [inventory, statusOverride],
  );

  const handleScan = async (): Promise<void> => {
    if (scanning) return;
    setScanning(true);
    try {
      const result = await onScan();
      setStatusOverride(result.status);
    } catch (raw) {
      message.error(
        t("settings.storage.residual.notice.scan_failed", {
          error: errorText(raw),
        }),
      );
    } finally {
      setScanning(false);
    }
  };

  const busy = scanning || status === "running";

  return (
    <section className="storage-section">
      <div className="storage-section-head">
        <div className="storage-section-head-main">
          <span className="storage-section-title">
            {t("settings.storage.residual.title")}
            <span className="storage-readonly-badge">
              {t("settings.storage.residual.read_only")}
            </span>
          </span>
          <span className="storage-section-hint">
            {t("settings.storage.residual.hint")}
          </span>
        </div>
        <div className="storage-section-head-actions">
          <span className={`storage-residual-status is-${view.statusTone}`}>
            {view.statusText}
          </span>
          <Button
            size="small"
            icon={<ScanOutlined />}
            loading={busy}
            disabled={busy}
            onClick={() => {
              void handleScan();
            }}
          >
            {t(`settings.storage.residual.${scanLabelKey(status)}`)}
          </Button>
        </div>
      </div>

      <ResidualTable view={view} />
    </section>
  );
};
