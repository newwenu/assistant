import React, { useEffect, useMemo, useState } from "react";
import { Checkbox, Select, Spin, Tag, Typography } from "antd";
import { useTranslation } from "react-i18next";
import { chatService } from "@/services/chat/chatService";
import { createLogger } from "@/services/system/logger";
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
  countEnabledSelectableTools,
  groupToolsByTier,
  isMetaTool,
  selectedIdsInTier,
  setTierSelection,
  stripCompanionIds,
} from "@/chat/core/toolConfig";
import css from "./ToolAssemblySection.module.css";
import { ApprovalRulesEditor } from "@/chat/tool/ApprovalRulesEditor";

const log = createLogger("ToolAssemblySection");

let toolsPromise: Promise<ToolDescriptorDto[]> | null = null;

/**
 * 工具清单加载（进程内只请求一次）。
 *
 * 导出是为了让 `useModeForm.handleSave` 也能拿到清单做同样的清理，
 * 保存路径不依赖用户是否点过工具区。
 */
export function loadToolDescriptors(): Promise<ToolDescriptorDto[]> {
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
 * 只影响之后新建的会话。工具清单视图与会话级 `ToolConfigPanel` 不复用——
 * 这里是抽屉内的纵向表单，可用高度充足，工具项带说明文字；
 * 但审批规则编辑器复用同一个 `ApprovalRulesEditor`（I70 §7.3），保证两处语义一致。
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

  /**
   * 唯一写回口：先剔除名单里的伴随 id（I58 §3.6）。
   *
   * 历史配置可能残留这些 id（旧面板曾把伴随项渲染成勾选项），
   * 这里在写回前静默修掉，用户下一次保存即完成清理。
   */
  const emit = (next: ConversationToolConfig) =>
    onChange(stripCompanionIds(next, tools));

  const setTier = (maxToolTier: ToolTierDto) =>
    emit({ ...config, maxToolTier });

  const setLevel = (approvalLevel: ApprovalLevelDto) =>
    emit({ ...config, approvalLevel });

  return (
    <div className={css.root}>
      <div className={css.header}>
        <Typography.Text strong>
          {t("settings.agent_mode.tool_assembly.title")}
        </Typography.Text>
        <Typography.Text type="secondary" className={css.count}>
          {t("settings.agent_mode.tool_assembly.enabled_count", {
            count: countEnabledSelectableTools(config, tools),
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
                  <div className={css.toolList}>
                    {items.map((tool) => {
                      const baseId = tool.companionOf;
                      if (baseId !== null) {
                        const baseEnabled =
                          config.enabledTools.includes(baseId);
                        const baseName =
                          tools.find((item) => item.id === baseId)?.name ??
                          baseId;
                        return (
                          <div
                            key={tool.id}
                            className={`${css.tool} ${css.companionRow}`}
                          >
                            <span
                              className={`${css.statusDot} ${
                                baseEnabled ? "" : css.statusDotIdle
                              }`}
                            />
                            <div className={css.toolText}>
                              <span className={css.toolName}>{tool.name}</span>
                              <span className={css.toolDesc}>
                                {baseEnabled
                                  ? t(
                                      "settings.agent_mode.tool_assembly.companion_of",
                                      { base: baseName },
                                    )
                                  : t(
                                      "settings.agent_mode.tool_assembly.companion_blocked",
                                      { base: baseName },
                                    )}
                              </span>
                            </div>
                          </div>
                        );
                      }
                      if (isMetaTool(tool)) {
                        return (
                          <Checkbox
                            key={tool.id}
                            checked={selected.includes(tool.id)}
                            disabled={disabled || beyondCeiling}
                            onChange={(event) => {
                              const next = event.target.checked
                                ? [...selected, tool.id]
                                : selected.filter((id) => id !== tool.id);
                              emit(setTierSelection(config, tools, tier, next));
                            }}
                            className={css.tool}
                          >
                            <span className={css.toolName}>
                              {tool.name}
                              <Tag className={css.metaBadge}>
                                {t(
                                  "settings.agent_mode.tool_assembly.meta_badge",
                                )}
                              </Tag>
                            </span>
                            <span className={css.toolDesc}>
                              {t("settings.agent_mode.tool_assembly.meta_hint")}
                            </span>
                          </Checkbox>
                        );
                      }
                      return (
                        <Checkbox
                          key={tool.id}
                          checked={selected.includes(tool.id)}
                          disabled={disabled || beyondCeiling}
                          onChange={(event) => {
                            const next = event.target.checked
                              ? [...selected, tool.id]
                              : selected.filter((id) => id !== tool.id);
                            emit(setTierSelection(config, tools, tier, next));
                          }}
                          className={css.tool}
                        >
                          <span className={css.toolName}>{tool.name}</span>
                          <span className={css.toolDesc}>
                            {tool.description || tool.name}
                          </span>
                        </Checkbox>
                      );
                    })}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      <div data-approval-rules>
        <ApprovalRulesEditor
          rules={config.approvalRules}
          onChange={(approvalRules) => emit({ ...config, approvalRules })}
          disabled={disabled}
        />
      </div>

      <Typography.Text type="secondary" className={css.hint}>
        {t("settings.agent_mode.tool_assembly.hint")}
      </Typography.Text>
    </div>
  );
};
