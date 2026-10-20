import React from "react";
import { useOrbStore } from "@/stores/orbStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { OrbBall } from "@/orb/OrbBall";
import { OrbPanel } from "@/orb/OrbPanel";

export const OrbWindow: React.FC = () => {
  const isExpanded = useOrbStore((s) => s.isExpanded);
  const expandDirection = useOrbStore((s) => s.expandDirection);
  const expandVertical = useOrbStore((s) => s.expandVertical);
  const loadOrbConf = useSettingsStore((s) => s.loadOrbConf);
  const loadMascotConf = useSettingsStore((s) => s.loadMascotConf);

  React.useEffect(() => {
    void loadOrbConf();
    void loadMascotConf();
  }, [loadOrbConf, loadMascotConf]);

  const orbClassName = [
    isExpanded ? "orb-expanded" : "",
    `orb-expand-${expandDirection}`,
    `orb-expand-${expandVertical}`,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div
      data-orb-window
      className={orbClassName}
      style={{
        width: "100%",
        height: "100%",
        position: "relative",
      }}
    >
      <OrbBall />
      <OrbPanel />
    </div>
  );
};
