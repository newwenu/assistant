import { useRef, useCallback } from "react";
import { Form } from "antd";
import { useSettingsStore } from "@/stores/settingsStore";
import { SettingsGroup } from "./SettingsGroup";
import { ORB_BALL_SETTINGS } from "./definitions";
import { dispatchConfPatch } from "./settingsDispatcher";

const DEBOUNCE_MS = 400;

interface OrbBallSectionProps {
  onSaved?: () => void;
}

export const OrbBallSection: React.FC<OrbBallSectionProps> = ({ onSaved }) => {
  const orbConf = useSettingsStore((s) => s.orbConf);
  const timerRef = useRef<number | null>(null);
  const pendingRef = useRef<Record<string, unknown>>({});

  const flushPatch = useCallback(async () => {
    const patch = { ...pendingRef.current };
    if (Object.keys(patch).length === 0) return;
    pendingRef.current = {};
    await dispatchConfPatch("orb", patch);
    onSaved?.();
  }, [onSaved]);

  const handlePatch = useCallback(
    (patch: Record<string, unknown>) => {
      if (!orbConf) return;

      const isSlider = Object.keys(patch).some(
        (k) => k.endsWith("_percent") || k === "hover_delay_ms",
      );

      useSettingsStore.setState({
        orbConf: { ...orbConf, ...patch } as typeof orbConf,
      });

      if (!isSlider) {
        if (timerRef.current !== null) {
          clearTimeout(timerRef.current);
          timerRef.current = null;
        }
        pendingRef.current = {};
        dispatchConfPatch("orb", patch).then(() => onSaved?.());
        return;
      }

      Object.assign(pendingRef.current, patch);
      if (timerRef.current !== null) clearTimeout(timerRef.current);
      timerRef.current = window.setTimeout(() => {
        timerRef.current = null;
        flushPatch();
      }, DEBOUNCE_MS);
    },
    [orbConf, onSaved, flushPatch],
  );

  return (
    <Form layout="vertical">
      <SettingsGroup
        group="orb"
        definitions={ORB_BALL_SETTINGS}
        onPatch={handlePatch}
      />
    </Form>
  );
};
