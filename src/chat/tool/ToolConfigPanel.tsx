import React, { useMemo } from "react";
import { Button, Checkbox, Select, Spin, Tag, Tooltip, Typography } from "antd";
import { useTranslation } from "react-i18next";
import type {
  ApprovalLevelDto,
  ToolDescriptorDto,
  ToolTierDto,
} from "@/types/protocol.contract";
import type { ConversationToolConfig } from "../core/chatTypes";
import { APPROVAL_LEVELS, TOOL_TIERS } from "../core/toolConfig";
import css from "./ToolConfigPanel.module.css";

interface ToolConfigPanelProps {
  config: ConversationToolConfig;
  tools: ToolDescriptorDto[];
  loading: boolean;
  /** 当前会话模式可写回模式默认值时才有值。 */
  canSetDefault: boolean;
  /** 写回模式默认值的请求进行中，按钮禁用。 */
  settingDefault: boolean;
  onChange: (next: ConversationToolConfig) => void;
  onSetDefault: () => void;
  onClose: () => void;
}

const TIER_RANK: Record<ToolTierDto, number> = {
  sandbox: 0,
  os: 1,
  external: 2,
};

/**
 * 会话工具装配面板（I38d §4.7）：
 * 层级上限 + 审批级别 + 按 tier 分组的工具多选。
 *
 * 面板只做「编辑 → 上报」，不自己持久化：唯一写入口是
 * `ConversationView` 的 `setConversationToolConfig`，避免多处写库分叉。
 */
export const ToolConfigPanel: React.FC<ToolConfigPanelProps> = ({
  config,
  tools,
  loading,
  canSetDefault,
  settingDefault,
  onChange,
  onSetDefault,
  onClose,
}) => {
  const { t } = useTranslation();
  const ceiling = TIER_RANK[config.maxToolTier];

  const grouped = useMemo(() => {
    const map = new Map<ToolTierDto, ToolDescriptorDto[]>();
    for (const tier of TOOL_TIERS) map.set(tier, []);
    for (const tool of tools) map.get(tool.tier)?.push(tool);
    return map;
  }, [tools]);

  const enabledCount = config.enabledTools.length;

  const setTier = (maxToolTier: ToolTierDto) =>
    onChange({ ...config, maxToolTier });

  const setLevel = (approvalLevel: ApprovalLevelDto) =>
    onChange({ ...config, approvalLevel });

  const setTierSelection = (tier: ToolTierDto, ids: string[]) => {
    const others = config.enabledTools.filter(
      (id) => !tools.some((tool) => tool.id === id && tool.tier === tier),
    );
    onChange({ ...config, enabledTools: [...others, ...ids] });
  };

  return (
    <div className={css.root}>
      <div className={css.header}>
        <Typography.Text strong>{t("chat.tool.panel.title")}</Typography.Text>
        <Typography.Text type="secondary" className={css.count}>
          {t("chat.tool.panel.enabled_count", { count: enabledCount })}
        </Typography.Text>
        <Button size="small" type="text" onClick={onClose}>
          {t("chat.tool.panel.done")}
        </Button>
      </div>

      <div className={css.controls}>
        <label className={css.field}>
          <span className={css.label}>{t("chat.tool.panel.tier_label")}</span>
          <Select
            size="small"
            value={config.maxToolTier}
            onChange={setTier}
            options={TOOL_TIERS.map((tier) => ({
              value: tier,
              label: t(`chat.tool.approval.tier.${tier}`),
            }))}
            className={css.select}
          />
        </label>
        <label className={css.field}>
          <span className={css.label}>{t("chat.tool.panel.level_label")}</span>
          <Select
            size="small"
            value={config.approvalLevel}
            onChange={setLevel}
            options={APPROVAL_LEVELS.map((level) => ({
              value: level,
              label: t(`chat.tool.approval.level.${level}`),
            }))}
            className={css.select}
          />
        </label>
        <Tooltip title={t("chat.tool.panel.set_default_hint")}>
          <Button
            size="small"
            disabled={!canSetDefault || settingDefault}
            loading={settingDefault}
            onClick={onSetDefault}
          >
            {t("chat.tool.panel.set_default")}
          </Button>
        </Tooltip>
      </div>

      {loading ? (
        <div className={css.loading}>
          <Spin size="small" />
        </div>
      ) : (
        <div className={css.groups}>
          {TOOL_TIERS.map((tier) => {
            const items = grouped.get(tier) ?? [];
            const beyondCeiling = TIER_RANK[tier] > ceiling;
            const selected = config.enabledTools.filter((id) =>
              items.some((tool) => tool.id === id),
            );
            return (
              <div
                key={tier}
                className={`${css.group} ${beyondCeiling ? css.groupDimmed : ""}`}
              >
                <div className={css.groupHeader}>
                  <span className={css.groupTitle}>
                    {t(`chat.tool.approval.tier.${tier}`)}
                  </span>
                  {beyondCeiling && (
                    <Tag color="warning" className={css.groupTag}>
                      {t("chat.tool.panel.out_of_tier")}
                    </Tag>
                  )}
                </div>
                {items.length === 0 ? (
                  <Typography.Text type="secondary" className={css.empty}>
                    {t("chat.tool.panel.group_empty")}
                  </Typography.Text>
                ) : (
                  <Checkbox.Group
                    value={selected}
                    disabled={beyondCeiling}
                    onChange={(values) =>
                      setTierSelection(tier, values.map(String))
                    }
                    className={css.checkboxGroup}
                  >
                    {items.map((tool) => (
                      <Tooltip
                        key={tool.id}
                        title={tool.description || tool.name}
                        placement="top"
                      >
                        <Checkbox value={tool.id} className={css.checkbox}>
                          {tool.name}
                        </Checkbox>
                      </Tooltip>
                    ))}
                  </Checkbox.Group>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
