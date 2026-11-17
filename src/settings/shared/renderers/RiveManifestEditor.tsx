import { type ReactNode, useCallback, useMemo, useState } from "react";
import {
  AutoComplete,
  Button,
  Input,
  Modal,
  Select,
  Space,
  Typography,
} from "antd";
import { useTranslation } from "react-i18next";
import type { RiveManifestDto } from "@/types/protocol.contract";
import { settingService } from "@/services/settingService";
import { bumpRiveManifestRevision } from "@/mascot/renderers/rive/riveManifestRevision";
import { FALLBACK_RIVE_BINDINGS } from "@/mascot/renderers/rive/riveIntrospect";
import {
  MASCOT_SCENE_DEFS,
  MASCOT_SCENE_GROUPS,
  MASCOT_SCENE_GROUP_LABEL_KEYS,
  isMascotScene,
  sceneFace,
  scenesInGroup,
} from "@/mascot/core/expressionPolicy";
import { MASCOT_SCENES } from "@/mascot/core/types";
import { emitSettingEventToPeers } from "@/services/system/crossWindowEvent";
import { SettingAction, SettingEvent } from "@/types/events";
import type { AssetManifestEditorProps } from "../../core/types/fieldSchema";
import { useSettingMessage } from "../useSettingMessage";

type ManifestPropertyKey =
  "pose" | "viseme" | "primaryColor" | "secondaryColor";

type ManifestPropertyLabelKey =
  | "settings.asset_picker.manifest.property_pose"
  | "settings.asset_picker.manifest.property_viseme"
  | "settings.asset_picker.manifest.property_primary_color"
  | "settings.asset_picker.manifest.property_secondary_color";

const PROPERTY_FIELDS: readonly {
  key: ManifestPropertyKey;
  labelKey: ManifestPropertyLabelKey;
  placeholder: string;
}[] = [
  {
    key: "pose",
    labelKey: "settings.asset_picker.manifest.property_pose",
    placeholder: FALLBACK_RIVE_BINDINGS.pose,
  },
  {
    key: "viseme",
    labelKey: "settings.asset_picker.manifest.property_viseme",
    placeholder: FALLBACK_RIVE_BINDINGS.viseme,
  },
  {
    key: "primaryColor",
    labelKey: "settings.asset_picker.manifest.property_primary_color",
    placeholder: FALLBACK_RIVE_BINDINGS.primaryColor,
  },
  {
    key: "secondaryColor",
    labelKey: "settings.asset_picker.manifest.property_secondary_color",
    placeholder: FALLBACK_RIVE_BINDINGS.secondaryColor,
  },
];

interface ManifestDraft {
  stateMachine: string;
  pose: string;
  viseme: string;
  primaryColor: string;
  secondaryColor: string;
  poses: string[];
  visemes: string[];
  sceneToPose: Record<string, string>;
  faceToPose: Record<string, string>;
  idlePoseCycle: string[];
}

const EMPTY_DRAFT: ManifestDraft = {
  stateMachine: "",
  pose: "",
  viseme: "",
  primaryColor: "",
  secondaryColor: "",
  poses: [],
  visemes: [],
  sceneToPose: {},
  faceToPose: {},
  idlePoseCycle: [],
};

function toDraft(manifest: RiveManifestDto | null): ManifestDraft {
  if (!manifest) return EMPTY_DRAFT;
  return {
    stateMachine: manifest.stateMachine ?? "",
    pose: manifest.properties?.pose ?? "",
    viseme: manifest.properties?.viseme ?? "",
    primaryColor: manifest.properties?.primaryColor ?? "",
    secondaryColor: manifest.properties?.secondaryColor ?? "",
    poses: [...(manifest.poses ?? [])],
    visemes: [...(manifest.visemes ?? [])],
    sceneToPose: { ...(manifest.sceneToPose ?? {}) },
    faceToPose: { ...(manifest.faceToPose ?? {}) },
    idlePoseCycle: [...(manifest.idlePoseCycle ?? [])],
  };
}

