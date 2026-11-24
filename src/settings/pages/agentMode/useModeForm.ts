import { useState } from "react";
import { App, Form } from "antd";
import { useTranslation } from "react-i18next";

import {
  useAgentModeStore,
  type AgentMode,
} from "@/stores/agent/agentModeStore";
import type { ConversationToolConfig } from "@/chat/core/chatTypes";
import {
  DEFAULT_TOOL_CONFIG,
  stripCompanionIds,
  validateApprovalRules,
} from "@/chat/core/toolConfig";
import { approvalRuleErrorText } from "@/chat/tool/ApprovalRulesEditor";
import { useSettingMessage } from "../../shared/useSettingMessage";
import { loadToolDescriptors } from "../../shared/sections/ToolAssemblySection";
import type { FieldSchema } from "../../core/types/fieldSchema";

import { DEFAULT_FORM_DATA, type ModeFormData } from "./fieldSchemas";

/**
 * 模式抽屉的表单编排：打开/关闭、编辑对象、工具装配草稿与保存。
 * 列表渲染与搜索留在页面组件，本 Hook 只管抽屉生命周期。
 */
export function useModeForm() {
  const { t } = useTranslation();
  const store = useAgentModeStore();
  const msg = useSettingMessage();
  const { modal } = App.useApp();
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [editingMode, setEditingMode] = useState<AgentMode | null>(null);
  const [form] = Form.useForm<ModeFormData>();
  const [toolConfigDraft, setToolConfigDraft] =
    useState<ConversationToolConfig>(DEFAULT_TOOL_CONFIG);
  const formValues = Form.useWatch([], form);

  const handleAdd = () => {
    setEditingMode(null);
    form.resetFields();
    form.setFieldsValue(DEFAULT_FORM_DATA);
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
        ? stripCompanionIds(toolConfigDraft, await loadToolDescriptors())
        : (editingMode?.toolConfig ?? DEFAULT_TOOL_CONFIG);
    if (kind === "agent") {
      const ruleError = validateApprovalRules(toolConfig.approvalRules);
      if (ruleError) {
        msg.error(
          "settings.agent_mode.save_failed",
          approvalRuleErrorText(ruleError),
        );
        return;
      }
    }
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

  const closeDrawer = () => {
    setDrawerOpen(false);
    setEditingMode(null);
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

  return {
    modes: store.modes,
    loadModes: store.loadModes,
    form,
    formValues,
    drawerOpen,
    editingMode,
    toolConfigDraft,
    setToolConfigDraft,
    showToolAssembly: formKind === "agent",
    handleAdd,
    handleEdit,
    handleDelete,
    handleReset,
    handleSave,
    closeDrawer,
    shouldShowField,
    isFieldDisabled,
  };
}
