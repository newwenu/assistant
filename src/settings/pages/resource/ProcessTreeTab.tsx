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
import {
  entryRowView,
  memoryColumnText,
  summaryView,
  type ProcessGroupView,
} from "./processTreeView";

const log = createLogger("resource");

const SAMPLE_INTERVAL_MS = 1000;

interface ProcessTreeTabState {
  capability: ResourceTabCapabilityDto | null;
  capabilityFailed: boolean;
  snapshot: ProcessSnapshotDto | null;
  lastGood: ProcessSnapshotDto | null;
  failure: string | null;
  loading: boolean;
}

const INITIAL_STATE: ProcessTreeTabState = {
  capability: null,
  capabilityFailed: false,
  snapshot: null,
  lastGood: null,
  failure: null,
  loading: true,
};

const UNAVAILABLE_KEYS = {
  available: "settings.resource.status.coverageSubtreeOnly",
  unavailable: "settings.resource.status.memoryUnavailable",
  notImplemented: "settings.resource.status.notImplemented",
  unreadable: "settings.resource.status.unreadable",
} as const satisfies Record<ResourceAvailabilityDto, I18nPath>;

function errorText(raw: unknown): string {
  return raw instanceof Error ? raw.message : String(raw);
}

const GroupCard: React.FC<{ view: ProcessGroupView }> = ({ view }) => {
  const { t } = useTranslation();
  return (
    <div className="resource-process-group">
      <div className="resource-process-group-head">
        <span className="resource-process-group-label">{view.label}</span>
        <span className="resource-process-group-count">{view.pidText}</span>
      </div>
      <div className="resource-process-group-metrics">
        <span
          className={
            view.cpuPending ? "resource-process-metric-pending" : undefined
          }
        >
          {t("settings.resource.column.cpu")} {view.cpuText}
        </span>
        <span
          className={
            view.memoryNote !== null
              ? "resource-process-metric-pending"
              : undefined
          }
        >
          {t("settings.resource.column.memory")} {view.memoryText}
        </span>
      </div>
      {view.memoryNote !== null && (
        <span className="resource-process-group-note">{view.memoryNote}</span>
      )}
    </div>
  );
};

const SummaryBlock: React.FC<{ snapshot: ProcessSnapshotDto }> = ({
  snapshot,
}) => {
  const summary = summaryView(snapshot);
  return (
    <div className="resource-process-summary">
      <div className="resource-process-summary-meta">
        <span>{summary.pidText}</span>
        <span>{summary.hostText}</span>
        {summary.sampledAtText !== null && <span>{summary.sampledAtText}</span>}
      </div>
      <div className="resource-process-groups">
        <GroupCard view={summary.self} />
        <GroupCard view={summary.subtree} />
      </div>
      <div className="resource-process-notes">
        <span>{summary.relationText}</span>
        <span>{summary.cpuNote}</span>
        {summary.memoryCaliberNote !== null && (
          <span>{summary.memoryCaliberNote}</span>
        )}
      </div>
    </div>
  );
};

const ProcessTable: React.FC<{ snapshot: ProcessSnapshotDto }> = ({
  snapshot,
}) => {
  const { t } = useTranslation();
  return (
    <div className="resource-process-table">
      <div className="resource-process-header">
        <span className="resource-process-col">
          {t("settings.resource.column.process")}
        </span>
        <span className="resource-process-col">
          {t("settings.resource.column.role")}
        </span>
        <span className="resource-process-col resource-process-col-num">
          {t("settings.resource.column.cpu")}
        </span>
        <span className="resource-process-col resource-process-col-num">
          {memoryColumnText(snapshot.memoryBasis)}
        </span>
      </div>
      {snapshot.entries.map((entry) => {
        const row = entryRowView(entry);
        return (
          <div className="resource-process-row" key={row.key}>
            <span className="resource-process-cell resource-process-cell-process">
              <span className="resource-process-name">{row.name}</span>
              <span className="resource-process-pid">{row.pidText}</span>
            </span>
            <span className="resource-process-cell">{row.roleText}</span>
            <span
              className={
                row.cpuPending
                  ? "resource-process-cell resource-process-col-num resource-process-cell-pending"
                  : "resource-process-cell resource-process-col-num"
              }
            >
              {row.cpuText}
            </span>
            <span className="resource-process-cell resource-process-col-num">
              {row.memoryText}
            </span>
          </div>
        );
      })}
    </div>
  );
};

export const ProcessTreeTab: React.FC<{ active: boolean }> = ({ active }) => {
  const { t } = useTranslation();
  const [state, setState] = useState<ProcessTreeTabState>(INITIAL_STATE);

  useEffect(() => {
    if (!active) return;
    let alive = true;
    let inFlight = false;

    const sample = () => {
      if (inFlight) return;
      inFlight = true;
      void resourceService
        .getProcessSnapshot()
        .then((snapshot) => {
          if (!alive) return;
          setState((prev) => ({
            ...prev,
            snapshot,
            lastGood:
              snapshot.availability === "available" ? snapshot : prev.lastGood,
            failure: null,
            loading: false,
          }));
        })
        .catch((raw: unknown) => {
          log.error("failed to load process snapshot", raw);
          if (!alive) return;
          setState((prev) => ({
            ...prev,
            failure: errorText(raw),
            loading: false,
          }));
        })
        .finally(() => {
          inFlight = false;
        });
    };

    void resourceService
      .getResourceCapabilities()
      .then((capabilities) => {
        if (!alive) return;
        const capability =
          capabilities.tabs.find((item) => item.tab === "processTree") ?? null;
        setState((prev) => ({
          ...prev,
          capability,
          capabilityFailed: false,
        }));
      })
      .catch((raw: unknown) => {
        log.error("failed to load resource capabilities", raw);
        if (!alive) return;
        setState((prev) => ({ ...prev, capabilityFailed: true }));
      });

    sample();
    const timer = setInterval(sample, SAMPLE_INTERVAL_MS);

    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [active]);

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
  const shown = state.lastGood;

  return (
    <div className="resource-process-tab">
      {state.capabilityFailed && (
        <Alert
          type="warning"
          showIcon
          title={t("settings.resource.status.capabilitiesLoadFailed")}
        />
      )}
      {(availability === "unreadable" || state.failure !== null) && (
        <Alert
          type="error"
          showIcon
          title={t("settings.resource.status.unreadable")}
          description={state.failure ?? reason ?? undefined}
        />
      )}
      {availability === "notImplemented" ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={
            <div className="storage-load-error">
              <span>{t("settings.resource.status.notImplemented")}</span>
              {reason !== null && (
                <span className="storage-load-error-detail">{reason}</span>
              )}
            </div>
          }
        />
      ) : shown === null ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={
            <div className="storage-load-error">
              <span>
                {translateKey(UNAVAILABLE_KEYS[availability ?? "unreadable"])}
              </span>
              {(state.failure ?? reason) !== null && (
                <span className="storage-load-error-detail">
                  {state.failure ?? reason}
                </span>
              )}
            </div>
          }
        />
      ) : (
        <>
          {shown.truncated && (
            <Alert
              type="warning"
              showIcon
              title={t("settings.resource.status.truncated")}
            />
          )}
          <SummaryBlock snapshot={shown} />
          <div className="resource-process-hints">
            <span>{t("settings.resource.status.coverageSubtreeOnly")}</span>
            {shown.webviewDetached && (
              <span>
                {t("settings.resource.status.coveragePlatformManaged")}
              </span>
            )}
          </div>
          <ProcessTable snapshot={shown} />
        </>
      )}
    </div>
  );
};