function trimmedOrUndefined(value: string): string | undefined {
  const trimmed = value.trim();
  return trimmed.length === 0 ? undefined : trimmed;
}

function normalizeTags(values: string[]): string[] {
  return values
    .map((value) => value.trim())
    .filter((value) => value.length > 0);
}

function toManifest(draft: ManifestDraft): RiveManifestDto {
  const pose = trimmedOrUndefined(draft.pose);
  const viseme = trimmedOrUndefined(draft.viseme);
  const primaryColor = trimmedOrUndefined(draft.primaryColor);
  const secondaryColor = trimmedOrUndefined(draft.secondaryColor);
  const hasProperties =
    pose !== undefined ||
    viseme !== undefined ||
    primaryColor !== undefined ||
    secondaryColor !== undefined;

  const sceneToPose = Object.fromEntries(
    Object.entries(draft.sceneToPose)
      .map(([scene, pose]) => [scene, pose.trim()] as const)
      .filter(([scene, value]) => isMascotScene(scene) && value.length > 0),
  );
  const faceToPose = Object.fromEntries(
    Object.entries(draft.faceToPose)
      .map(([face, pose]) => [face, pose.trim()] as const)
      .filter(([, value]) => value.length > 0),
  );
  const poses = normalizeTags(draft.poses);
  const visemes = normalizeTags(draft.visemes);
  const idlePoseCycle = normalizeTags(draft.idlePoseCycle);

  return {
    stateMachine: trimmedOrUndefined(draft.stateMachine),
    properties: hasProperties
      ? { pose, viseme, primaryColor, secondaryColor }
      : undefined,
    poses: poses.length > 0 ? poses : undefined,
    visemes: visemes.length > 0 ? visemes : undefined,
    sceneToPose: Object.keys(sceneToPose).length > 0 ? sceneToPose : undefined,
    faceToPose: Object.keys(faceToPose).length > 0 ? faceToPose : undefined,
    idlePoseCycle: idlePoseCycle.length > 0 ? idlePoseCycle : undefined,
  };
}

interface SectionProps {
  title: string;
  hint?: string;
  children: ReactNode;
}

function Section({ title, hint, children }: SectionProps) {
  return (
    <div>
      <Typography.Text strong>{title}</Typography.Text>
      {hint && (
        <Typography.Text type="secondary" style={{ marginLeft: 8 }}>
          {hint}
        </Typography.Text>
      )}
      <div style={{ marginTop: 8 }}>{children}</div>
    </div>
  );
}

