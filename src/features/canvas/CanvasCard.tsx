import { memo } from "react";
import type { CanvasCardView, CanvasPortSide } from "./types";
import styles from "./canvas.module.css";

export interface CanvasCardProps {
  card: CanvasCardView;
  selected: boolean;
  onSelect: (id: string) => void;
  /** 占位端口圆点按下：从该端口拖出新建连线（编辑层，仅落会话状态）。 */
  onPortDown?: (
    cardId: string,
    side: CanvasPortSide,
    event: React.PointerEvent<HTMLSpanElement>,
  ) => void;
}

/**
 * 卡片壳（I68 §7.3）：六件构成全部在场——头部（拖拽柄 + 类别徽标 + 名称 + 「示例」徽标）、
 * 主体（参数摘要 + 附加区）、端口区（圆点 + 占位类型标签）、脚部（状态药丸 + 引用 chip + 时长）、
 * 选择态、拖拽柄；轮次卡片另有出口角标与阶段轨道。
 * 只认已本地化文案、`tone` 与占位端口方位，不判断区带、行号与状态语义（§5.1 分层纪律）。
 */
function CanvasCardImpl({
  card,
  selected,
  onSelect,
  onPortDown,
}: CanvasCardProps) {
  const ports = card.ports ?? [];
  const params = card.params ?? [];

  const ariaLabel = [
    card.kindBadge,
    card.title,
    card.status,
    card.body.kind === "stages"
      ? `${card.body.label} ${card.body.count}`
      : undefined,
  ]
    .filter((part): part is string => part !== undefined)
    .join(" · ");

  return (
    <button
      type="button"
      className={styles.card}
      data-canvas-card
      data-card-id={card.id}
      data-tone={card.tone ?? "default"}
      data-container={card.container === true}
      data-selected={selected}
      aria-pressed={selected}
      aria-label={ariaLabel}
      style={{
        left: card.box.x,
        top: card.box.y,
        width: card.box.w,
        height: card.box.h,
      }}
      onClick={() => onSelect(card.id)}
    >
      <span className={styles.cardGrip} aria-hidden="true" />

      <span className={styles.cardHeader}>
        <span className={styles.cardKindBadge}>{card.kindBadge}</span>
        <span className={styles.cardTitle}>{card.title}</span>
        {card.exampleBadge !== undefined && (
          <span className={styles.cardExampleBadge}>{card.exampleBadge}</span>
        )}
      </span>

      <span className={styles.cardBody}>
        {params.length > 0 && (
          <span className={styles.cardParams}>
            {params.map((param, index) => (
              <span
                key={`${card.id}-param-${index}`}
                className={styles.cardParamRow}
              >
                <span className={styles.cardParamLabel}>{param.label}</span>
                <span className={styles.cardParamValue}>{param.value}</span>
              </span>
            ))}
          </span>
        )}
        <span className={styles.cardExtra} data-kind={card.body.kind}>
          <span className={styles.cardExtraHead}>
            <span className={styles.cardExtraLabel}>{card.body.label}</span>
            {card.body.kind === "stages" && (
              <span className={styles.cardStageCount}>{card.body.count}</span>
            )}
          </span>
          {card.body.kind === "stages" ? (
            <span className={styles.cardStages}>
              {card.body.stages.map((stage) => (
                <span
                  key={`${card.id}-stage-${stage.id}`}
                  className={styles.cardStage}
                  data-tone={stage.tone}
                  title={`${stage.title} · ${stage.state} · ${stage.description}`}
                >
                  <span className={styles.cardStageOrdinal}>
                    {stage.ordinal}
                  </span>
                  <span className={styles.cardStageTitle}>{stage.title}</span>
                  <span
                    className={styles.cardStageDot}
                    data-tone={stage.tone}
                    aria-hidden="true"
                  />
                  <span className={styles.cardStageState}>{stage.state}</span>
                </span>
              ))}
            </span>
          ) : card.body.kind === "subgraph" ? (
            <span className={styles.cardSubgraph} aria-hidden="true">
              {Array.from({ length: 6 }, (_, index) => (
                <span
                  key={`${card.id}-subgraph-node-${index}`}
                  className={styles.cardSubgraphNode}
                />
              ))}
              {Array.from({ length: 4 }, (_, index) => (
                <span
                  key={`${card.id}-subgraph-link-${index}`}
                  className={styles.cardSubgraphLink}
                  data-link={index}
                />
              ))}
            </span>
          ) : (
            <span className={styles.cardPlaceholder} aria-hidden="true" />
          )}
        </span>
      </span>

      <span className={styles.cardMeta}>
        {card.status !== undefined && (
          <span
            className={styles.cardStatusPill}
            data-tone={card.tone ?? "default"}
          >
            {card.status}
          </span>
        )}
        {card.reference !== undefined && (
          <span className={styles.cardReference}>{card.reference}</span>
        )}
        {card.measure !== undefined && (
          <span className={styles.cardMeasure}>{card.measure}</span>
        )}
      </span>

      {card.exit !== undefined && card.exit !== null && (
        <span className={styles.cardExit} aria-hidden="true">
          <span className={styles.exitSwatch}>
            <span className={styles.exitGlyph} data-exit={card.exit} />
          </span>
        </span>
      )}

      {ports.length > 0 && (
        <span className={styles.cardPorts}>
          {ports.map((port, index) => (
            <span
              key={`${card.id}-port-${port.side}-${index}`}
              className={styles.cardPortAnchor}
              data-side={port.side}
            >
              <span
                className={styles.cardPortDot}
                data-port-side={port.side}
                onPointerDown={(event) =>
                  onPortDown?.(card.id, port.side, event)
                }
              />
              <span className={styles.cardPortLabel} aria-hidden="true">
                {port.typeLabel}
              </span>
            </span>
          ))}
        </span>
      )}
    </button>
  );
}

export const CanvasCard = memo(CanvasCardImpl);
