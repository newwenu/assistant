import { useEffect, useRef, useState } from "react";
import { Button, Drawer, Form, Space, Tag, Typography } from "antd";
import { DeleteOutlined, EditOutlined, UndoOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";

import { translateKey } from "@/i18n";
import type { AgentMode } from "@/stores/agent/agentModeStore";
import { SettingField } from "../../shared/renderers/SettingField";
import { SettingsPageShell } from "../../shared/layout/SettingsPageShell";
import {
  EntityListContent,
  ListHeaderActions,
  type SectionConfig,
} from "../../shared/EntityListContent";
import {
  getAgentKindColor,
  getAgentKindLabelKey,
} from "../../core/agentKind/agentModeRegistry";
import { PermissionAuditSection } from "../../shared/sections/PermissionAuditSection";
import { PermissionGrantsSection } from "../../shared/sections/PermissionGrantsSection";
import { ProjectRulesSection } from "../../shared/sections/ProjectRulesSection";
import { ToolAssemblySection } from "../../shared/sections/ToolAssemblySection";

import { DEFAULT_FORM_DATA, useFieldSchemas } from "./fieldSchemas";
import { useModeForm } from "./useModeForm";

export const AgentModeSettings: React.FC = () => {
  const { t } = useTranslation();
  const [searchQuery, setSearchQuery] = useState("");
  const {
    modes,
    loadModes,
    form,
    formValues,
    drawerOpen,
    editingMode,
    toolConfigDraft,
    setToolConfigDraft,
    showToolAssembly,
    handleAdd,
    handleEdit,
    handleDelete,
    handleReset,
    handleSave,
    closeDrawer,
    shouldShowField,
    isFieldDisabled,
  } = useModeForm();
  const fieldSchemas = useFieldSchemas();

  useEffect(() => {
    void loadModes();
  }, [loadModes]);

  /**
   * 「去设置编辑」的落点（I70 §7.3）：会话面板只读展示规则，
   * 点击后带 `?mode=<id>&focus=rules` 打开本页，这里据此自动打开对应模式的抽屉。
   *
   * 用 ref 记录已处理过的 id：`handleEdit` 每次渲染都是新函数，
   * 仅靠依赖去重会让抽屉在关闭后被重新打开。
   */
  const [searchParams, setSearchParams] = useSearchParams();
  const focusModeId = searchParams.get("mode");
  const handledFocusRef = useRef<string | null>(null);
  /**
   * 滚动意图必须存成 state：清掉 query 后参数就没了，
   * 直接读 `searchParams` 会让滚动 effect 在抽屉真正打开前提前失效。
   */
  const [focusRules, setFocusRules] = useState(false);

  useEffect(() => {
    if (!focusModeId || handledFocusRef.current === focusModeId) return;
    const mode = modes.find((m) => m.id === focusModeId);
    if (!mode) return;
    handledFocusRef.current = focusModeId;
    if (searchParams.get("focus") === "rules") setFocusRules(true);
    handleEdit(mode);
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        next.delete("mode");
        next.delete("focus");
        return next;
      },
      { replace: true },
    );
  }, [focusModeId, modes, handleEdit, searchParams, setSearchParams]);

  /** 抽屉打开后再滚动到规则编辑器：渲染顺序决定它此刻才存在。 */
  useEffect(() => {
    if (!focusRules || !drawerOpen) return;
    const timer = window.setTimeout(() => {
      const el = document.querySelector<HTMLElement>("[data-approval-rules]");
      if (!el) return;
      el.scrollIntoView({ behavior: "smooth", block: "center" });
      el.classList.add("setting-focus-highlight");
      window.setTimeout(
        () => el.classList.remove("setting-focus-highlight"),
        2000,
      );
      setFocusRules(false);
    }, 200);
    return () => window.clearTimeout(timer);
  }, [focusRules, drawerOpen, editingMode?.id]);

  const builtinModes = modes.filter((m) => m.isBuiltin);
  const customModes = modes.filter((m) => !m.isBuiltin);

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
        items={modes}
        sections={sections}
        emptyDescription={t("settings.agent_mode.empty")}
      />
      <PermissionGrantsSection />
      <ProjectRulesSection />
      <PermissionAuditSection />
      <Drawer
        title={
          editingMode
            ? t("settings.agent_mode.edit")
            : t("settings.agent_mode.add")
        }
        open={drawerOpen}
        onClose={closeDrawer}
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
          initialValues={DEFAULT_FORM_DATA}
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
  );
};
