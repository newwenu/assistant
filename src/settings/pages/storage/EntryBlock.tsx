import React, { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type {
  StorageEntryDto,
  StorageReclaimActionDto,
} from "@/types/protocol.contract";
import { entryRowView } from "./entryView";
import { EntryTable } from "./EntryTable";
import { CleanupDialog } from "./CleanupDialog";
import { useCleanup } from "./useCleanup";

interface EntryBlockProps {
  entries: StorageEntryDto[];
  onFinished: () => void;
}

export const EntryBlock: React.FC<EntryBlockProps> = ({
  entries,
  onFinished,
}) => {
  const { t } = useTranslation();
  const cleanup = useCleanup(onFinished);
  const [expandedKey, setExpandedKey] = useState<string | null>(null);

  const rows = useMemo(() => entries.map(entryRowView), [entries]);
  const cleanupBusy =
    cleanup.state.phase === "planning" || cleanup.state.phase === "executing";

  const toggleDetail = useCallback((key: string) => {
    setExpandedKey((prev) => (prev === key ? null : key));
  }, []);

  const requestCleanup = useCallback(
    (key: string, action: StorageReclaimActionDto) => {
      cleanup.requestCleanup(key, action);
    },
    [cleanup],
  );

  return (
    <section className="storage-section">
      <div className="storage-section-head">
        <div className="storage-section-head-main">
          <span className="storage-section-title">
            {t("settings.storage.entries.title")}
          </span>
          <span className="storage-section-hint">
            {t("settings.storage.entries.hint")}
          </span>
        </div>
      </div>

      <EntryTable
        rows={rows}
        entries={entries}
        expandedKey={expandedKey}
        cleanupBusy={cleanupBusy}
        onToggleDetail={toggleDetail}
        onCleanup={requestCleanup}
      />

      <CleanupDialog
        open={cleanup.dialogOpen}
        state={cleanup.state}
        canExecute={cleanup.canExecute}
        onConfirm={cleanup.confirm}
        onCancel={cleanup.cancel}
        onTypedKey={cleanup.setTypedKey}
      />
    </section>
  );
};
