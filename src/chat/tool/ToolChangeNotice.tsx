import React from "react";
import { InfoCircleOutlined } from "@ant-design/icons";
import css from "./ToolChangeNotice.module.css";

interface ToolChangeNoticeProps {
  text: string;
}

/**
 * 工具可用性变更的系统提示（I38d §4.7）。
 *
 * 只做展示，不落库：提示是「本次装配动作的回执」，切换会话或下一次变更即替换，
 * 不进入模型上下文，也不污染历史消息。
 */
export const ToolChangeNotice: React.FC<ToolChangeNoticeProps> = ({ text }) => (
  <div className={css.root} role="status">
    <InfoCircleOutlined className={css.icon} />
    <span className={css.text}>{text}</span>
  </div>
);
