import { useEffect } from "react";
import { IS_MAC } from "@/settings/core/shortcutTypes";

export function useCommandPaletteShortcut(
  binding: string | undefined,
  onToggle: () => void,
): void {
  useEffect(() => {
    const b = binding ?? "Meta+K";

    const requireMeta =
      /\bmeta\b/i.test(b) || /\bsuper\b/i.test(b) || /\bcmd\b/i.test(b);
    const requireCtrl = /\b(ctrl|control)\b/i.test(b) && !requireMeta;
    const requireShift = /\bshift\b/i.test(b);
    const requireAlt = /\balt\b/i.test(b);
    const keyMatch = b.match(/\b([A-Z0-9])\s*$/i);
    const key = keyMatch ? keyMatch[1].toLowerCase() : "k";

    const handler = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement && e.target.isContentEditable)
        return;
      const metaOk = requireMeta
        ? IS_MAC
          ? e.metaKey
          : e.ctrlKey
        : !e.metaKey && (requireCtrl || !e.ctrlKey);
      const ctrlOk = requireCtrl ? e.ctrlKey : true;
      const shiftOk = requireShift ? e.shiftKey : !e.shiftKey;
      const altOk = requireAlt ? e.altKey : !e.altKey;

      if (metaOk && ctrlOk && shiftOk && altOk && e.key.toLowerCase() === key) {
        e.preventDefault();
        onToggle();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [binding, onToggle]);
}
