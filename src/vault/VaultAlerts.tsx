import { useState } from "react";
import { Alert, Button } from "antd";
import { useTranslation } from "react-i18next";
import type { PendingRemovals } from "./core/vault.types";
import styles from "./vault.module.css";

interface VaultAlertsProps {
  pendingRemovals: PendingRemovals | null;
  repoState: string | null;
  conflictCount: number;
  onOpenRemovals: () => void;
  onOpenConflicts: () => void;
}

export default function VaultAlerts({
  pendingRemovals,
  repoState,
  conflictCount,
  onOpenRemovals,
  onOpenConflicts,
}: VaultAlertsProps) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);

  const alerts: { key: string; node: React.ReactNode }[] = [];

  if (repoState !== null) {
    alerts.push({
      key: "conflict",
      node: (
        <Alert
          key="conflict"
          type="warning"
          showIcon
          title={t("vault.conflict.detected")}
          description={t("vault.conflict.detected_hint", {
            state: repoState,
            count: conflictCount,
          })}
          action={
            <Button size="small" onClick={onOpenConflicts}>
              {t("vault.conflict.open_resolver")}
            </Button>
          }
        />
      ),
    });
  }

  if (pendingRemovals !== null) {
    alerts.push({
      key: "removals",
      node: (
        <Alert
          key="removals"
          type="warning"
          showIcon
          title={t("vault.sync.pending_removals_banner", {
            count: pendingRemovals.entries.length,
          })}
          description={t("vault.sync.pending_removals_hint")}
          action={
            <Button size="small" onClick={onOpenRemovals}>
              {t("vault.sync.pending_removals_open")}
            </Button>
          }
        />
      ),
    });
  }

  if (alerts.length === 0) return null;

  const visible = expanded ? alerts : alerts.slice(0, 1);
  const hiddenCount = alerts.length - visible.length;

  return (
    <div className={styles.alertStack}>
      {visible.map((alert) => alert.node)}
      {alerts.length > 1 && (
        <Button type="link" size="small" onClick={() => setExpanded((v) => !v)}>
          {expanded
            ? t("vault.alert.collapse")
            : t("vault.alert.more", { count: hiddenCount })}
        </Button>
      )}
    </div>
  );
}
