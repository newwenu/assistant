import React, { useEffect, useMemo, useRef, useState } from "react";
import {
  AutoComplete,
  Button,
  Dropdown,
  Input,
  Select,
  Tooltip,
  Typography,
} from "antd";
import {
  DeleteOutlined,
  InfoCircleOutlined,
  PlusOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { translateKey } from "@/i18n";
import type {
  ApprovalRuleDto,
  ApprovalRuleEffectDto,
} from "@/types/protocol.contract";
import type { ApprovalRuleError } from "../core/toolConfig";
import {
  APPROVAL_ACTION_OPTIONS,
  APPROVAL_RULES_MAX,
  APPROVAL_RULE_SHORTCUTS,
  areApprovalRulesEqual,
  validateApprovalRules,
} from "../core/toolConfig";
import css from "./ApprovalRulesEditor.module.css";

/** 效果选项：顺序即下拉展示顺序，文案描述实际行为而非「允许 / 拒绝」。 */
const EFFECTS: ApprovalRuleEffectDto[] = ["allow", "ask", "deny"];

/** 效果文案键写成字面量，i18n 键的类型检查才能生效。 */
const EFFECT_LABEL_KEYS = {
  allow: "common.approval_rules.effect.allow",
  ask: "common.approval_rules.effect.ask",
  deny: "common.approval_rules.effect.deny",
} as const;

/** 路径类动作：资源是路径，占位文案与其他动作不同（I70 §3.4）。 */
const PATH_ACTIONS: ReadonlySet<string> = new Set([
  "read",
  "edit",
  "delete",
  "purge",
]);

/**
 * 校验错误的展示文案（I70 §6.1 E1–E5）。
 *
 * 编辑器内联提示与保存前拒绝共用同一份文案来源，避免两处措辞漂移；
 * 后端 `error.approval_rules_invalid` 只作为兜底，不承担字段级描述。
 */
export function approvalRuleErrorText(error: ApprovalRuleError): string {
  switch (error.code) {
    case "tooMany":
      return translateKey("common.approval_rules.errors.tooMany", {
        len: error.len,
        limit: error.limit,
      });
    case "actionLength":
      return translateKey("common.approval_rules.errors.actionLength", {
        index: error.index,
        limit: error.limit,
      });
    case "resourceLength":
      return translateKey("common.approval_rules.errors.resourceLength", {
        index: error.index,
        limit: error.limit,
      });
    case "allowBottomLine":
      return translateKey("common.approval_rules.errors.allowBottomLine", {
        index: error.index,
        action: error.action,
      });
    case "reservedTier":
      return translateKey("common.approval_rules.errors.reservedTier", {
        index: error.index,
      });
  }
}

interface ApprovalRulesEditorProps {
  rules: ApprovalRuleDto[];
  onChange: (next: ApprovalRuleDto[]) => void;
  disabled?: boolean;
  /**
   * 草稿校验状态上报（可选）。
   *
   * 非法中间态不走 `onChange`，父级否则只能看到「没有新改动」，会把上一个合法草稿
   * 当成用户要保存的内容。项目规则视图据此禁用保存；模式级不传，行为不变。
   */
  onValidationChange?: (error: ApprovalRuleError | null) => void;
}

/**
 * 用户审批规则编辑器（I70 §7.3）。
 *
 * 只在设置页使用：模式级 `ToolAssemblySection` 与项目级 `ProjectRulesSection`
 * 共用同一份编辑器与校验（后者写入工作区的 `permissions.json`）；
 * 会话面板空间紧张，只做只读展示（`ApprovalRulesSummary`），修改统一收在设置里。
 * 组件只上报新数组，不自行持久化。
 * 校验与后端 `validate_rules` 同规则，后端仍会复校验并以其结果为准。
 *
 * 内部保留一份草稿：清空某个输入框这类中间态必然不合法，若直接上报，
 * 「每次变更即落库」会把整条配置打回并弹错，用户连字都改不了。
 * 因此**只有合法状态才上报**，不合法时留在草稿里由内联提示引导修正；
 * 外部（回滚 / 重新加载）改动会覆盖草稿。
 */
export const ApprovalRulesEditor: React.FC<ApprovalRulesEditorProps> = ({
  rules,
  onChange,
  disabled = false,
  onValidationChange,
}) => {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<ApprovalRuleDto[]>(rules);
  const error = useMemo(() => validateApprovalRules(draft), [draft]);

  useEffect(() => {
    setDraft((current) =>
      areApprovalRulesEqual(current, rules) ? current : rules,
    );
  }, [rules]);

  /** 回调放 ref 里：上报只跟随 `error` 变化，不因父级传入新函数而重复触发。 */
  const reportValidation = useRef(onValidationChange);
  reportValidation.current = onValidationChange;

  useEffect(() => {
    reportValidation.current?.(error);
  }, [error]);

  const apply = (next: ApprovalRuleDto[]) => {
    setDraft(next);
    if (validateApprovalRules(next) === null) onChange(next);
  };

  const replaceAt = (index: number, patch: Partial<ApprovalRuleDto>) => {
    apply(draft.map((rule, i) => (i === index ? { ...rule, ...patch } : rule)));
  };

  const removeAt = (index: number) => {
    apply(draft.filter((_, i) => i !== index));
  };

  const append = () => {
    apply([...draft, { action: "*", resource: "*", effect: "ask" }]);
  };

  /**
   * `delete` 与 `purge` 是两个动作：只写 `delete` 时永久删除仍需确认。
   * 只提示、不自动合并——合并会悄悄改写用户写的规则（I70 §3.4）。
   */
  const deleteOnly =
    draft.some((rule) => rule.action === "delete") &&
    !draft.some((rule) => rule.action === "purge");

  const addShortcut = (action: string, effect: ApprovalRuleEffectDto) => {
    apply([...draft, { action, resource: "*", effect }]);
  };

  return (
    <div className={css.root}>
      <div className={css.header}>
        <Tooltip
          title={
            <span>
              {t("common.approval_rules.hint")}
              <br />
              {t("common.approval_rules.order_hint")}
            </span>
          }
          placement="top"
        >
          <span className={css.title}>
            {t("common.approval_rules.title")}
            <InfoCircleOutlined className={css.titleIcon} />
          </span>
        </Tooltip>
        <Typography.Text type="secondary" className={css.summary}>
          {draft.length === 0
            ? t("common.approval_rules.none")
            : t("common.approval_rules.count", { count: draft.length })}
        </Typography.Text>
      </div>

      {draft.length === 0 ? (
        <Typography.Text type="secondary" className={css.empty}>
          {t("common.approval_rules.empty")}
        </Typography.Text>
      ) : (
        <div className={css.rows}>
          <div className={css.columns} aria-hidden>
            <span className={css.indexCell} />
            <span>{t("common.approval_rules.action_label")}</span>
            <span>{t("common.approval_rules.resource_label")}</span>
            <span>{t("common.approval_rules.effect_label")}</span>
            <span />
          </div>
          {draft.map((rule, index) => (
            <div
              key={index}
              className={`${css.row} ${rule.effect === "deny" ? css.rowDeny : ""}`}
            >
              <span className={css.indexCell}>
                {t("common.approval_rules.row_index", { index: index + 1 })}
              </span>
              <AutoComplete
                size="small"
                value={rule.action}
                onChange={(value) => replaceAt(index, { action: value })}
                options={APPROVAL_ACTION_OPTIONS.map((action) => ({
                  value: action,
                  label: action,
                }))}
                disabled={disabled}
                placeholder={t("common.approval_rules.action_placeholder")}
                className={css.actionInput}
              />
              <Input
                size="small"
                value={rule.resource}
                onChange={(event) =>
                  replaceAt(index, { resource: event.target.value })
                }
                disabled={disabled}
                placeholder={t(
                  PATH_ACTIONS.has(rule.action)
                    ? "common.approval_rules.resource_placeholder_path"
                    : "common.approval_rules.resource_placeholder",
                )}
                className={css.resourceInput}
              />
              <Select<ApprovalRuleEffectDto>
                size="small"
                value={rule.effect}
                onChange={(effect) => replaceAt(index, { effect })}
                disabled={disabled}
                options={EFFECTS.map((effect) => ({
                  value: effect,
                  label: t(EFFECT_LABEL_KEYS[effect]),
                }))}
                className={css.effectSelect}
              />
              <Tooltip title={t("common.approval_rules.remove")}>
                <Button
                  size="small"
                  type="text"
                  danger
                  icon={<DeleteOutlined />}
                  disabled={disabled}
                  onClick={() => removeAt(index)}
                  className={css.removeButton}
                />
              </Tooltip>
            </div>
          ))}
        </div>
      )}

      {deleteOnly && (
        <Typography.Text type="secondary" className={css.hint}>
          {t("common.approval_rules.delete_purge_hint")}
        </Typography.Text>
      )}

      {error && (
        <Typography.Text type="danger" className={css.error}>
          {approvalRuleErrorText(error)}
        </Typography.Text>
      )}

      <div className={css.footer}>
        <Button
          size="small"
          icon={<PlusOutlined />}
          disabled={disabled || draft.length >= APPROVAL_RULES_MAX}
          onClick={append}
          className={css.addButton}
        >
          {t("common.approval_rules.add")}
        </Button>

        {/* 快捷规则只追加标准用户规则，不保存任何开关状态（I70 §3.3）。 */}
        <div className={css.shortcuts}>
          {APPROVAL_RULE_SHORTCUTS.map((shortcut) => (
            <Dropdown
              key={shortcut.key}
              disabled={disabled || draft.length >= APPROVAL_RULES_MAX}
              menu={{
                items: shortcut.effects.map((effect) => ({
                  key: effect,
                  label: t(EFFECT_LABEL_KEYS[effect]),
                })),
                onClick: ({ key }) =>
                  addShortcut(shortcut.action, key as ApprovalRuleEffectDto),
              }}
              trigger={["click"]}
            >
              <Button size="small" disabled={disabled}>
                {t(`common.approval_rules.shortcuts.${shortcut.key}`)}
              </Button>
            </Dropdown>
          ))}
        </div>
      </div>
    </div>
  );
};
