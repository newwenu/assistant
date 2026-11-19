import { useState } from "react";
import { Alert, Empty, Tag, Typography } from "antd";
import { useTranslation } from "react-i18next";
import { useChatStore } from "@/stores/chat/chatStore";
import { observationKey } from "@/stores/agent/observation/types";
import { GraphCanvas, TemplatePreview } from "./GraphCanvas";
import { InspectorPanel } from "./InspectorPanel";
import { useObservationInstances, useProjections } from "./hooks";
import styles from "./observation.module.css";

/**
 * 观测页（I55 §6.2 渲染侧）：只读副本，不参与控制（O6 / G7）。
 *
 * 无实例时显示空态；会话切走后实例继续累积（后台实例，I55 §6.3）。
 */
export function ObservationPage() {
  const { t } = useTranslation();
  const instances = useObservationInstances();
  const currentConversationId = useChatStore((s) => s.currentConversationId);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);

  const selected =
    (selectedKey === null
      ? null
      : instances.find(
          (item) =>
            observationKey(item.conversationId, item.turnId) === selectedKey,
        )) ??
    instances[0] ??
    null;

  const projections = useProjections(
    selected?.conversationId ?? null,
    selected?.turnId ?? null,
  );
  const selectedObservationKey =
    selected === null
      ? null
      : observationKey(selected.conversationId, selected.turnId);

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <Typography.Title className={styles.headerTitle} level={5}>
          {t("observation.title")}
        </Typography.Title>
        <span className={styles.headerSubtitle}>
          {t("observation.subtitle")}
        </span>
      </header>
      <div className={styles.body} data-empty={instances.length === 0}>
        {instances.length > 0 && (
          <aside className={styles.instancePane}>
            <span className={styles.instancePaneTitle}>
              {t("observation.instances")}
            </span>
            <div className={styles.instanceList}>
              {instances.map((item) => {
                const key = observationKey(item.conversationId, item.turnId);
                return (
                  <button
                    key={key}
                    type="button"
                    className={styles.instanceItem}
                    data-selected={key === selectedObservationKey}
                    onClick={() => setSelectedKey(key)}
                  >
                    <span className={styles.instanceItemHead}>
                      <span className={styles.instanceLabel}>
                        {item.turnIndex === null
                          ? t("observation.turn.unnamed")
                          : t("observation.turn.label", {
                              index: item.turnIndex,
                            })}
                      </span>
                      <Tag
                        color={statusColor(item.status)}
                        style={{ marginInlineEnd: 0 }}
                      >
                        {t(`observation.status.${item.status}`)}
                      </Tag>
                    </span>
                    <span className={styles.instanceMeta}>{item.turnId}</span>
                    <span className={styles.instanceTag}>
                      {item.conversationId === currentConversationId
                        ? t("observation.turn.active")
                        : t("observation.turn.background")}
                    </span>
                  </button>
                );
              })}
            </div>
          </aside>
        )}
        <main className={styles.mainPane}>
          {selected === null || projections === null ? (
            <>
              <TemplatePreview />
              <div className={styles.emptyWrap}>
                <Empty
                  description={
                    <>
                      <div>{t("observation.empty.title")}</div>
                      <span className={styles.canvasHint}>
                        {t("observation.empty.description")}
                      </span>
                    </>
                  }
                />
              </div>
            </>
          ) : selected.evicted ? (
            <Alert
              className={styles.notice}
              type="info"
              showIcon
              title={t("observation.turn.evicted")}
              description={t(`observation.status.${selected.status}`)}
            />
          ) : (
            <>
              <GraphCanvas graph={projections.graph} />
              <InspectorPanel
                summary={projections.summary}
                timeline={projections.timeline}
              />
            </>
          )}
        </main>
      </div>
    </div>
  );
}

function statusColor(status: string): string {
  switch (status) {
    case "running":
      return "processing";
    case "completed":
      return "success";
    case "halted_by_budget":
    case "wrap_up":
      return "warning";
    case "failed":
      return "error";
    default:
      return "default";
  }
}
