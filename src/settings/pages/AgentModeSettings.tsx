import React, { useEffect, useMemo, useState } from "react";
import {
  Button,
  Typography,
  Drawer,
  Form,
  Tag,
  Space,
  App,
  Select,
} from "antd";
import { EditOutlined, DeleteOutlined, UndoOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { translateKey } from "@/i18n";
import { useAgentModeStore, type AgentMode } from "@/stores/agentModeStore";
import type { ConversationToolConfig } from "@/chat/core/chatTypes";
import { DEFAULT_TOOL_CONFIG } from "@/chat/core/toolConfig";
import type {
  FieldSchema,
  FieldRenderProps,
  SelectOption,
} from "../core/types/fieldSchema";
import { SettingField } from "../shared/renderers/SettingField";
import { SettingsPageShell } from "../shared/layout/SettingsPageShell";
import {
  EntityListContent,
  ListHeaderActions,
  type SectionConfig,
} from "../shared/EntityListContent";
import {
  getAllAgentKinds,
  getAgentKindLabelKey,
  getAgentKindColor,
} from "../core/agentKind/agentModeRegistry";
import "../core/agentKind/agentModeDefinitions";
import { useSettingMessage } from "../shared/useSettingMessage";
import { PermissionGrantsSection } from "../shared/sections/PermissionGrantsSection";
import { ToolAssemblySection } from "../shared/sections/ToolAssemblySection";
import { REASONING_EFFORTS, type ReasoningEffort } from "@/chat/core/reasoning";
import {
  BUILTIN_AVATAR_OPTIONS,
  BUILTIN_AVATARS,
  resolveBuiltinAvatar,
} from "@/chat/message/avatars";

const AvatarPreview: React.FC<{ avatar: string; kind: string }> = ({
  avatar,
  kind,
}) => {
  const Builtin =
    resolveBuiltinAvatar(avatar) ??
    (kind === "agent"
      ? BUILTIN_AVATARS.agent
      : kind === "chat"
        ? BUILTIN_AVATARS.chat
        : BUILTIN_AVATARS.default);
  return (
    <div
      style={{
        width: 20,
        height: 20,
        flex: "0 0 auto",
        borderRadius: 6,
        overflow: "hidden",
      }}
    >
      <Builtin />
    </div>
  );
};

const AvatarFieldRender: React.FC<FieldRenderProps> = ({
  value,
  onChange,
  schema,
  disabled,
  fieldContext,
}) => {
  const avatar = typeof value === "string" ? value : "";
  const kind = (fieldContext?.formValues.kind as string | undefined) ?? "chat";
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
      <Select
        options={schema.options}
        value={avatar}
        onChange={(v) => onChange?.(v)}
        disabled={disabled}
        style={{ flex: 1 }}
      />
      <AvatarPreview avatar={avatar} kind={kind} />
    </div>
  );
};

interface ModeFormData {
  name: string;
  description: string;
  avatar: string;
  kind: string;
  prompt: string;
  model: string | null;
  temperature: number | null;
  maxTokens: number | null;
  reasoningEnabled: boolean | null;
  reasoningEffort: string | null;
}

const defaultFormData: ModeFormData = {
  name: "",
  description: "",
  avatar: "",
  kind: "chat",
  prompt: "",
  model: null,
  temperature: null,
  maxTokens: null,
  reasoningEnabled: null,
  reasoningEffort: null,
};

