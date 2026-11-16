import type { Message } from "../core/chatTypes";
import type { AgentMode } from "../../stores/agent/agentModeStore";
import type { Provider } from "../../settings/provider/types";

/** 身份行解析结果；`null` 表示该段缺失，由渲染层兜底或省略（I43 §4.3）。 */
export interface MessageIdentity {
  /** 模式展示名；`null` 表示模式缺失（已删除 / `modeId` 为空）。 */
  modeName: string | null;
  /** 模型展示名；`null` 表示 `modelId` 为空，此时省略模型段。 */
  modelName: string | null;
}

/**
 * 解析消息身份（模式名 / 模型名）。
 *
 * 只读 `message.modeId` / `message.modelId` / `message.providerId`，**禁止**回落到
 * 会话或当前选中项（I43 N3 / 禁止项 1、2）。目录为空时自然退化为「未知模式 +
 * 原始 `modelId`」，因此目录加载完成后重算即可，不缓存结果。
 */
export function resolveMessageIdentity(
  message: Pick<Message, "modeId" | "modelId" | "providerId">,
  modes: AgentMode[],
  providers: Provider[],
): MessageIdentity {
  const mode = message.modeId
    ? modes.find((m) => m.id === message.modeId)
    : undefined;

  const provider = message.providerId
    ? providers.find((p) => p.id === message.providerId)
    : undefined;
  const model = message.modelId
    ? provider?.models.find((m) => m.id === message.modelId)
    : undefined;

  const modelName = message.modelId ? model?.name || message.modelId : null;

  return {
    modeName: mode?.name ?? null,
    modelName,
  };
}
