import React, { useCallback, useMemo, useState } from "react";
import { Button, Checkbox, Select, Spin, Tag, Tooltip, Typography } from "antd";
import { useTranslation } from "react-i18next";
import { useOrbOverlayClose } from "@/orb/orbOverlays";
import type {
  ApprovalLevelDto,
  ToolDescriptorDto,
  ToolTierDto,
} from "@/types/protocol.contract";
import type { I18nPath } from "@/i18n";
import type { ConversationToolConfig } from "../core/chatTypes";
import {
  APPROVAL_LEVELS,
  TIER_RANK,
  TOOL_TIERS,
  countEnabledSelectableTools,
  groupToolsByTier,
  groupToolUnits,
  isMetaTool,
  selectedIdsInTier,
  setTierSelection,
} from "../core/toolConfig";
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
}

/**
 * 面板短提示：`ToolDescriptorDto.description` 是给模型看的提示词（`ask_user` 长达数行），
 * 悬停时对用户无意义。这里按工具 id 换成一句话说明；未收录的工具回落到后端描述。
 *
 * 伴随条目与元工具**不**回落到 `description`（那是模型的 `full` 档文案，I58 §6）。
 * 伴随条目的悬停提示由「一句用途 + 从属状态」组成，面板里只留工具名，
 * 否则小窗口会被这些说明文字撑满（I58 §3.6）。
 */
const TOOL_DESC_KEYS: Record<string, I18nPath> = {
  read: "chat.tool.panel.tool_desc.read",
  ls: "chat.tool.panel.tool_desc.ls",
  glob: "chat.tool.panel.tool_desc.glob",
  grep: "chat.tool.panel.tool_desc.grep",
  write: "chat.tool.panel.tool_desc.write",
  edit: "chat.tool.panel.tool_desc.edit",
  mkdir: "chat.tool.panel.tool_desc.mkdir",
  move: "chat.tool.panel.tool_desc.move",
  delete: "chat.tool.panel.tool_desc.delete",
  ask_user: "chat.tool.panel.tool_desc.ask_user",
  super_read: "chat.tool.panel.tool_desc.super_read",
  super_edit: "chat.tool.panel.tool_desc.super_edit",
  request_tool: "chat.tool.panel.tool_desc.request_tool",
};

/**
 * 审批级别下拉项：`label` 是档位名，`hint` 说明该档位下哪些操作会弹审批。
 * 下拉列表用 `optionRender` 渲染两行，收起态只显示 `label`。
 */
