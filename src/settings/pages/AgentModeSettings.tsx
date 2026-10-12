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
import { useAgentModeStore, type AgentMode } from "@/stores/agentModeStore";
import type { FieldSchema } from "../core/fieldSchema";
import { SettingField } from "../shared/SettingField";
import {
  EntityListPageShell,
  type EntityListSectionConfig,
} from "../shared/EntityListPageShell";
import {
  getAllAgentKinds,
  getAgentKindLabel,
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
    () => getAllAgentKinds().map((k) => ({ value: k.id, label: k.label })),
    [],
  );

  const fieldSchemas = useMemo<FieldSchema[]>(
    () => [
      { key: "name", type: "string", label: "名称", placeholder: "模式名称" },
      {
        key: "description",
        type: "textarea",
        label: "描述",
        rows: 2,
        placeholder: "模式描述",
      },
      { key: "kind", type: "select", label: "类型", options: kindOptions },
      {
        key: "prompt",
        type: "textarea",
        label: "System Prompt",
        rows: 4,
        placeholder: "留空则不注入 system prompt",
      },
      {
        key: "model",
        type: "select",
        label: "指定模型",
        options: [],
        allowClear: true,
        placeholder: "跟随对话模型",
      },
      {
        key: "temperature",
        type: "slider",
        label: "Temperature",
        min: 0,
        max: 2,
        step: 0.1,
        marks: { 0: "0", 0.3: "0.3", 0.7: "0.7", 1: "1", 2: "2" },
      },
      {
        key: "max_tokens",
        type: "number",
        label: "Max Tokens",
        min: 256,
        max: 128_000,
        step: 256,
        placeholder: "默认 4096",
      },
      { key: "reasoning_enabled", type: "boolean", label: "Reasoning" },
      {
        key: "reasoning_effort",
        type: "select",
        label: "Reasoning Effort",
        options: REASONING_EFFORT_OPTIONS,
        allowClear: true,
        placeholder: "选择推理强度",
        visible: (fv) => fv.reasoning_enabled === true,
      },
    ],
    [kindOptions],
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
      title: "确认删除此模式？",
      content: "删除后不可恢复",
      onOk: async () => {
        try {
          await store.deleteMode(id);
          messageApi.success("已删除");
        } catch (e) {
          messageApi.error(
            `删除失败: ${e instanceof Error ? e.message : String(e)}`,
          );
        }
      },
    });
  };

  const handleReset = (mode: AgentMode) => {
    modal.confirm({
      title: "恢复默认参数？",
      content: `将"${mode.name}"的 prompt、temperature、max_tokens 等恢复为内置默认值`,
      onOk: async () => {
        try {
          await store.resetBuiltinMode(mode.id);
          messageApi.success("已恢复默认");
        } catch (e) {
          messageApi.error(
            `恢复失败: ${e instanceof Error ? e.message : String(e)}`,
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
      messageApi.success(editingMode ? "已保存修改" : "已创建模式");
      setDrawerOpen(false);
      setEditingMode(null);
    } catch (e) {
      messageApi.error(
        `保存失败: ${e instanceof Error ? e.message : String(e)}`,
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
          {getAgentKindLabel(mode.kind)}
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
    { label: "内置模式", items: filteredBuiltin, renderRow: renderModeRow },
    { label: "自定义模式", items: filteredCustom, renderRow: renderModeRow },
  ];

  return (
    <>
      {messageContextHolder}
      {modalContextHolder}
      <EntityListPageShell<AgentMode>
        title="Agent 模式管理"
        items={store.modes}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        addLabel="新建模式"
        onAdd={handleAdd}
        emptyDescription="暂无 Agent 模式"
        sections={sections}
      >
        <Drawer
          title={editingMode ? "编辑模式" : "新建模式"}
          open={drawerOpen}
          onClose={() => {
            setDrawerOpen(false);
            setEditingMode(null);
          }}
          size="large"
          extra={
            <Button type="primary" onClick={() => void handleSave()}>
              保存
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
                tooltip={schema.description || undefined}
                extra={schema.extra}
                rules={
                  schema.key === "name"
                    ? [{ required: true, message: "请输入模式名称" }]
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
