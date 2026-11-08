import React, { useEffect, useMemo, useState } from "react";
import { Checkbox, Select, Spin, Tag, Typography } from "antd";
import { useTranslation } from "react-i18next";
import { chatService } from "@/services/chatService";
import { createLogger } from "@/services/logger";
import type {
  ApprovalLevelDto,
  ToolDescriptorDto,
  ToolTierDto,
} from "@/types/protocol.contract";
import type { ConversationToolConfig } from "@/chat/core/chatTypes";
import {
  APPROVAL_LEVELS,
  TIER_RANK,
  TOOL_TIERS,
  groupToolsByTier,
  selectedIdsInTier,
  setTierSelection,
} from "@/chat/core/toolConfig";
import css from "./ToolAssemblySection.module.css";

const log = createLogger("ToolAssemblySection");

let toolsPromise: Promise<ToolDescriptorDto[]> | null = null;

function loadToolDescriptors(): Promise<ToolDescriptorDto[]> {
  if (!toolsPromise) {
    toolsPromise = chatService.listAvailableTools().catch((e) => {
      toolsPromise = null;
      throw e;
    });
  }
  return toolsPromise;
}

interface ToolAssemblySectionProps {
  config: ConversationToolConfig;
  onChange: (next: ConversationToolConfig) => void;
  disabled?: boolean;
}

/**
 * 模式级工具装配（I38d §4.4 / §4.7）：写入 `agent_modes.tool_config_json`，
 * 只影响之后新建的会话。与会话级 `ToolConfigPanel` 不复用视图——
 * 这里是抽屉内的纵向表单，可用高度充足，工具项带说明文字。
 */
export const ToolAssemblySection: React.FC<ToolAssemblySectionProps> = ({
  config,
  onChange,
  disabled = false,
}) => {
  const { t } = useTranslation();
  const [tools, setTools] = useState<ToolDescriptorDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void loadToolDescriptors()
      .then((list) => {
        if (cancelled) return;
        setTools(list);
        setFailed(false);
      })
      .catch((e) => {
        log.error("listAvailableTools failed", e);
        if (!cancelled) setFailed(true);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const grouped = useMemo(() => groupToolsByTier(tools), [tools]);
  const ceiling = TIER_RANK[config.maxToolTier];

  const setTier = (maxToolTier: ToolTierDto) =>
    onChange({ ...config, maxToolTier });

  const setLevel = (approvalLevel: ApprovalLevelDto) =>
    onChange({ ...config, approvalLevel });

  return (
    <div className={css.root}>
      <div className={css.header}>
        <Typography.Text strong>
          {t("settings.agent_mode.tool_assembly.title")}
        </Typography.Text>
        <Typography.Text type="secondary" className={css.count}>
          {t("settings.agent_mode.tool_assembly.enabled_count", {
            count: config.enabledTools.length,
          })}
        </Typography.Text>
      </div>

      <div className={css.field}>
        <span className={css.label}>
          {t("settings.agent_mode.tool_assembly.tier_label")}
        </span>
        <Select
          value={config.maxToolTier}
          onChange={setTier}
          disabled={disabled}
          options={TOOL_TIERS.map((tier) => ({
            value: tier,
            label: t(`settings.agent_mode.field.max_tool_tier.options.${tier}`),
          }))}
          className={css.select}
        />
      </div>

      <div className={css.field}>
        <span className={css.label}>
          {t("settings.agent_mode.tool_assembly.level_label")}
        </span>
        <Select
          value={config.approvalLevel}
          onChange={setLevel}
          disabled={disabled}
          options={APPROVAL_LEVELS.map((level) => ({
            value: level,
            label: t(
              `settings.agent_mode.field.approval_level.options.${level}`,
            ),
          }))}
          className={css.select}
        />
      </div>

      {loading ? (
        <div className={css.loading}>
          <Spin size="small" />
        </div>
      ) : failed ? (
        <Typography.Text type="secondary" className={css.empty}>
          {t("settings.agent_mode.tool_assembly.load_failed")}
        </Typography.Text>
      ) : (
        <div className={css.groups}>
          {TOOL_TIERS.map((tier) => {
            const items = grouped.get(tier) ?? [];
            const beyondCeiling = TIER_RANK[tier] > ceiling;
            const selected = selectedIdsInTier(config, tools, tier);
            return (
              <div
                key={tier}
                className={`${css.group} ${beyondCeiling ? css.groupDimmed : ""}`}
              >
                <div className={css.groupHeader}>
                  <span className={css.groupTitle}>
                    {t(
                      `settings.agent_mode.field.max_tool_tier.options.${tier}`,
                    )}
                  </span>
                  {beyondCeiling && (
                    <Tag color="warning" className={css.groupTag}>
                      {t("settings.agent_mode.tool_assembly.out_of_tier")}
                    </Tag>
                  )}
                </div>
                {items.length === 0 ? (
                  <Typography.Text type="secondary" className={css.empty}>
                    {t("settings.agent_mode.tool_assembly.group_empty")}
                  </Typography.Text>
                ) : (
                  <Checkbox.Group
                    value={selected}
                    disabled={disabled || beyondCeiling}
                    onChange={(values) =>
                      onChange(
                        setTierSelection(
                          config,
                          tools,
                          tier,
                          values.map(String),
                        ),
                      )
                    }
                    className={css.toolList}
                  >
                    {items.map((tool) => (
                      <Checkbox
                        key={tool.id}
                        value={tool.id}
                        className={css.tool}
                      >
                        <span className={css.toolName}>{tool.name}</span>
                        <span className={css.toolDesc}>
                          {tool.description || tool.name}
                        </span>
                      </Checkbox>
                    ))}
                  </Checkbox.Group>
                )}
              </div>
            );
          })}
        </div>
      )}

      <Typography.Text type="secondary" className={css.hint}>
        {t("settings.agent_mode.tool_assembly.hint")}
      </Typography.Text>
    </div>
  );
};
