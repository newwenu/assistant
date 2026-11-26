import React, { useEffect, useState } from "react";
import { Alert, Empty, Spin } from "antd";
import { useTranslation } from "react-i18next";
import { translateKey, type I18nPath } from "@/i18n";
import { resourceService } from "@/services/desktop/resourceService";
import { createLogger } from "@/services/system/logger";
import type {
  ProcessSnapshotDto,
  ResourceAvailabilityDto,
  ResourceTabCapabilityDto,
} from "@/types/protocol.contract";

const log = createLogger("resource");

interface ProcessTreeTabState {
  capability: ResourceTabCapabilityDto | null;
  snapshot: ProcessSnapshotDto | null;
  capabilityFailed: boolean;
  failure: string | null;
  loading: boolean;
}

const INITIAL_STATE: ProcessTreeTabState = {
  capability: null,
  snapshot: null,
  capabilityFailed: false,
  failure: null,
  loading: true,
};

function errorText(raw: unknown): string {
  return raw instanceof Error ? raw.message : String(raw);
}

const AVAILABILITY_KEYS = {
  available: "settings.resource.status.coverageSubtreeOnly",
  unavailable: "settings.resource.status.unreadable",
  notImplemented: "settings.resource.status.notImplemented",
  unreadable: "settings.resource.status.unreadable",
} as const satisfies Record<ResourceAvailabilityDto, I18nPath>;

export const ProcessTreeTab: React.FC = () => {
  const { t } = useTranslation();
  const [state, setState] = useState<ProcessTreeTabState>(INITIAL_STATE);

  useEffect(() => {
    let alive = true;

    void resourceService
      .getResourceCapabilities()
      .then((capabilities) => {
        if (!alive) return;
        const capability =
          capabilities.tabs.find((item) => item.tab === "processTree") ?? null;
        setState((prev) => ({ ...prev, capability, loading: false }));
      })
      .catch((raw: unknown) => {
        log.error("failed to load resource capabilities", raw);
        if (!alive) return;
        setState((prev) => ({
          ...prev,
          capabilityFailed: true,
          failure: errorText(raw),
          loading: false,
        }));
      });

    void resourceService
      .getProcessSnapshot()
      .then((snapshot) => {
        if (!alive) return;
        setState((prev) => ({ ...prev, snapshot, loading: false }));
      })
      .catch((raw: unknown) => {
        log.error("failed to load process snapshot", raw);
        if (!alive) return;
        setState((prev) => ({ ...prev, failure: errorText(raw) }));
      });

    return () => {
      alive = false;
    };
  }, []);

  if (state.loading) {
    return (
      <div className="storage-loading">
        <Spin />
      </div>
    );
  }

  const availability: ResourceAvailabilityDto | null =
    state.snapshot?.availability ?? state.capability?.availability ?? null;
  const reason = state.snapshot?.reason ?? state.capability?.reason ?? null;

  return (
    <div className="resource-process-tab">
      {state.capabilityFailed && (
        <Alert
          type="warning"
          showIcon
          title={t("settings.resource.status.capabilitiesLoadFailed")}
        />
      )}
      {availability === null ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={
            <div className="storage-load-error">
              <span>{t("settings.resource.status.unreadable")}</span>
              {state.failure !== null && (
                <span className="storage-load-error-detail">
                  {state.failure}
                </span>
              )}
            </div>
          }
        />
      ) : (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={
            <div className="storage-load-error">
              <span>{translateKey(AVAILABILITY_KEYS[availability])}</span>
              {reason !== null && (
                <span className="storage-load-error-detail">{reason}</span>
              )}
            </div>
          }
        />
      )}
    </div>
  );
};
