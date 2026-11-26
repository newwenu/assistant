import React, { useCallback, useEffect, useRef, useState } from "react";
import { Tabs } from "antd";
import { useTranslation } from "react-i18next";
import { SettingsPageShell } from "../shared/layout/SettingsPageShell";
import { ProcessTreeTab } from "./resource/ProcessTreeTab";
import { StorageContent } from "./storage/StorageContent";

type ResourceTabKey = "storage" | "processTree";

const RESOURCE_TABS: readonly ResourceTabKey[] = ["storage", "processTree"];

function isResourceTabKey(value: string): value is ResourceTabKey {
  return RESOURCE_TABS.some((key) => key === value);
}

function findScrollParent(node: HTMLElement | null): HTMLElement | null {
  let current = node?.parentElement ?? null;
  while (current !== null) {
    const overflowY = getComputedStyle(current).overflowY;
    if (overflowY === "auto" || overflowY === "scroll") return current;
    current = current.parentElement;
  }
  return null;
}

export const StorageSettings: React.FC = () => {
  const { t } = useTranslation();
  const rootRef = useRef<HTMLDivElement>(null);
  const scrollTopsRef = useRef<Record<ResourceTabKey, number>>({
    storage: 0,
    processTree: 0,
  });
  const [activeKey, setActiveKey] = useState<ResourceTabKey>("storage");

  useEffect(() => {
    const scroller = findScrollParent(rootRef.current);
    if (scroller !== null) {
      scroller.scrollTop = scrollTopsRef.current[activeKey];
    }
  }, [activeKey]);

  const handleChange = useCallback(
    (key: string) => {
      if (!isResourceTabKey(key) || key === activeKey) return;
      const scroller = findScrollParent(rootRef.current);
      if (scroller !== null) {
        scrollTopsRef.current[activeKey] = scroller.scrollTop;
      }
      setActiveKey(key);
    },
    [activeKey],
  );

  return (
    <SettingsPageShell wide>
      <div ref={rootRef}>
        <Tabs
          className="resource-tabs"
          activeKey={activeKey}
          onChange={handleChange}
          items={[
            {
              key: "storage",
              label: t("settings.resource.tab.storage"),
              children: <StorageContent />,
            },
            {
              key: "processTree",
              label: t("settings.resource.tab.processTree"),
              children: <ProcessTreeTab active={activeKey === "processTree"} />,
            },
          ]}
        />
      </div>
    </SettingsPageShell>
  );
};
