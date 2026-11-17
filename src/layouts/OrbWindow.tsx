import React from "react";
import { ConfigProvider } from "antd";
import { useOrbStore } from "@/stores/orbStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useScreenshotStore } from "@/stores/screenshotStore";
import { OrbBall } from "@/orb/OrbBall";
import { OrbPanel } from "@/orb/OrbPanel";
import { ScreenshotOverlay } from "@/screenshot/Overlay";
import { useScreenshotSession } from "@/screenshot/useScreenshotSession";

export const OrbWindow: React.FC = () => {
  const isExpanded = useOrbStore((s) => s.isExpanded);
  const expandDirection = useOrbStore((s) => s.expandDirection);
  const expandVertical = useOrbStore((s) => s.expandVertical);
  const loadConf = useSettingsStore((s) => s.loadConf);
  const session = useScreenshotStore((s) => s.session);
  const preparing = useScreenshotStore((s) => s.preparing);

  useScreenshotSession();

  React.useEffect(() => {
    void loadConf("orb");
    void loadConf("mascot");
  }, [loadConf]);

  const overlayActive = session !== null || preparing;

  const orbClassName = [
    isExpanded ? "orb-expanded" : "",
    `orb-expand-${expandDirection}`,
    `orb-expand-${expandVertical}`,
    overlayActive ? "orb-screenshot" : "",
  ]
    .filter(Boolean)
    .join(" ");

  /**
   * 所有 antd 浮层（Select / Dropdown / Tooltip / Popover / Modal）都挂进 orb 窗口根节点，
   * 而不是默认的 `document.body`。这样浮层属于 orb 自己的 DOM 子树，
   * 收起时可以由 `global.css` 的兜底规则整体隐藏，任何组件漏订阅关闭广播也不会盖住小球。
   */
  const rootRef = React.useRef<HTMLDivElement>(null);
  const getPopupContainer = React.useCallback(
    () => rootRef.current ?? document.body,
    [],
  );

  return (
    <ConfigProvider getPopupContainer={getPopupContainer}>
      <div
        ref={rootRef}
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
        {session ? <ScreenshotOverlay session={session} /> : null}
      </div>
    </ConfigProvider>
  );
};
