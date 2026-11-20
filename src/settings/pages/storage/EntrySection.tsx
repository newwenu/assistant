import React, { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type {
  StorageEntryDto,
  StorageReclaimActionDto,
} from "@/types/protocol.contract";
import { entryRowView } from "./entryView";
import { EntryTable } from "./EntryTable";
import { ReclaimDialog } from "./ReclaimDialog";
import { useReclaim } from "./useReclaim";

interface EntrySectionProps {
  entries: StorageEntryDto[];
  onReclaimed: () => void;
}

export const EntrySection: React.FC<EntrySectionProps> = ({
  entries,
  onReclaimed,
}) => {
  const { t } = useTranslation();
  const reclaim = useReclaim(onReclaimed);
  const [expandedKey, setExpandedKey] = useState<string | null>(null);

  const rows = useMemo(() => entries.map(entryRowView), [entries]);
  const reclaimBusy =
    reclaim.state.phase === "planning" || reclaim.state.phase === "executing";

  const toggleDetail = useCallback((key: string) => {
    setExpandedKey((prev) => (prev === key ? null : key));
  }, []);

  const requestReclaim = useCallback(
    (key: string, action: StorageReclaimActionDto) => {
      reclaim.requestReclaim(key, action);
    },
    [reclaim],
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
        reclaimBusy={reclaimBusy}
        onToggleDetail={toggleDetail}
        onReclaim={requestReclaim}
      />

      <ReclaimDialog
        open={reclaim.dialogOpen}
        state={reclaim.state}
        canExecute={reclaim.canExecute}
        onConfirm={reclaim.confirm}
        onCancel={reclaim.cancel}
        onTypedKey={reclaim.setTypedKey}
      />
    </section>
  );
};
