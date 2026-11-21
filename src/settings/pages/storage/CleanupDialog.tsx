import React, { useEffect, useRef } from "react";
import { Alert, Button, Input, Modal } from "antd";
import { useTranslation } from "react-i18next";
import { dialogBody, dialogTitle, type CleanupState } from "./cleanupFlow";

interface CleanupDialogProps {
  open: boolean;
  state: CleanupState;
  canExecute: boolean;
  onConfirm: () => void;
  onCancel: () => void;
  onTypedKey: (value: string) => void;
}

export const CleanupDialog: React.FC<CleanupDialogProps> = ({
  open,
  state,
  canExecute,
  onConfirm,
  onCancel,
  onTypedKey,
}) => {
  const { t } = useTranslation();
  const plan = state.plan;
  const executing = state.phase === "executing";
  const strong = plan !== null && plan.confirmation === "strongConfirm";

  const triggerRef = useRef<HTMLElement | null>(null);
  const wasOpenRef = useRef(false);

  useEffect(() => {
    if (open && !wasOpenRef.current) {
      const active = document.activeElement;
      triggerRef.current = active instanceof HTMLElement ? active : null;
    }
    if (!open && wasOpenRef.current) {
      triggerRef.current?.focus();
      triggerRef.current = null;
    }
    wasOpenRef.current = open;
  }, [open]);

  const mismatch =
    strong &&
    plan !== null &&
    state.typedKey !== "" &&
    state.typedKey !== plan.key;

  return (
    <Modal
      open={open}
      title={dialogTitle(state)}
      onCancel={executing ? undefined : onCancel}
      closable={!executing}
      mask={{ closable: false }}
      keyboard={!executing}
      footer={[
        <Button key="cancel" disabled={executing} onClick={onCancel}>
          {t("settings.storage.cleanup.cancel")}
        </Button>,
        <Button
          key="ok"
          type="primary"
          danger
          loading={executing}
          disabled={!canExecute}
          onClick={onConfirm}
        >
          {executing
            ? t("settings.storage.cleanup.executing")
            : t("settings.storage.cleanup.ok")}
        </Button>,
      ]}
    >
      <p className="storage-cleanup-body">{dialogBody(state)}</p>

      {plan !== null && (
        <p className="storage-cleanup-key">
          {t("settings.storage.cleanup.dialog.key_hint", { key: plan.key })}
        </p>
      )}

      {strong && plan !== null && (
        <div className="storage-cleanup-input">
          <Input
            value={state.typedKey}
            autoFocus
            disabled={executing}
            status={mismatch ? "error" : undefined}
            placeholder={t("settings.storage.cleanup.strong.placeholder", {
              key: plan.key,
            })}
            onChange={(event) => onTypedKey(event.target.value)}
            onPressEnter={() => {
              if (canExecute) onConfirm();
            }}
          />
          {mismatch && (
            <span className="storage-cleanup-mismatch">
              {t("settings.storage.cleanup.strong.mismatch")}
            </span>
          )}
        </div>
      )}

      <Alert
        type="warning"
        showIcon
        title={t("settings.storage.cleanup.dialog.irreversible")}
      />
    </Modal>
  );
};
