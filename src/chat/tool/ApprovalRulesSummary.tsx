import React from "react";
import { Button, Tooltip } from "antd";
import { InfoCircleOutlined, SettingOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import css from "./ApprovalRulesSummary.module.css";

interface ApprovalRulesSummaryProps {
  /** 跳转设置页编辑；导航属于视图职责，组件不做路由决策。 */
  onEdit: () => void;
}

/**
 * 会话面板里的审批规则入口（I70 §7.3）。
 *
 * 只保留「标题 + 去设置编辑」一行：面板挂在输入区上方、空间紧张，
 * 逐条列出规则会挤占消息区。这里**不显示条数**：会话里的 `approvalRules`
 * 只是播种快照，既不含项目级 `permissions.json` 的约束层，也不含预设档与
 * 硬保护规则，用它算「几条规则」会把「正在生效的审批约束」显示成「无规则」，
 * 与后端实际求值不符。规则的查看与修改统一收在设置页的 `ApprovalRulesEditor`。
 */
export const ApprovalRulesSummary: React.FC<ApprovalRulesSummaryProps> = ({
  onEdit,
}) => {
  const { t } = useTranslation();

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
        <Button
          type="link"
          size="small"
          icon={<SettingOutlined />}
          onClick={onEdit}
          className={css.edit}
        >
          {t("common.approval_rules.edit_in_settings")}
        </Button>
      </div>
    </div>
  );
};
