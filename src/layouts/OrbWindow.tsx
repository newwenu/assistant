import React from "react";
import { useOrbStore } from "@/stores/orbStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { OrbBall } from "@/orb/OrbBall";
import { OrbPanel } from "@/orb/OrbPanel";

export const OrbWindow: React.FC = () => {
  const isExpanded = useOrbStore((s) => s.isExpanded);
  const expandDirection = useOrbStore((s) => s.expandDirection);
  const expandVertical = useOrbStore((s) => s.expandVertical);
  const loadConf = useSettingsStore((s) => s.loadConf);

  React.useEffect(() => {
    void loadConf("orb");
    void loadConf("mascot");
  }, [loadConf]);

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
