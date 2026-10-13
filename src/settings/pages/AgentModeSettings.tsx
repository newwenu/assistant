import React, { useEffect, useMemo, useState } from "react";
import {
  Button,
  Typography,
  Modal,
  Drawer,
  Form,
  Tag,
  Space,
  message,
} from "antd";
import { EditOutlined, DeleteOutlined, UndoOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { translateKey } from "@/i18n";
import { useAgentModeStore, type AgentMode } from "@/stores/agentModeStore";
import type { FieldSchema } from "../core/fieldSchema";
import { SettingField } from "../shared/SettingField";
import {
  EntityListPageShell,
  type EntityListSectionConfig,
} from "../shared/EntityListPageShell";
import {
  getAllAgentKinds,
  getAgentKindLabelKey,
  getAgentKindColor,
} from "../core/agentModeRegistry";
import "../core/agentModeDefinitions";

const REASONING_EFFORT_OPTIONS = [
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
];

interface ModeFormData {
  name: string;
  description: string;
  kind: string;
  prompt: string;
  model: string | null;
  temperature: number | null;
  max_tokens: number | null;
  reasoning_enabled: boolean | null;
  reasoning_effort: string | null;
}

const defaultFormData: ModeFormData = {
  name: "",
  description: "",
  kind: "custom",
  prompt: "",
  model: null,
  temperature: null,
  max_tokens: null,
  reasoning_enabled: null,
  reasoning_effort: null,
};

export const AgentModeSettings: React.FC = () => {
  const { t } = useTranslation();
  const store = useAgentModeStore();
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [editingMode, setEditingMode] = useState<AgentMode | null>(null);
  const [form] = Form.useForm<ModeFormData>();
  const [messageApi, messageContextHolder] = message.useMessage();
  const [modal, modalContextHolder] = Modal.useModal();
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
        key: "max_tokens",
        type: "number",
        label: t("settings.agent_mode.field.max_tokens.label"),
        min: 256,
        max: 128_000,
        step: 256,
        placeholder: t("settings.agent_mode.field.max_tokens.placeholder"),
      },
      {
        key: "reasoning_enabled",
        type: "boolean",
        label: t("settings.agent_mode.field.reasoning_enabled.label"),
      },
      {
        key: "reasoning_effort",
        type: "select",
        label: t("settings.agent_mode.field.reasoning_effort.label"),
        options: REASONING_EFFORT_OPTIONS,
        allowClear: true,
        placeholder: t(
          "settings.agent_mode.field.reasoning_effort.placeholder",
        ),
        visible: (fv) => fv.reasoning_enabled === true,
      },
    ],
    [t, kindOptions],
  );

  useEffect(() => {
    void loadModes();
  }, [loadModes]);

  const builtinModes = store.modes.filter((m) => m.is_builtin);
  const customModes = store.modes.filter((m) => !m.is_builtin);

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

  const isBuiltinEditing = !!editingMode?.is_builtin;

  const handleAdd = () => {
    setEditingMode(null);
    form.resetFields();
    form.setFieldsValue(defaultFormData);
    setDrawerOpen(true);
  };

  const handleEdit = (mode: AgentMode) => {
    setEditingMode(mode);
    form.setFieldsValue({
      name: mode.name,
      description: mode.description,
      kind: mode.kind,
      prompt: mode.prompt,
      model: mode.model,
      temperature: mode.temperature,
      max_tokens: mode.max_tokens,
      reasoning_enabled: mode.reasoning_enabled,
      reasoning_effort: mode.reasoning_effort,
    });
    setDrawerOpen(true);
  };

  const handleDelete = (id: string) => {
    modal.confirm({
      title: t("settings.agent_mode.confirm_delete.title"),
      content: t("settings.agent_mode.confirm_delete.content"),
      onOk: async () => {
        try {
          await store.deleteMode(id);
          messageApi.success(t("settings.agent_mode.deleted"));
        } catch (e) {
          messageApi.error(
            t("settings.agent_mode.delete_failed", {
              error: e instanceof Error ? e.message : String(e),
            }),
          );
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
          messageApi.success(t("settings.agent_mode.reset_success"));
        } catch (e) {
          messageApi.error(
            t("settings.agent_mode.reset_failed", {
              error: e instanceof Error ? e.message : String(e),
            }),
          );
        }
      },
    });
  };

  const handleSave = async () => {
    const values = await form.validateFields();
    const kind = (values.kind || "custom") as AgentMode["kind"];
    try {
      if (editingMode) {
        await store.saveMode({
          ...editingMode,
          ...values,
          kind,
          tools: editingMode.tools,
          presentation_rank: editingMode.presentation_rank,
          is_builtin: editingMode.is_builtin,
        });
      } else {
        await store.saveMode({
          ...values,
          kind,
          tools: [],
          presentation_rank: 99,
          is_builtin: false,
        });
      }
      messageApi.success(
        editingMode
          ? t("settings.agent_mode.saved")
          : t("settings.agent_mode.created"),
      );
      setDrawerOpen(false);
      setEditingMode(null);
    } catch (e) {
      messageApi.error(
        t("settings.agent_mode.save_failed", {
          error: e instanceof Error ? e.message : String(e),
        }),
      );
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
    if (!isBuiltinEditing) return false;
    return schema.key === "name" || schema.key === "kind";
  };

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
      </div>
      <Space size={4}>
        <Button
          type="text"
          size="small"
          icon={<EditOutlined />}
          onClick={() => handleEdit(mode)}
        />
        {mode.is_builtin && (
          <Button
            type="text"
            size="small"
            icon={<UndoOutlined />}
            onClick={() => handleReset(mode)}
          />
        )}
        {!mode.is_builtin && (
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

  const sections: EntityListSectionConfig<AgentMode>[] = [
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
      {messageContextHolder}
      {modalContextHolder}
      <EntityListPageShell<AgentMode>
        title={t("settings.agent_mode.title")}
        items={store.modes}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        addLabel={t("settings.agent_mode.add")}
        onAdd={handleAdd}
        emptyDescription={t("settings.agent_mode.empty")}
        sections={sections}
      >
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
              if (
                "reasoning_enabled" in changed &&
                !changed.reasoning_enabled
              ) {
                form.setFieldValue("reasoning_effort", null);
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
                />
              </Form.Item>
            ))}
          </Form>
        </Drawer>
      </EntityListPageShell>
    </>
  );
};
