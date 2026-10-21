import React from "react";
import { CheckOutlined, CloseOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import css from "./EditBlock.module.css";

interface EditBlockProps {
  value: string;
  onChange: (value: string) => void;
  onConfirm: () => void;
  onCancel: () => void;
}

export const EditBlock: React.FC<EditBlockProps> = ({
  value,
  onChange,
  onConfirm,
  onCancel,
}) => {
  const { t } = useTranslation();

  return (
    <div className={css.root}>
      <textarea
        className={css.textarea}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        autoFocus
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            onConfirm();
          }
          if (e.key === "Escape") {
            onCancel();
          }
        }}
      />
      <div className={css.actions}>
        <button
          className={css.cancelBtn}
          onClick={onCancel}
          title={t("common.actions.cancel")}
          type="button"
        >
          <CloseOutlined style={{ fontSize: 14 }} />
        </button>
        <button
          className={css.confirmBtn}
          onClick={onConfirm}
          title={t("common.actions.confirm")}
          type="button"
        >
          <CheckOutlined style={{ fontSize: 14 }} />
        </button>
      </div>
    </div>
  );
};
