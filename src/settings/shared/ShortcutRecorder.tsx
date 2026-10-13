import { useState, useEffect, useRef, useCallback } from "react";
import { Tag } from "antd";
import { useTranslation } from "react-i18next";
import { formatBinding } from "../core/shortcutTypes";

interface ShortcutRecorderProps {
  currentBinding: string;
  onConfirm: (binding: string) => void;
  onClear: () => void;
}

const CODE_MAP: Record<string, string> = {
  KeyA: "A",
  KeyB: "B",
  KeyC: "C",
  KeyD: "D",
  KeyE: "E",
  KeyF: "F",
  KeyG: "G",
  KeyH: "H",
  KeyI: "I",
  KeyJ: "J",
  KeyK: "K",
  KeyL: "L",
  KeyM: "M",
  KeyN: "N",
  KeyO: "O",
  KeyP: "P",
  KeyQ: "Q",
  KeyR: "R",
  KeyS: "S",
  KeyT: "T",
  KeyU: "U",
  KeyV: "V",
  KeyW: "W",
  KeyX: "X",
  KeyY: "Y",
  KeyZ: "Z",
  Digit0: "0",
  Digit1: "1",
  Digit2: "2",
  Digit3: "3",
  Digit4: "4",
  Digit5: "5",
  Digit6: "6",
  Digit7: "7",
  Digit8: "8",
  Digit9: "9",
  Space: "Space",
  Enter: "Enter",
  Tab: "Tab",
  Backspace: "Backspace",
  Delete: "Delete",
  Escape: "Escape",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  F1: "F1",
  F2: "F2",
  F3: "F3",
  F4: "F4",
  F5: "F5",
  F6: "F6",
  F7: "F7",
  F8: "F8",
  F9: "F9",
  F10: "F10",
  F11: "F11",
  F12: "F12",
};

export const ShortcutRecorder: React.FC<ShortcutRecorderProps> = ({
  currentBinding,
  onConfirm,
  onClear,
}) => {
  const { t } = useTranslation();
  const [recording, setRecording] = useState(false);
  const [captured, setCaptured] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const capturedRef = useRef<string | null>(null);
  const onConfirmRef = useRef(onConfirm);
  const onClearRef = useRef(onClear);
  onConfirmRef.current = onConfirm;
  onClearRef.current = onClear;

  const buildBinding = useCallback((e: KeyboardEvent): string | null => {
    const parts: string[] = [];
    if (e.metaKey) parts.push("Meta");
    if (e.ctrlKey && !e.metaKey) parts.push("Control");
    if (e.shiftKey) parts.push("Shift");
    if (e.altKey) parts.push("Alt");

    const key = CODE_MAP[e.code];
    if (!key) return null;

    if (["Meta", "Control", "Shift", "Alt"].includes(key)) return null;

    parts.push(key);
    return parts.join("+");
  }, []);

  useEffect(() => {
    if (!recording) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (e.key === "Escape") {
        setRecording(false);
        setCaptured(null);
        capturedRef.current = null;
        return;
      }

      if (
        e.key === "Backspace" &&
        !e.shiftKey &&
        !e.metaKey &&
        !e.ctrlKey &&
        !e.altKey
      ) {
        setCaptured(null);
        capturedRef.current = null;
        onClearRef.current();
        setRecording(false);
        return;
      }

      const binding = buildBinding(e);
      if (binding) {
        setCaptured(binding);
        capturedRef.current = binding;
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      const currentCaptured = capturedRef.current;
      if (!currentCaptured) return;
      e.preventDefault();
      e.stopPropagation();

      const binding = buildBinding(e);
      if (binding && binding === currentCaptured) {
        onConfirmRef.current(binding);
        setRecording(false);
        setCaptured(null);
        capturedRef.current = null;
      }
    };

    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("keyup", handleKeyUp, true);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("keyup", handleKeyUp, true);
    };
  }, [recording, buildBinding]);

  if (recording) {
    return (
      <div
        ref={ref}
        style={{
          display: "inline-flex",
          alignItems: "center",
          gap: 4,
          cursor: "pointer",
        }}
      >
        {captured ? (
          <Tag color="blue">{formatBinding(captured)}</Tag>
        ) : (
          <Tag color="processing">{t("shortcut.recording")}</Tag>
        )}
      </div>
    );
  }

  return (
    <div
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 4,
        cursor: "pointer",
      }}
      onClick={() => setRecording(true)}
    >
      <Tag
        style={{ cursor: "pointer" }}
        color={currentBinding ? "default" : "warning"}
      >
        {currentBinding ? formatBinding(currentBinding) : t("shortcut.not_set")}
      </Tag>
    </div>
  );
};
