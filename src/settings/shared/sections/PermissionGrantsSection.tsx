import { useEffect, useState } from "react";
import {
  Button,
  Empty,
  Modal,
  Popconfirm,
  Select,
  Space,
  Tag,
  Typography,
} from "antd";
import { DeleteOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import {
  permissionService,
  type PermissionGrant,
} from "@/services/permissionService";
import { chatService } from "@/services/chatService";
import type { Conversation } from "@/chat/core/chatTypes";
import { formatRelativeTime } from "@/utils/formatRelativeTime";
import { useSettingMessage } from "../useSettingMessage";

const CLEAR_BUSY_KEY = "__all__";

/**
 * 「已授权操作」列表（I38d §3.3）。
 *
 * 与审批卡片的 `始终允许` 成对出现：给永久放行留一条回收路径。
 * 单条撤销按 `grantKey` 精确匹配，撤销后行不存在时重新拉取列表对齐。
 * 作用域支持会话级 ⇄ 全局互转：升级直接确认，降级需选定目标会话。
 */
export const PermissionGrantsSection: React.FC = () => {
  const { t } = useTranslation();
  const msg = useSettingMessage();
  const [grants, setGrants] = useState<PermissionGrant[]>([]);
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [loading, setLoading] = useState(true);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [scopeTarget, setScopeTarget] = useState<PermissionGrant | null>(null);
  const [targetConversationId, setTargetConversationId] = useState<
    string | null
  >(null);

  const refresh = async () => {
    setLoading(true);
    try {
      setGrants(await permissionService.list());
    } catch (err) {
      msg.error("settings.agent_mode.grants.load_failed", err);
    } finally {
      setLoading(false);
    }
  };

  const loadConversations = async () => {
    try {
      setConversations(await chatService.listConversations());
    } catch {
      setConversations([]);
    }
  };

  useEffect(() => {
    void refresh();
    void loadConversations();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const conversationTitle = (id: string | null): string => {
    if (!id) return "";
    return conversations.find((c) => c.id === id)?.title || id;
  };

  const revoke = async (grantKey: string) => {
    setBusyKey(grantKey);
    try {
      const removed = await permissionService.revoke(grantKey);
      if (removed) {
        setGrants((prev) => prev.filter((g) => g.grantKey !== grantKey));
        msg.success("settings.agent_mode.grants.revoked");
      } else {
        await refresh();
      }
    } catch (err) {
      msg.error("settings.agent_mode.grants.revoke_failed", err);
    } finally {
      setBusyKey(null);
    }
  };

  const clearAll = async () => {
    setBusyKey(CLEAR_BUSY_KEY);
    try {
      await permissionService.clear();
      setGrants([]);
      msg.success("settings.agent_mode.grants.cleared");
    } catch (err) {
      msg.error("settings.agent_mode.grants.clear_failed", err);
    } finally {
      setBusyKey(null);
    }
  };

  const changeScope = async (
    grant: PermissionGrant,
    scope: "session" | "global",
    conversationId?: string | null,
  ) => {
    setBusyKey(grant.grantKey);
    try {
      const moved = await permissionService.setScope(
        grant.grantKey,
        scope,
        conversationId ?? null,
      );
      if (moved) {
        await refresh();
        msg.success("settings.agent_mode.grants.scope_changed");
      } else {
        await refresh();
      }
    } catch (err) {
      msg.error("settings.agent_mode.grants.scope_failed", err);
    } finally {
      setBusyKey(null);
    }
  };

  const openScopeModal = (grant: PermissionGrant) => {
    setScopeTarget(grant);
    setTargetConversationId(grant.conversationId ?? null);
  };

  const confirmScopeModal = async () => {
    if (!scopeTarget || !targetConversationId) return;
    const grant = scopeTarget;
    setScopeTarget(null);
    await changeScope(grant, "session", targetConversationId);
  };

  return (
    <div style={{ marginTop: 32 }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 4,
        }}
      >
        <Typography.Title level={5} style={{ margin: 0 }}>
          {t("settings.agent_mode.grants.title")}
        </Typography.Title>
        <Popconfirm
          title={t("settings.agent_mode.grants.clear_confirm")}
          onConfirm={() => void clearAll()}
          disabled={grants.length === 0 || busyKey !== null}
        >
          <Button
            size="small"
            danger
            icon={<DeleteOutlined />}
            disabled={grants.length === 0 || busyKey !== null}
            loading={busyKey === CLEAR_BUSY_KEY}
          >
            {t("settings.agent_mode.grants.clear")}
          </Button>
        </Popconfirm>
      </div>
      <Typography.Text
        type="secondary"
        style={{ fontSize: 12, display: "block", marginBottom: 8 }}
      >
        {t("settings.agent_mode.grants.description")}
      </Typography.Text>

      {!loading && grants.length === 0 && (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={t("settings.agent_mode.grants.empty")}
        />
      )}

      {grants.map((grant) => (
        <div
          key={grant.grantKey}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            padding: "6px 0",
            borderBottom: "1px solid var(--color-border-secondary)",
          }}
        >
          <Tag style={{ marginInlineEnd: 0 }}>{grant.tool}</Tag>
          <Tag
            color={grant.scope === "global" ? "blue" : "default"}
            style={{ marginInlineEnd: 0 }}
          >
            {t(
              grant.scope === "global"
                ? "settings.agent_mode.grants.scope.global"
                : "settings.agent_mode.grants.scope.session",
            )}
          </Tag>
          {grant.scope === "session" && grant.conversationId && (
            <Typography.Text
              type="secondary"
              style={{ fontSize: 12, flexShrink: 0 }}
              ellipsis={{ tooltip: conversationTitle(grant.conversationId) }}
            >
              {conversationTitle(grant.conversationId)}
            </Typography.Text>
          )}
          <Typography.Text
            style={{
              flex: 1,
              minWidth: 0,
              fontFamily: "var(--font-mono)",
              fontSize: 12,
            }}
            ellipsis={{ tooltip: grant.scopeSummary }}
          >
            {grant.scopeSummary}
          </Typography.Text>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            {formatRelativeTime(grant.createdAt)}
          </Typography.Text>
          <Space size={4}>
            {grant.scope === "session" ? (
              <Popconfirm
                title={t("settings.agent_mode.grants.set_global_confirm")}
                onConfirm={() => void changeScope(grant, "global")}
                disabled={busyKey !== null}
              >
                <Button size="small" disabled={busyKey !== null}>
                  {t("settings.agent_mode.grants.set_global")}
                </Button>
              </Popconfirm>
            ) : (
              <Button
                size="small"
                disabled={busyKey !== null}
                onClick={() => openScopeModal(grant)}
              >
                {t("settings.agent_mode.grants.set_session")}
              </Button>
            )}
            <Button
              size="small"
              loading={busyKey === grant.grantKey}
              disabled={busyKey !== null}
              onClick={() => void revoke(grant.grantKey)}
            >
              {t("settings.agent_mode.grants.revoke")}
            </Button>
          </Space>
        </div>
      ))}

      <Modal
        open={scopeTarget !== null}
        title={t("settings.agent_mode.grants.set_session_title")}
        okText={t("common.actions.confirm")}
        cancelText={t("common.actions.cancel")}
        okButtonProps={{ disabled: !targetConversationId }}
        onOk={() => void confirmScopeModal()}
        onCancel={() => setScopeTarget(null)}
      >
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          {t("settings.agent_mode.grants.set_session_desc")}
        </Typography.Text>
        <Select
          style={{ width: "100%", marginTop: 8 }}
          placeholder={t("settings.agent_mode.grants.set_session_placeholder")}
          value={targetConversationId ?? undefined}
          onChange={(value: string) => setTargetConversationId(value)}
          options={conversations.map((c) => ({
            value: c.id,
            label: c.title || c.id,
          }))}
          showSearch={{ optionFilterProp: "label" }}
        />
      </Modal>
    </div>
  );
};
