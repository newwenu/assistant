import React, { useEffect, useRef } from "react";
import { Alert, Button, Input, Modal } from "antd";
import { useTranslation } from "react-i18next";
import { dialogBody, dialogTitle, type ReclaimState } from "./reclaimFlow";

interface ReclaimDialogProps {
  open: boolean;
  state: ReclaimState;
  canExecute: boolean;
  onConfirm: () => void;
  onCancel: () => void;
  onTypedKey: (value: string) => void;
}

export const ReclaimDialog: React.FC<ReclaimDialogProps> = ({
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
          {t("settings.storage.reclaim.cancel")}
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
            ? t("settings.storage.reclaim.executing")
            : t("settings.storage.reclaim.ok")}
        </Button>,
      ]}
    >
      <p className="storage-reclaim-body">{dialogBody(state)}</p>

      {plan !== null && (
        <p className="storage-reclaim-key">
          {t("settings.storage.reclaim.dialog.key_hint", { key: plan.key })}
        </p>
      )}

      {strong && plan !== null && (
        <div className="storage-reclaim-input">
          <Input
            value={state.typedKey}
            autoFocus
            disabled={executing}
            status={mismatch ? "error" : undefined}
            placeholder={t("settings.storage.reclaim.strong.placeholder", {
              key: plan.key,
            })}
            onChange={(event) => onTypedKey(event.target.value)}
            onPressEnter={() => {
              if (canExecute) onConfirm();
            }}
          />
          {mismatch && (
            <span className="storage-reclaim-mismatch">
              {t("settings.storage.reclaim.strong.mismatch")}
            </span>
          )}
        </div>
      )}

      <Alert
        type="warning"
        showIcon
        title={t("settings.storage.reclaim.dialog.irreversible")}
      />
    </Modal>
  );
};
