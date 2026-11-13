/** 目录未收录模型时使用的保守档位集合（通用 Provider 路径只认这三档） */
export const REASONING_EFFORTS = ["low", "medium", "high"] as const;

export type ReasoningEffort = (typeof REASONING_EFFORTS)[number];

/** 内置档位值：前后端共认，表示不指定强度、交给 Provider 默认 */
export const REASONING_DEFAULT = "default";
/** 内部「未覆盖」态：只在初始时出现，不占选项位，下发前必被解析成具体档位 */
export const REASONING_INHERIT = "inherit";
export const REASONING_OFF = "off";
export const REASONING_ON = "on";

/** 可选值来自目录投影，故不收敛为字面量联合 */
export type ReasoningChoice = string;

export const DEFAULT_REASONING_CHOICE: ReasoningChoice = REASONING_INHERIT;

/** 目录无信息时的保守集合。`default` 是内置档位，任何模型都可选 */
export const REASONING_CHOICES: readonly ReasoningChoice[] = [
  REASONING_DEFAULT,
  REASONING_OFF,
  ...REASONING_EFFORTS,
];

/** 目录声明的档位；结构对齐 `ModelConfig.reasoningLevels` */
export interface ReasoningLevelsLike {
  toggle: boolean;
  efforts: string[];
}

/**
 * 由目录投影组合出选项，`default` 始终在首位。
 * - 无投影（未知模型）→ 保守集合
 * - 有开关 + 有档位 → 默认 + 关闭 + 每个 effort
 * - 有开关 + 无档位 → 默认 + 关闭 + 开启
 * - 无开关 + 有档位 → 默认 + 每个 effort
 * - 无开关 + 无档位 → 仅默认（模型不接受任何档位声明）
 *
 * `unsupported` 是**黑名单**：只从目录声明的 effort 里剔除，不用可表达集合求交集
 * （否则别名档位会被误删）；`default` / `off` / `on` 是内置语义位，原样保留。
 */
export function reasoningChoices(
  levels: ReasoningLevelsLike | undefined,
  unsupported?: readonly string[],
): readonly ReasoningChoice[] {
  if (!levels) return REASONING_CHOICES;
  const blocked = unsupported ?? [];
  const efforts =
    blocked.length === 0
      ? levels.efforts
      : levels.efforts.filter((effort) => !blocked.includes(effort));
  if (levels.toggle) {
    return efforts.length > 0
      ? [REASONING_DEFAULT, REASONING_OFF, ...efforts]
      : [REASONING_DEFAULT, REASONING_OFF, REASONING_ON];
  }
  return efforts.length > 0
    ? [REASONING_DEFAULT, ...efforts]
    : [REASONING_DEFAULT];
}

/**
 * 差集说明里的档位清单（按目录顺序拼成 `minimal / xhigh`）。
 * 没有会被丢弃的档位时返回 `null`，调用方不追加说明行。
 */
export function unsupportedEffortSummary(
  unsupported: readonly string[] | undefined,
  labelFor: (effort: string) => string,
): string | null {
  if (!unsupported || unsupported.length === 0) return null;
  return unsupported.map(labelFor).join(" / ");
}

export interface ReasoningModeDefaults {
  reasoningEnabled: boolean | null;
  reasoningEffort: string | null;
}

/**
 * 把模式配置映射成档位。
 * 顺序即优先级：模式显式关闭 → 关闭；有 effort → 该档位；显式开启 → 开启；
 * 两者都没声明 → `null`（等于 Provider 默认）。
 */
export function modeReasoningChoice(
  mode: ReasoningModeDefaults | undefined,
): ReasoningChoice | null {
  if (!mode) return null;
  if (mode.reasoningEnabled === false) return REASONING_OFF;
  if (mode.reasoningEffort) return mode.reasoningEffort;
  if (mode.reasoningEnabled === true) return REASONING_ON;
  return null;
}

/**
 * 控件显示值与下发值：已覆盖则原样返回，未覆盖则回落到模式配置。
 * 永不返回 `inherit`，也永不返回 `null`——模式没声明时落到内置 `default`。
 */
export function resolveReasoningChoice(
  choice: ReasoningChoice,
  mode: ReasoningModeDefaults | undefined,
): ReasoningChoice {
  if (choice !== REASONING_INHERIT) return choice;
  return modeReasoningChoice(mode) ?? REASONING_DEFAULT;
}

/** 下发前把内部「未覆盖」态兜底成 `default`，后端不认识 `inherit` */
export function toReasoningLevel(choice: ReasoningChoice): ReasoningChoice {
  return choice === REASONING_INHERIT ? REASONING_DEFAULT : choice;
}
