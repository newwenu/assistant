import React from "react";
import { CheckOutlined, CloseOutlined } from "@ant-design/icons";
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
          className={css.confirmBtn}
          onClick={onConfirm}
          title="确认"
          type="button"
        >
          <CheckOutlined style={{ fontSize: 12 }} />
        </button>
        <button
          className={css.cancelBtn}
          onClick={onCancel}
          title="取消"
          type="button"
        >
          <CloseOutlined style={{ fontSize: 12 }} />
        </button>
      </div>
    </div>
  );
};
