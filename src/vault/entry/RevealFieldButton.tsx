import { Button } from "antd";
import { EyeOutlined, EyeInvisibleOutlined } from "@ant-design/icons";
import { useCallback, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

interface RevealFieldButtonProps {
  revealed: boolean;
  onReveal: () => Promise<void>;
  onHide: () => void;
}

export default function RevealFieldButton({
  revealed,
  onReveal,
  onHide,
}: RevealFieldButtonProps) {
  const { t } = useTranslation();
  const holdingRef = useRef(false);

  const stop = useCallback(() => {
    if (!holdingRef.current) return;
    holdingRef.current = false;
    onHide();
  }, [onHide]);

  const start = useCallback(() => {
    if (holdingRef.current) return;
    holdingRef.current = true;
    void onReveal().then(() => {
      if (!holdingRef.current) onHide();
    });
  }, [onReveal, onHide]);

  useEffect(() => {
    window.addEventListener("pointerup", stop);
    window.addEventListener("pointercancel", stop);
    window.addEventListener("blur", stop);
    return () => {
      window.removeEventListener("pointerup", stop);
      window.removeEventListener("pointercancel", stop);
      window.removeEventListener("blur", stop);
    };
  }, [stop]);

  return (
    <Button
      type="text"
      size="small"
      icon={revealed ? <EyeInvisibleOutlined /> : <EyeOutlined />}
      onPointerDown={start}
      onPointerUp={stop}
      onPointerLeave={stop}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          start();
        }
      }}
      onKeyUp={(e) => {
        if (e.key === "Enter" || e.key === " ") stop();
      }}
      onBlur={stop}
      onContextMenu={(e) => e.preventDefault()}
    >
      {revealed
        ? t("vault.entry.release_to_hide")
        : t("vault.entry.hold_to_reveal")}
    </Button>
  );
}
