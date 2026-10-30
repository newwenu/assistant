import React, { useEffect, useMemo } from "react";
import { Typography } from "antd";
import { RightOutlined } from "@ant-design/icons";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { translateKey } from "@/i18n";
import { useSettingsStore } from "@/stores/settingsStore";
import { getSettingsDetailPages } from "../core/pageRegistry";
import type { SettingsPageManifest } from "../core/types/pageTypes";
import {
  getSectionsByPage,
  getDefinitionsBySection,
  CONF_KEY_MAP,
  type ConfGroup,
  type SettingDefinition,
} from "../core/field/fieldRegistry";
import { toConfRecord } from "../core/types/confUtils";

function useConfByGroup(group: ConfGroup): Record<string, unknown> | null {
  const confKey = CONF_KEY_MAP[group];
  const conf = useSettingsStore((s) => s[confKey]);
  return toConfRecord(conf);
}

function readFieldValue(
  def: SettingDefinition,
  conf: Record<string, unknown>,
): unknown {
  return def.readValue ? def.readValue(conf) : conf[def.key];
}

function formatSummaryValue(def: SettingDefinition, value: unknown): string {
  if (value == null) return "-";
  if (typeof value === "string") return value;
  if (typeof value === "number") return String(value);
  if (typeof value === "boolean") {
    return value
      ? translateKey("settings.overview.on")
      : translateKey("settings.overview.off");
  }
  switch (def.type) {
    case "boolean":
      return value
        ? translateKey("settings.overview.on")
        : translateKey("settings.overview.off");
    case "select": {
      const opt = def.options?.find((o) => o.value === value);
      return opt
        ? translateKey(opt.label)
        : typeof value === "string"
          ? value
          : `${value as number}`;
    }
    case "slider":
      return `${value as number}`;
    case "keybinding":
      return value as string;
    default:
      return "-";
  }
}

interface SummaryPanelProps {
  title: string;
  pageId: string;
  icon: React.FC;
  children: React.ReactNode;
}

const SummaryPanel: React.FC<SummaryPanelProps> = ({
  title,
  pageId,
  icon: Icon,
  children,
}) => {
  const navigate = useNavigate();

  return (
    <div
      className="settings-summary-panel"
      role="button"
      tabIndex={0}
      onClick={() => {
        void navigate(`/settings/${pageId}`);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          void navigate(`/settings/${pageId}`);
        }
      }}
      style={{ cursor: "pointer" }}
    >
      <div className="settings-summary-panel-header">
        <div className="settings-summary-panel-title">
          <span className="settings-summary-panel-icon">
            <Icon />
          </span>
          <Typography.Text strong style={{ fontSize: 15 }}>
            {title}
          </Typography.Text>
        </div>
        <RightOutlined style={{ color: "var(--color-text-quaternary)" }} />
      </div>
      <div className="settings-summary-panel-body">{children}</div>
    </div>
  );
};

const FieldDrivenSummary: React.FC<{ page: SettingsPageManifest }> = ({
  page,
}) => {
  const sections = useMemo(() => getSectionsByPage(page.id), [page.id]);

  const allGroups = useMemo(() => {
    const groups = new Set<ConfGroup>();
    for (const sec of sections) {
      for (const def of getDefinitionsBySection(sec.id)) {
        groups.add(def.group);
      }
    }
    return groups;
  }, [sections]);

  const loadAppConf = useSettingsStore((s) => s.loadAppConf);
  const loadOrbConf = useSettingsStore((s) => s.loadOrbConf);
  const loadMascotConf = useSettingsStore((s) => s.loadMascotConf);
  const loadVaultConf = useSettingsStore((s) => s.loadVaultConf);

  useEffect(() => {
    if (allGroups.has("app")) void loadAppConf();
    if (allGroups.has("orb")) void loadOrbConf();
    if (allGroups.has("mascot")) void loadMascotConf();
    if (allGroups.has("vault")) void loadVaultConf();
  }, [allGroups, loadAppConf, loadOrbConf, loadMascotConf, loadVaultConf]);

  const appConf = useConfByGroup("app");
  const orbConf = useConfByGroup("orb");
  const mascotConf = useConfByGroup("mascot");
  const vaultConf = useConfByGroup("vault");

  const items = useMemo(() => {
    const confMap: Record<ConfGroup, Record<string, unknown> | null> = {
      app: appConf,
      orb: orbConf,
      mascot: mascotConf,
      vault: vaultConf,
    };
    const result: { key: string; label: string; value: string }[] = [];
    for (const sec of sections) {
      const defs = getDefinitionsBySection(sec.id);
      for (const def of defs) {
        if (def.type === "color" || def.type === "textarea") continue;
        const conf = confMap[def.group];
        if (!conf) continue;
        if (def.visible && !def.visible(conf)) continue;
        const rawValue = readFieldValue(def, conf);
        result.push({
          key: def.key,
          label: translateKey(def.label),
          value: formatSummaryValue(def, rawValue),
        });
      }
    }
    return result;
  }, [sections, appConf, orbConf, mascotConf, vaultConf]);

  if (items.length === 0) return null;

  const MAX_PREVIEW = 5;
  const visible = items.slice(0, MAX_PREVIEW);
  const remaining = items.length - MAX_PREVIEW;

  return (
    <SummaryPanel
      title={translateKey(page.label)}
      pageId={page.id}
      icon={page.icon}
    >
      <div className="settings-summary-kv-grid">
        {visible.map((item) => (
          <div key={item.key} className="settings-summary-kv-item">
            <span className="settings-summary-kv-key">{item.label}</span>
            <span className="settings-summary-kv-value">{item.value}</span>
          </div>
        ))}
        {remaining > 0 && (
          <div className="settings-summary-kv-item">
            <span
              className="settings-summary-kv-value"
              style={{
                color: "var(--color-text-quaternary)",
                fontStyle: "italic",
              }}
            >
              {translateKey("settings.overview.more_items", {
                count: remaining,
              })}
            </span>
          </div>
        )}
      </div>
    </SummaryPanel>
  );
};

const NavOnlySummary: React.FC<{ page: SettingsPageManifest }> = ({ page }) => {
  const navigate = useNavigate();

  return (
    <div
      className="settings-summary-panel"
      role="button"
      tabIndex={0}
      onClick={() => {
        void navigate(`/settings/${page.id}`);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          void navigate(`/settings/${page.id}`);
        }
      }}
      style={{ cursor: "pointer" }}
    >
      <div className="settings-summary-panel-header">
        <div className="settings-summary-panel-title">
          <span className="settings-summary-panel-icon">
            <page.icon />
          </span>
          <Typography.Text strong style={{ fontSize: 15 }}>
            {translateKey(page.label)}
          </Typography.Text>
        </div>
        <RightOutlined style={{ color: "var(--color-text-quaternary)" }} />
      </div>
    </div>
  );
};

export const SettingsOverview: React.FC = () => {
  const { t } = useTranslation();

  return (
    <div className="settings-overview">
      <div className="settings-overview-header">
        <Typography.Title level={4} style={{ margin: 0, fontWeight: 600 }}>
          {t("settings.overview.title")}
        </Typography.Title>
        <Typography.Text type="secondary" style={{ fontSize: 13 }}>
          {t("settings.overview.subtitle")}
        </Typography.Text>
      </div>

      <div className="settings-overview-panels">
        {getSettingsDetailPages().map((page) =>
          getSectionsByPage(page.id).length > 0 ? (
            <FieldDrivenSummary key={page.id} page={page} />
          ) : (
            <NavOnlySummary key={page.id} page={page} />
          ),
        )}
      </div>
    </div>
  );
};
