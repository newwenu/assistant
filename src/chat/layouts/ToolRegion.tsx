import React from "react";
import type { ToolRegionView } from "./layoutProps";
import { ToolConfigPanel } from "../tool/ToolConfigPanel";
import { ToolToolbar } from "../tool/ToolToolbar";
import { ToolChangeNotice } from "../tool/ToolChangeNotice";

/**
 * 工具区域三个挂点的展示适配（I48 §5）：只把 `ToolRegionView` 摊平成组件 props，
 * 不满足挂载条件时返回 `null`。无状态、无副作用、不 import 服务。
 */

/** 输入区工具栏上方：面板打开时挂载。 */
export const ToolRegionPanel: React.FC<{ view: ToolRegionView }> = ({
  view,
}) => (view.open ? <ToolConfigPanel {...view.panel} /> : null);

/** 输入区工具栏内：概览条恒挂载（agent 链路）。 */
export const ToolRegionToolbar: React.FC<{ view: ToolRegionView }> = ({
  view,
}) => <ToolToolbar {...view.toolbar} open={view.open} />;

/** 消息区下方：有提示文案时挂载。 */
export const ToolRegionNotice: React.FC<{ view: ToolRegionView }> = ({
  view,
}) => (view.notice ? <ToolChangeNotice text={view.notice} /> : null);