export const AgentModeSettings: React.FC = () => {
  const { t } = useTranslation();
  const store = useAgentModeStore();
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [editingMode, setEditingMode] = useState<AgentMode | null>(null);
  const [form] = Form.useForm<ModeFormData>();
  const [toolConfigDraft, setToolConfigDraft] =
    useState<ConversationToolConfig>(DEFAULT_TOOL_CONFIG);
  const msg = useSettingMessage();
  const { modal } = App.useApp();
  const [searchQuery, setSearchQuery] = useState("");

  const formValues = Form.useWatch([], form);

  const loadModes = useAgentModeStore((s) => s.loadModes);

  const kindOptions = useMemo(
    () =>
      getAllAgentKinds().map((k) => ({
        value: k.id,
        label: translateKey(k.label),
      })),
    [],
  );

  const avatarOptions = useMemo<SelectOption[]>(
    () => [
      { value: "", label: t("settings.agent_mode.avatar.none") },
      ...BUILTIN_AVATAR_OPTIONS.map((o) => ({
        value: o.value,
        label: translateKey(o.labelKey),
      })),
    ],
    [t],
  );

  const reasoningEffortOptions = useMemo(() => {
    const labels: Record<ReasoningEffort, string> = {
      low: t("settings.agent_mode.field.reasoning_effort.options.low"),
      medium: t("settings.agent_mode.field.reasoning_effort.options.medium"),
      high: t("settings.agent_mode.field.reasoning_effort.options.high"),
    };
    return REASONING_EFFORTS.map((value) => ({
      value,
      label: labels[value],
    }));
  }, [t]);

  const fieldSchemas = useMemo<FieldSchema[]>(
    () => [
      {
        key: "name",
        type: "string",
        label: t("settings.agent_mode.field.name.label"),
        placeholder: t("settings.agent_mode.field.name.placeholder"),
      },
      {
        key: "description",
        type: "textarea",
        label: t("settings.agent_mode.field.description.label"),
        rows: 2,
        placeholder: t("settings.agent_mode.field.description.placeholder"),
      },
      {
        key: "avatar",
        type: "select",
        label: t("settings.agent_mode.field.avatar.label"),
        description: t("settings.agent_mode.field.avatar.description"),
        options: avatarOptions,
        customRender: AvatarFieldRender,
      },
      {
        key: "kind",
        type: "select",
        label: t("settings.agent_mode.field.kind.label"),
        options: kindOptions,
      },
      {
        key: "prompt",
        type: "textarea",
        label: t("settings.agent_mode.field.prompt.label"),
        rows: 4,
        placeholder: t("settings.agent_mode.field.prompt.placeholder"),
      },
      {
        key: "model",
        type: "select",
        label: t("settings.agent_mode.field.model.label"),
        options: [],
        allowClear: true,
        placeholder: t("settings.agent_mode.field.model.placeholder"),
      },
      {
        key: "temperature",
        type: "slider",
        label: t("settings.agent_mode.field.temperature.label"),
        min: 0,
        max: 2,
        step: 0.1,
        marks: { 0: "0", 0.3: "0.3", 0.7: "0.7", 1: "1", 2: "2" },
      },
      {
        key: "maxTokens",
        type: "number",
        label: t("settings.agent_mode.field.max_tokens.label"),
        min: 256,
        max: 128_000,
        step: 256,
        placeholder: t("settings.agent_mode.field.max_tokens.placeholder"),
      },
      {
        key: "reasoningEnabled",
        type: "boolean",
        label: t("settings.agent_mode.field.reasoning_enabled.label"),
      },
      {
        key: "reasoningEffort",
        type: "select",
        label: t("settings.agent_mode.field.reasoning_effort.label"),
        options: reasoningEffortOptions,
        allowClear: true,
        placeholder: t(
          "settings.agent_mode.field.reasoning_effort.placeholder",
        ),
        visible: (fv) => fv.reasoningEnabled === true,
      },
    ],
    [t, kindOptions, reasoningEffortOptions, avatarOptions],
  );

  useEffect(() => {
    void loadModes();
  }, [loadModes]);

  const builtinModes = store.modes.filter((m) => m.isBuiltin);
  const customModes = store.modes.filter((m) => !m.isBuiltin);

  const filteredBuiltin = searchQuery
    ? builtinModes.filter(
        (m) =>
          m.name.includes(searchQuery) || m.description.includes(searchQuery),
      )
    : builtinModes;

  const filteredCustom = searchQuery
    ? customModes.filter(
        (m) =>
          m.name.includes(searchQuery) || m.description.includes(searchQuery),
      )
    : customModes;

  const handleAdd = () => {
    setEditingMode(null);
    form.resetFields();
    form.setFieldsValue(defaultFormData);
    setToolConfigDraft(DEFAULT_TOOL_CONFIG);
    setDrawerOpen(true);
  };

  const handleEdit = (mode: AgentMode) => {
    setEditingMode(mode);
    form.setFieldsValue({
      name: mode.name,
      description: mode.description,
      avatar: mode.avatar ?? "",
      kind: mode.kind === "agent" ? "agent" : "chat",
      prompt: mode.prompt,
      model: mode.model,
      temperature: mode.temperature,
      maxTokens: mode.maxTokens,
      reasoningEnabled: mode.reasoningEnabled,
      reasoningEffort: mode.reasoningEffort,
    });
    setToolConfigDraft(mode.toolConfig ?? DEFAULT_TOOL_CONFIG);
    setDrawerOpen(true);
  };

  const handleDelete = (id: string) => {
    modal.confirm({
      title: t("settings.agent_mode.confirm_delete.title"),
      content: t("settings.agent_mode.confirm_delete.content"),
      onOk: async () => {
        try {
          await store.deleteMode(id);
          msg.success("settings.agent_mode.deleted");
        } catch (e) {
          msg.error("settings.agent_mode.delete_failed", e);
        }
      },
    });
  };

  const handleReset = (mode: AgentMode) => {
    modal.confirm({
      title: t("settings.agent_mode.confirm_reset.title"),
      content: t("settings.agent_mode.confirm_reset.content", {
        name: mode.name,
      }),
      onOk: async () => {
        try {
          await store.resetBuiltinMode(mode.id);
          msg.success("settings.agent_mode.reset_success");
        } catch (e) {
          msg.error("settings.agent_mode.reset_failed", e);
        }
      },
    });
  };

  const handleSave = async () => {
    const values = await form.validateFields();
    const kind = (values.kind || "chat") as AgentMode["kind"];
    const toolConfig =
      kind === "agent"
        ? toolConfigDraft
        : (editingMode?.toolConfig ?? DEFAULT_TOOL_CONFIG);
    try {
      if (editingMode) {
        await store.saveMode({
          ...editingMode,
          ...values,
          kind,
          toolConfig,
          presentationRank: editingMode.presentationRank,
          isBuiltin: editingMode.isBuiltin,
        });
      } else {
        await store.saveMode({
          ...values,
          kind,
          toolConfig,
          presentationRank: 99,
          isBuiltin: false,
        });
      }
      msg.success(
        editingMode
          ? "settings.agent_mode.saved"
          : "settings.agent_mode.created",
      );
      setDrawerOpen(false);
      setEditingMode(null);
    } catch (e) {
      msg.error("settings.agent_mode.save_failed", e);
    }
  };

  const shouldShowField = (schema: FieldSchema): boolean => {
    if (!schema.visible) return true;
    const fv: Record<string, unknown> = {};
    if (formValues) {
      for (const [k, v] of Object.entries(formValues)) {
        fv[k] = v;
      }
    }
    return schema.visible(fv);
  };

  const isFieldDisabled = (schema: FieldSchema): boolean => {
    if (!editingMode) return false;
    if (schema.key === "kind") return true;
    return editingMode.isBuiltin && schema.key === "name";
  };

  const formKind = (formValues?.kind as string | undefined) ?? "chat";
  const showToolAssembly = formKind === "agent";

  const renderModeRow = (mode: AgentMode) => (
    <div key={mode.id} className="entity-list-row">
      <div className="entity-list-row-info">
        <Typography.Text strong className="entity-list-row-name">
          {mode.name}
        </Typography.Text>
        <Tag color={getAgentKindColor(mode.kind)} style={{ flexShrink: 0 }}>
          {translateKey(getAgentKindLabelKey(mode.kind))}
        </Tag>
        {mode.description && (
          <Typography.Text
            type="secondary"
            ellipsis
            className="entity-list-row-detail"
          >
            {mode.description}
          </Typography.Text>
        )}
        {mode.kind === "agent" && (
          <Typography.Text type="secondary" className="entity-list-row-model">
            {t("settings.agent_mode.tool_assembly.summary", {
              tier: t(
                `settings.agent_mode.field.max_tool_tier.options.${mode.toolConfig.maxToolTier}`,
              ),
              level: t(
                `settings.agent_mode.field.approval_level.options.${mode.toolConfig.approvalLevel}`,
              ),
              count: mode.toolConfig.enabledTools.length,
            })}
          </Typography.Text>
        )}
      </div>
      <Space size={4}>
        <Button
          type="text"
          size="small"
          icon={<EditOutlined />}
          onClick={() => handleEdit(mode)}
        />
        {mode.isBuiltin && (
          <Button
            type="text"
            size="small"
            icon={<UndoOutlined />}
            onClick={() => handleReset(mode)}
          />
        )}
        {!mode.isBuiltin && (
          <Button
            type="text"
            size="small"
            danger
            icon={<DeleteOutlined />}
            onClick={() => handleDelete(mode.id)}
          />
        )}
      </Space>
    </div>
  );

  const sections: SectionConfig<AgentMode>[] = [
    {
      label: t("settings.agent_mode.section.builtin"),
      items: filteredBuiltin,
      renderRow: renderModeRow,
    },
    {
      label: t("settings.agent_mode.section.custom"),
      items: filteredCustom,
      renderRow: renderModeRow,
    },
  ];

  return (
    <>
      <SettingsPageShell
        wide
        extra={
          <ListHeaderActions
            searchQuery={searchQuery}
            onSearchChange={setSearchQuery}
            addLabel={t("settings.agent_mode.add")}
            onAdd={handleAdd}
          />
        }
      >
        <EntityListContent
          items={store.modes}
          sections={sections}
          emptyDescription={t("settings.agent_mode.empty")}
        />
        <PermissionGrantsSection />
        <Drawer
          title={
            editingMode
              ? t("settings.agent_mode.edit")
              : t("settings.agent_mode.add")
          }
          open={drawerOpen}
          onClose={() => {
            setDrawerOpen(false);
            setEditingMode(null);
          }}
          size="large"
          extra={
            <Button type="primary" onClick={() => void handleSave()}>
              {t("common.actions.save")}
            </Button>
          }
        >
          <Form
            form={form}
            layout="vertical"
            initialValues={defaultFormData}
            onValuesChange={(changed) => {
              if ("reasoningEnabled" in changed && !changed.reasoningEnabled) {
                form.setFieldValue("reasoningEffort", null);
              }
            }}
          >
            {fieldSchemas.filter(shouldShowField).map((schema) => (
              <Form.Item
                key={schema.key}
                name={schema.key}
                label={schema.label}
                tooltip={
                  typeof schema.description === "string"
                    ? schema.description
                    : undefined
                }
                extra={
                  typeof schema.extra === "string" ? schema.extra : undefined
                }
                rules={
                  schema.key === "name"
                    ? [
                        {
                          required: true,
                          message: t("settings.agent_mode.field.name.required"),
                        },
                      ]
                    : undefined
                }
                getValueProps={
                  schema.key === "temperature"
                    ? (v: unknown) => ({ value: v ?? 0.7 })
                    : undefined
                }
              >
                <SettingField
                  schema={schema}
                  value={null}
                  onChange={() => {}}
                  disabled={isFieldDisabled(schema)}
                  fieldContext={
                    schema.key === "avatar"
                      ? {
                          mode: editingMode ? "edit" : "add",
                          formValues: { ...(formValues ?? {}) },
                        }
                      : undefined
                  }
                />
              </Form.Item>
            ))}
          </Form>
          {showToolAssembly && (
            <ToolAssemblySection
              config={toolConfigDraft}
              onChange={setToolConfigDraft}
            />
          )}
        </Drawer>
      </SettingsPageShell>
    </>
  );
};