export function RiveManifestEditor({ assetType }: AssetManifestEditorProps) {
  const { t } = useTranslation();
  const msg = useSettingMessage();
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState<ManifestDraft>(EMPTY_DRAFT);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);

  const openEditor = useCallback(() => {
    setOpen(true);
    setLoading(true);
    void settingService
      .getMascotManifest(assetType)
      .then((manifest) => {
        setDraft(toDraft(manifest));
      })
      .catch((e: unknown) => {
        msg.error("settings.asset_picker.manifest.load_failed", e);
        setDraft(EMPTY_DRAFT);
      })
      .finally(() => {
        setLoading(false);
      });
  }, [assetType, msg]);

  const persist = useCallback(
    async (manifest: RiveManifestDto, successKey: string) => {
      setSaving(true);
      try {
        await settingService.setMascotManifest(assetType, manifest);
        bumpRiveManifestRevision();
        void emitSettingEventToPeers(SettingEvent.MascotManifestChanged, {
          action: SettingAction.MascotManifestChanged,
          id: assetType,
        });
        setOpen(false);
        msg.success(successKey);
      } catch (e) {
        msg.error("settings.asset_picker.manifest.save_failed", e);
      } finally {
        setSaving(false);
      }
    },
    [assetType, msg],
  );

  const handleSave = useCallback(() => {
    void persist(toManifest(draft), "settings.asset_picker.manifest.saved");
  }, [draft, persist]);

  const handleClear = useCallback(() => {
    setDraft(EMPTY_DRAFT);
    void persist({}, "settings.asset_picker.manifest.cleared");
  }, [persist]);

  const patch = useCallback(
    <K extends keyof ManifestDraft>(key: K, value: ManifestDraft[K]) => {
      setDraft((prev) => ({ ...prev, [key]: value }));
    },
    [],
  );

  const setProperty = useCallback((key: ManifestPropertyKey, value: string) => {
    setDraft((prev) => ({ ...prev, [key]: value }));
  }, []);

  const setScenePose = useCallback((scene: string, pose: string) => {
    setDraft((prev) => {
      const next = { ...prev.sceneToPose };
      const trimmed = pose.trim();
      if (trimmed.length === 0) delete next[scene];
      else next[scene] = trimmed;
      return { ...prev, sceneToPose: next };
    });
  }, []);

  const legacyFaceCount = Object.keys(draft.faceToPose).length;

  const migrateLegacyFaceToPose = useCallback(() => {
    setDraft((prev) => {
      const next = { ...prev.sceneToPose };
      for (const scene of MASCOT_SCENES) {
        const pose = prev.faceToPose[sceneFace(scene)]?.trim();
        if (pose && !next[scene]?.trim()) next[scene] = pose;
      }
      return { ...prev, sceneToPose: next };
    });
  }, []);

  const clearLegacyFaceToPose = useCallback(() => {
    setDraft((prev) => ({ ...prev, faceToPose: {} }));
  }, []);

  const poseOptions = useMemo(
    () => draft.poses.map((pose) => ({ value: pose, label: pose })),
    [draft.poses],
  );

  const disabled = loading || saving;

  return (
    <>
      <Button size="small" onClick={openEditor}>
        {t("settings.asset_picker.manifest.open")}
      </Button>
      <Modal
        open={open}
        title={t("settings.asset_picker.manifest.title")}
        width={720}
        destroyOnHidden
        mask={{ closable: false }}
        onCancel={() => setOpen(false)}
        styles={{ body: { maxHeight: "60vh", overflowY: "auto" } }}
        footer={
          <Space>
            <Button
              danger
              loading={saving}
              disabled={loading}
              onClick={handleClear}
            >
              {t("settings.asset_picker.manifest.clear")}
            </Button>
            <Button onClick={() => setOpen(false)}>
              {t("common.actions.cancel")}
            </Button>
            <Button
              type="primary"
              loading={saving}
              disabled={loading}
              onClick={handleSave}
            >
              {t("common.actions.save")}
            </Button>
          </Space>
        }
      >
        <Space orientation="vertical" size="middle" style={{ width: "100%" }}>
          <Typography.Text type="secondary">
            {t("settings.asset_picker.manifest.hint")}
          </Typography.Text>

          <Section
            title={t("settings.asset_picker.manifest.state_machine")}
            hint={t("settings.asset_picker.manifest.state_machine_hint")}
          >
            <Input
              value={draft.stateMachine}
              disabled={disabled}
              placeholder={t(
                "settings.asset_picker.manifest.state_machine_placeholder",
              )}
              onChange={(e) => patch("stateMachine", e.target.value)}
            />
          </Section>

          <Section
            title={t("settings.asset_picker.manifest.properties")}
            hint={t("settings.asset_picker.manifest.properties_hint")}
          >
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "1fr 1fr",
                gap: 8,
              }}
            >
              {PROPERTY_FIELDS.map((field) => (
                <div
                  key={field.key}
                  style={{
                    display: "flex",
                    flexDirection: "column",
                    gap: 4,
                  }}
                >
                  <Typography.Text type="secondary">
                    {t(field.labelKey)}
                  </Typography.Text>
                  <Input
                    value={draft[field.key]}
                    disabled={disabled}
                    placeholder={field.placeholder}
                    onChange={(e) => setProperty(field.key, e.target.value)}
                  />
                </div>
              ))}
            </div>
          </Section>

          <Section
            title={t("settings.asset_picker.manifest.poses")}
            hint={t("settings.asset_picker.manifest.poses_hint")}
          >
            <Select
              mode="tags"
              style={{ width: "100%" }}
              value={draft.poses}
              disabled={disabled}
              tokenSeparators={[",", " "]}
              placeholder={t("settings.asset_picker.manifest.tags_placeholder")}
              onChange={(values: string[]) => patch("poses", values)}
            />
          </Section>

          <Section
            title={t("settings.asset_picker.manifest.visemes")}
            hint={t("settings.asset_picker.manifest.visemes_hint")}
          >
            <Select
              mode="tags"
              style={{ width: "100%" }}
              value={draft.visemes}
              disabled={disabled}
              tokenSeparators={[",", " "]}
              placeholder={t("settings.asset_picker.manifest.tags_placeholder")}
              onChange={(values: string[]) => patch("visemes", values)}
            />
          </Section>

          <Section
            title={t("settings.asset_picker.manifest.scene_to_pose")}
            hint={t("settings.asset_picker.manifest.scene_to_pose_hint")}
          >
            <Space
              orientation="vertical"
              size="small"
              style={{ width: "100%" }}
            >
              {MASCOT_SCENE_GROUPS.map((group) => (
                <div key={group}>
                  <Typography.Text
                    strong
                    style={{ display: "block", marginBottom: 6 }}
                  >
                    {t(MASCOT_SCENE_GROUP_LABEL_KEYS[group])}
                  </Typography.Text>
                  <div
                    style={{
                      display: "grid",
                      gridTemplateColumns: "1fr 1fr",
                      gap: 8,
                    }}
                  >
                    {scenesInGroup(group).map((scene) => {
                      const def = MASCOT_SCENE_DEFS[scene];
                      return (
                        <div
                          key={scene}
                          style={{
                            display: "flex",
                            flexDirection: "column",
                            gap: 2,
                          }}
                        >
                          <Typography.Text>{t(def.labelKey)}</Typography.Text>
                          <Typography.Text
                            type="secondary"
                            style={{ fontSize: 12 }}
                          >
                            {t(def.hintKey)}
                          </Typography.Text>
                          <AutoComplete
                            style={{ width: "100%" }}
                            value={draft.sceneToPose[scene] ?? ""}
                            options={poseOptions}
                            disabled={disabled}
                            placeholder={t(
                              "settings.asset_picker.manifest.scene_placeholder",
                            )}
                            onChange={(value: string) =>
                              setScenePose(scene, value)
                            }
                          />
                        </div>
                      );
                    })}
                  </div>
                </div>
              ))}
            </Space>
          </Section>

          {legacyFaceCount > 0 && (
            <Section
              title={t("settings.asset_picker.manifest.legacy_face_to_pose")}
              hint={t(
                "settings.asset_picker.manifest.legacy_face_to_pose_hint",
              )}
            >
              <Space wrap>
                <Typography.Text type="secondary">
                  {t(
                    "settings.asset_picker.manifest.legacy_face_to_pose_count",
                    { count: legacyFaceCount },
                  )}
                </Typography.Text>
                <Button
                  size="small"
                  disabled={disabled}
                  onClick={migrateLegacyFaceToPose}
                >
                  {t("settings.asset_picker.manifest.legacy_migrate")}
                </Button>
                <Button
                  size="small"
                  danger
                  disabled={disabled}
                  onClick={clearLegacyFaceToPose}
                >
                  {t("settings.asset_picker.manifest.legacy_clear")}
                </Button>
              </Space>
            </Section>
          )}

          <Section
            title={t("settings.asset_picker.manifest.idle_pose_cycle")}
            hint={t("settings.asset_picker.manifest.idle_pose_cycle_hint")}
          >
            <Select
              mode="tags"
              style={{ width: "100%" }}
              value={draft.idlePoseCycle}
              disabled={disabled}
              options={poseOptions}
              tokenSeparators={[",", " "]}
              placeholder={t("settings.asset_picker.manifest.tags_placeholder")}
              onChange={(values: string[]) => patch("idlePoseCycle", values)}
            />
          </Section>
        </Space>
      </Modal>
    </>
  );
}
