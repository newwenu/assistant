import React, { useEffect, useState } from "react";
import {
  Button,
  Typography,
  Modal,
  Empty,
  Drawer,
  Form,
  Input,
  Select,
  Slider,
  InputNumber,
  Tag,
  Space,
} from "antd";
import {
  PlusOutlined,
  EditOutlined,
  DeleteOutlined,
  UndoOutlined,
} from "@ant-design/icons";
import { useAgentModeStore, type AgentMode } from "../stores/agentModeStore";

const KIND_OPTIONS = [
  { value: "default", label: "默认" },
  { value: "quick", label: "快速" },
  { value: "deep", label: "深度" },
  { value: "custom", label: "自定义" },
];

const KIND_TAG_COLORS: Record<string, string> = {
  default: "blue",
  quick: "green",
  deep: "purple",
  custom: "orange",
};

const KIND_LABELS: Record<string, string> = {
  default: "默认",
  quick: "快速",
  deep: "深度",
  custom: "自定义",
};

interface ModeFormData {
  name: string;
  description: string;
  kind: string;
  prompt: string;
  model: string | null;
  temperature: number | null;
  max_tokens: number | null;
}

const defaultFormData: ModeFormData = {
  name: "",
  description: "",
  kind: "custom",
  prompt: "",
  model: null,
  temperature: null,
  max_tokens: null,
};

export const AgentModeManager: React.FC = () => {
  const store = useAgentModeStore();
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [editingMode, setEditingMode] = useState<AgentMode | null>(null);
  const [form] = Form.useForm<ModeFormData>();
  const [searchQuery, setSearchQuery] = useState("");

  useEffect(() => {
    store.loadModes();
  }, [store.loadModes]);

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
    });
    setDrawerOpen(true);
  };

  const handleDelete = (id: string) => {
    Modal.confirm({
      title: "确认删除此模式？",
      content: "删除后不可恢复",
      onOk: () => store.deleteMode(id),
    });
  };

  const handleReset = (mode: AgentMode) => {
    Modal.confirm({
      title: "恢复默认参数？",
      content: `将"${mode.name}"的 prompt、temperature、max_tokens 等恢复为内置默认值`,
      onOk: () => store.resetBuiltinMode(mode.id),
    });
  };

  const handleSave = async () => {
    const values = await form.validateFields();
    const kind = (values.kind || "custom") as AgentMode["kind"];
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
    setDrawerOpen(false);
    setEditingMode(null);
  };

  const renderModeRow = (mode: AgentMode) => (
    <div
      key={mode.id}
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "8px 12px",
        borderBottom: "1px solid var(--color-border-secondary)",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          flex: 1,
          minWidth: 0,
        }}
      >
        <Typography.Text strong style={{ flexShrink: 0 }}>
          {mode.name}
        </Typography.Text>
        <Tag
          color={KIND_TAG_COLORS[mode.kind] || "default"}
          style={{ flexShrink: 0 }}
        >
          {KIND_LABELS[mode.kind] || mode.kind}
        </Tag>
        {mode.description && (
          <Typography.Text
            type="secondary"
            ellipsis
            style={{ flex: 1, minWidth: 0 }}
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

  return (
    <div className="agent-mode-manager">
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: 16,
        }}
      >
        <Typography.Title level={5} style={{ margin: 0 }}>
          Agent 模式管理
        </Typography.Title>
        <Space>
          <Input
            placeholder="搜索模式"
            size="small"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            allowClear
            style={{ width: 160 }}
          />
          <Button type="primary" icon={<PlusOutlined />} onClick={handleAdd}>
            新建模式
          </Button>
        </Space>
      </div>

      {store.modes.length === 0 ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description="暂无 Agent 模式"
        />
      ) : (
        <>
          {filteredBuiltin.length > 0 && (
            <div style={{ marginBottom: 16 }}>
              <Typography.Text
                type="secondary"
                style={{ fontSize: 12, marginBottom: 4, display: "block" }}
              >
                内置模式
              </Typography.Text>
              {filteredBuiltin.map(renderModeRow)}
            </div>
          )}
          {filteredCustom.length > 0 && (
            <div>
              <Typography.Text
                type="secondary"
                style={{ fontSize: 12, marginBottom: 4, display: "block" }}
              >
                自定义模式
              </Typography.Text>
              {filteredCustom.map(renderModeRow)}
            </div>
          )}
        </>
      )}

      <Drawer
        title={editingMode ? "编辑模式" : "新建模式"}
        open={drawerOpen}
        onClose={() => {
          setDrawerOpen(false);
          setEditingMode(null);
        }}
        width={400}
        extra={
          <Button type="primary" onClick={handleSave}>
            保存
          </Button>
        }
      >
        <Form form={form} layout="vertical" initialValues={defaultFormData}>
          <Form.Item
            name="name"
            label="名称"
            rules={[{ required: true, message: "请输入模式名称" }]}
          >
            <Input disabled={editingMode?.is_builtin} placeholder="模式名称" />
          </Form.Item>
          <Form.Item name="description" label="描述">
            <Input.TextArea rows={2} placeholder="模式描述" />
          </Form.Item>
          <Form.Item name="kind" label="类型">
            <Select options={KIND_OPTIONS} disabled={editingMode?.is_builtin} />
          </Form.Item>
          <Form.Item name="prompt" label="System Prompt">
            <Input.TextArea rows={4} placeholder="留空则不注入 system prompt" />
          </Form.Item>
          <Form.Item name="model" label="指定模型">
            <Select allowClear placeholder="跟随对话模型" options={[]} />
          </Form.Item>
          <Form.Item name="temperature" label="Temperature">
            <Slider
              min={0}
              max={2}
              step={0.1}
              marks={{ 0: "0", 0.3: "0.3", 0.7: "0.7", 1: "1", 2: "2" }}
              value={form.getFieldValue("temperature") ?? 0.7}
              onChange={(v) => form.setFieldValue("temperature", v)}
            />
          </Form.Item>
          <Form.Item name="max_tokens" label="Max Tokens">
            <InputNumber
              min={256}
              max={128_000}
              step={256}
              placeholder="默认 4096"
              style={{ width: "100%" }}
            />
          </Form.Item>
        </Form>
      </Drawer>
    </div>
  );
};
