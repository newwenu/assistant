import { Input, Button, Tooltip, Typography } from "antd";
import {
  SearchOutlined,
  PlusOutlined,
  SafetyOutlined,
  SyncOutlined,
  DeleteOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import styles from "./vault.module.css";

interface VaultToolbarProps {
  searchQuery: string;
  syncing: boolean;
  hibpAuditing: boolean;
  hibpProgress: { done: number; total: number } | null;
  conflictState: string | null;
  pendingRemovalCount: number;
  onSearchChange: (query: string) => void;
  onAdd: () => void;
  onAudit: () => void;
  onCancelAudit: () => void;
  onSync: () => void;
  onOpenTrash: () => void;
}

export default function VaultToolbar({
  searchQuery,
  syncing,
  hibpAuditing,
  hibpProgress,
  conflictState,
  pendingRemovalCount,
  onSearchChange,
  onAdd,
  onAudit,
  onCancelAudit,
  onSync,
  onOpenTrash,
}: VaultToolbarProps) {
  const { t } = useTranslation();

  const syncTooltip =
    pendingRemovalCount > 0
      ? t("vault.sync.pending_removals_banner", { count: pendingRemovalCount })
      : conflictState !== null
        ? t("vault.conflict.detected")
        : t("vault.sync.now");

  return (
    <div className={styles.listHeader}>
      <Input
        prefix={<SearchOutlined />}
        placeholder={t("vault.entry.search")}
        value={searchQuery}
        onChange={(event) => onSearchChange(event.target.value)}
        allowClear
        className={styles.searchInput}
      />
      <Button type="primary" icon={<PlusOutlined />} onClick={onAdd}>
        {t("vault.entry.add_entry")}
      </Button>
      <Tooltip title={t("vault.hibp.audit_all")}>
        <Button
          type="text"
          icon={<SafetyOutlined />}
          aria-label={t("vault.hibp.audit_all")}
          onClick={onAudit}
          loading={hibpAuditing && (hibpProgress?.total ?? 0) === 0}
        />
      </Tooltip>
      {hibpAuditing && (hibpProgress?.total ?? 0) > 0 && (
        <Typography.Text type="secondary" className={styles.hibpProgress}>
          {t("vault.hibp.progress", {
            done: hibpProgress?.done ?? 0,
            total: hibpProgress?.total ?? 0,
          })}
        </Typography.Text>
      )}
      {hibpAuditing && (
        <Button type="link" size="small" onClick={onCancelAudit}>
          {t("vault.hibp.cancel")}
        </Button>
      )}
      <Tooltip title={syncTooltip}>
        <Button
          type="text"
          danger={conflictState !== null || pendingRemovalCount > 0}
          aria-label={t("vault.sync.now")}
          icon={<SyncOutlined spin={syncing} />}
          onClick={onSync}
          disabled={syncing}
        />
      </Tooltip>
      <Tooltip title={t("vault.entry.trash")}>
        <Button
          icon={<DeleteOutlined />}
          aria-label={t("vault.entry.trash")}
          onClick={onOpenTrash}
        />
      </Tooltip>
    </div>
  );
}
