import { useEffect, useRef } from "react";
import { Form, message } from "antd";
import { useSearchParams, useLocation } from "react-router-dom";
import type { SettingDefinition, ConfGroup } from "../core/fieldRegistry";
import {
  getDefinitionsByGroup,
  INTERMEDIATE_TYPES,
} from "../core/fieldRegistry";
import { runSideEffects } from "../core/sideEffectRegistry";
import { SettingField } from "./SettingField";
import { toConfRecord, optimisticConfUpdate } from "../core/settingsDispatcher";
import { useConfPatchHandlers } from "../core/useConfPatchHandlers";
import { useSettingsStore } from "@/stores/settingsStore";

interface SettingsGroupProps {
  group: ConfGroup;
  definitions?: SettingDefinition[];
  onPatch?: (patch: Record<string, unknown>) => Promise<void>;
  onPatchComplete?: (patch: Record<string, unknown>) => Promise<void>;
}

const DEFAULT_SAVE_MESSAGE = "已保存";

function useConfByGroup(group: ConfGroup): Record<string, unknown> | null {
  const conf = useSettingsStore((s) => {
    if (group === "app") return s.appConf;
    if (group === "orb") return s.orbConf;
    return s.mascotConf;
  });
  return toConfRecord(conf);
}

function readFieldValue(
  def: SettingDefinition,
  conf: Record<string, unknown>,
): unknown {
  return def.readValue ? def.readValue(conf) : conf[def.key];
}

function buildFieldPatch(
  def: SettingDefinition,
  value: unknown,
  conf: Record<string, unknown>,
): Record<string, unknown> {
  return def.buildPatch ? def.buildPatch(value, conf) : { [def.key]: value };
}

export const SettingsGroup: React.FC<SettingsGroupProps> = ({
  group,
  definitions,
  onPatch: externalOnPatch,
  onPatchComplete: externalOnPatchComplete,
}) => {
  const defs = definitions ?? getDefinitionsByGroup(group);
  const conf = useConfByGroup(group);
  const { onPatch: internalOnPatch, onPatchComplete: internalOnPatchComplete } =
    useConfPatchHandlers(group);
  const onPatch = externalOnPatch ?? internalOnPatch;
  const onPatchComplete = externalOnPatchComplete ?? internalOnPatchComplete;

  const [messageApi, contextHolder] = message.useMessage();
  const [searchParams] = useSearchParams();
  const focusKey = searchParams.get("focus");
  const location = useLocation();
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!focusKey) return;
    const container = containerRef.current;
    if (!container) return;

    let selector: string;
    if (focusKey.startsWith("section:")) {
      const sectionId = focusKey.slice("section:".length);
      selector = `[data-section-id="${sectionId}"]`;
    } else {
      selector = `[data-setting-key="${focusKey}"]`;
    }

    const el = container.querySelector(selector) as HTMLElement | null;
    if (!el) {
      const root = container.closest(".settings-page-shell");
      if (root) {
        const rootEl = root.querySelector(selector) as HTMLElement | null;
        if (rootEl) {
          rootEl.scrollIntoView({ behavior: "smooth", block: "center" });
          rootEl.classList.add("setting-focus-highlight");
          const timer = setTimeout(
            () => rootEl.classList.remove("setting-focus-highlight"),
            2000,
          );
          return () => clearTimeout(timer);
        }
      }
      return;
    }
    el.scrollIntoView({ behavior: "smooth", block: "center" });
    el.classList.add("setting-focus-highlight");
    const timer = setTimeout(
      () => el.classList.remove("setting-focus-highlight"),
      2000,
    );
    return () => clearTimeout(timer);
  }, [focusKey, location.pathname, location.search]);

  if (!conf) return null;

  const visibleDefs = defs.filter((d) => !d.visible || d.visible(conf));

  const showSaveMessage = (def: SettingDefinition) => {
    if (def.saveMessage === false) return;
    const msg = def.saveMessage || DEFAULT_SAVE_MESSAGE;
    messageApi.success(msg);
  };

  const handleChange = async (def: SettingDefinition, value: unknown) => {
    const patch = buildFieldPatch(def, value, conf);
    try {
      await onPatch(patch);
      showSaveMessage(def);
      if (def.sideEffect) {
        runSideEffects(def.sideEffect, conf, { ...conf, [def.key]: value });
      }
    } catch (e) {
      console.error(`[SettingsGroup] patch failed for ${def.key}:`, e);
    }
  };

  const handleIntermediateChange = (key: string, value: unknown) => {
    optimisticConfUpdate(group, key, value);
  };

  const handleIntermediateChangeComplete = async (
    def: SettingDefinition,
    value: unknown,
  ) => {
    const patch = buildFieldPatch(def, value, conf);
    try {
      await onPatchComplete(patch);
      showSaveMessage(def);
      if (def.sideEffect) {
        runSideEffects(def.sideEffect, conf, { ...conf, [def.key]: value });
      }
    } catch (e) {
      console.error(`[SettingsGroup] patch failed for ${def.key}:`, e);
    }
  };

  return (
    <>
      <div ref={containerRef} style={{ display: "contents" }}>
        {contextHolder}
        {visibleDefs.map((d) => {
          const isIntermediate = INTERMEDIATE_TYPES.has(d.type as never);
          return (
            <span key={d.key} data-setting-key={d.key}>
              <Form.Item
                label={d.label}
                tooltip={d.description || undefined}
                extra={d.extra}
                valuePropName={d.type === "boolean" ? "checked" : "value"}
              >
                <SettingField
                  def={d}
                  value={readFieldValue(d, conf)}
                  onChange={
                    isIntermediate
                      ? (v) => handleIntermediateChange(d.key, v)
                      : (v) => handleChange(d, v)
                  }
                  onChangeComplete={
                    isIntermediate
                      ? (v) => handleIntermediateChangeComplete(d, v)
                      : undefined
                  }
                  conf={conf}
                />
              </Form.Item>
            </span>
          );
        })}
      </div>
    </>
  );
};