type ApprovalLevelOption = {
  value: ApprovalLevelDto;
  label: string;
  hint: string;
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
}) => {
  const { t } = useTranslation();
  const ceiling = TIER_RANK[config.maxToolTier];

  /**
   * 两个下拉都受控：orb 收起时由 `collapseOrbPanel()` 广播统一归零，
   * 避免下拉弹层留在收起后的小球窗口上。
   */
  const [tierOpen, setTierOpen] = useState(false);
  const [levelOpen, setLevelOpen] = useState(false);
  const closeDropdowns = useCallback(() => {
    setTierOpen(false);
    setLevelOpen(false);
  }, []);
  useOrbOverlayClose(closeDropdowns);

  const grouped = useMemo(() => groupToolsByTier(tools), [tools]);

  const levelOptions = useMemo<ApprovalLevelOption[]>(
    () =>
      APPROVAL_LEVELS.map((level) => ({
        value: level,
        label: t(`chat.tool.approval.level.${level}`),
        hint: t(`chat.tool.approval.level_hint.${level}`),
      })),
    [t],
  );

  const enabledCount = countEnabledSelectableTools(config, tools);

  const setTier = (maxToolTier: ToolTierDto) =>
    onChange({ ...config, maxToolTier });

  const setLevel = (approvalLevel: ApprovalLevelDto) =>
    onChange({ ...config, approvalLevel });

  /**
   * 渲染单个条目：普通工具与元工具是可勾选项，伴随条目只显示名称。
   *
   * 伴随条目必须和基工具渲染在同一个 `.toolUnit` 内（见下方 tier 循环），
   * 否则 `flex-wrap` 会在两者之间断行，它看起来就像一个无关的独立工具（I58 §3.6）。
   */
  const renderEntry = (
    tool: ToolDescriptorDto,
    tier: ToolTierDto,
    selected: string[],
    beyondCeiling: boolean,
  ) => {
    const baseId = tool.companionOf;
    if (baseId !== null) {
      const baseEnabled = config.enabledTools.includes(baseId);
      const baseName = tools.find((item) => item.id === baseId)?.name ?? baseId;
      const status = baseEnabled
        ? t("chat.tool.panel.companion_of", { base: baseName })
        : t("chat.tool.panel.companion_blocked", { base: baseName });
      const descKey = TOOL_DESC_KEYS[tool.id];
      return (
        <Tooltip
          key={tool.id}
          placement="top"
          title={
            <div className={css.tooltipBody}>
              {descKey && <span>{t(descKey)}</span>}
              <span className={css.tooltipHint}>{status}</span>
            </div>
          }
        >
          <span
            className={`${css.companion} ${baseEnabled ? "" : css.companionIdle}`}
          >
            {tool.name}
          </span>
        </Tooltip>
      );
    }
    if (isMetaTool(tool)) {
      return (
        <Tooltip
          key={tool.id}
          title={
            TOOL_DESC_KEYS[tool.id]
              ? t(TOOL_DESC_KEYS[tool.id])
              : tool.description || tool.name
          }
          placement="top"
        >
          <Checkbox
            checked={selected.includes(tool.id)}
            disabled={beyondCeiling}
            onChange={(event) => {
              const next = event.target.checked
                ? [...selected, tool.id]
                : selected.filter((id) => id !== tool.id);
              onChange(setTierSelection(config, tools, tier, next));
            }}
            className={css.checkbox}
          >
            {tool.name}
            <Tag className={css.metaBadge}>
              {t("chat.tool.panel.meta_badge")}
            </Tag>
          </Checkbox>
        </Tooltip>
      );
    }
    return (
      <Tooltip
        key={tool.id}
        title={
          TOOL_DESC_KEYS[tool.id]
            ? t(TOOL_DESC_KEYS[tool.id])
            : tool.description || tool.name
        }
        placement="top"
      >
        <Checkbox
          checked={selected.includes(tool.id)}
          disabled={beyondCeiling}
          onChange={(event) => {
            const next = event.target.checked
              ? [...selected, tool.id]
              : selected.filter((id) => id !== tool.id);
            onChange(setTierSelection(config, tools, tier, next));
          }}
          className={css.checkbox}
        >
          {tool.name}
        </Checkbox>
      </Tooltip>
    );
  };

  return (
    <div className={css.root} data-tool-panel>
      <div className={css.header}>
        <Typography.Text strong>{t("chat.tool.panel.title")}</Typography.Text>
        <Typography.Text type="secondary" className={css.count}>
          {t("chat.tool.panel.enabled_count", { count: enabledCount })}
        </Typography.Text>
      </div>

      <div className={css.controls}>
        <label className={css.field}>
          <span className={css.label}>{t("chat.tool.panel.tier_label")}</span>
          <Select
            size="small"
            value={config.maxToolTier}
            onChange={setTier}
            open={tierOpen}
            onOpenChange={setTierOpen}
            classNames={{ popup: { root: "tool-panel-dropdown" } }}
            options={TOOL_TIERS.map((tier) => ({
              value: tier,
              label: t(`chat.tool.approval.tier.${tier}`),
            }))}
            className={css.select}
          />
        </label>
        <label className={css.field}>
          <span className={css.label}>{t("chat.tool.panel.level_label")}</span>
          <Select<ApprovalLevelDto, ApprovalLevelOption>
            size="small"
            value={config.approvalLevel}
            onChange={setLevel}
            open={levelOpen}
            onOpenChange={setLevelOpen}
            popupMatchSelectWidth={false}
            virtual={false}
            classNames={{ popup: { root: "tool-panel-dropdown" } }}
            options={levelOptions}
            optionRender={(option) => (
              <div className={css.levelOption}>
                <span>{option.data.label}</span>
                <span className={css.levelOptionHint}>{option.data.hint}</span>
              </div>
            )}
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
            const selected = selectedIdsInTier(config, tools, tier);
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
                  <div className={css.toolList}>
                    {groupToolUnits(items).map((unit) => (
                      <div key={unit.head.id} className={css.toolUnit}>
                        {renderEntry(unit.head, tier, selected, beyondCeiling)}
                        {unit.companions.map((companion) =>
                          renderEntry(companion, tier, selected, beyondCeiling),
                        )}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
