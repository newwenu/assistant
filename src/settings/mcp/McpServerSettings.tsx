import React, { useCallback, useEffect, useMemo, useState } from "react";
import {
  Alert,
  App,
  Button,
  Drawer,
  Form,
  Input,
  Space,
  Switch,
  Tag,
  Typography,
} from "antd";
import { DeleteOutlined, EditOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { translateKey } from "@/i18n";
import { useMcpServerStore } from "@/stores/mcpServerStore";
import type { McpServerDto } from "@/types/protocol.contract";
import { SettingsPageShell } from "../shared/layout/SettingsPageShell";
import {
  EntityListContent,
  ListHeaderActions,
  type SectionConfig,
} from "../shared/EntityListContent";
import { useSettingMessage } from "../shared/useSettingMessage";
import {
  buildLaunch,
  formatArgs,
  isLaunchDirty,
  maskEnvKeys,
  parseEnv,
  requiresEnvRefill,
  statusLabelKey,
  statusTagColor,
  type LaunchDraft,
} from "./serverForm";

interface ServerFormData {
  name: string;
  command: string;
  argsText: string;
  cwd: string;
  envText: string;
}

const EMPTY_FORM: ServerFormData = {
  name: "",
  command: "",
  argsText: "",
  cwd: "",
  envText: "",
};

export const McpServerSettings: React.FC = () => {
  const { t } = useTranslation();
  const { modal } = App.useApp();
  const msg = useSettingMessage();
  const store = useMcpServerStore();
  const [form] = Form.useForm<ServerFormData>();
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [editing, setEditing] = useState<McpServerDto | null>(null);
  const [searchQuery, setSearchQuery] = useState("");

  const loadServers = useMcpServerStore((s) => s.loadServers);

  useEffect(() => {
    void loadServers();
  }, [loadServers]);

  const openCreate = useCallback(() => {
    setEditing(null);
    form.setFieldsValue(EMPTY_FORM);
    setDrawerOpen(true);
  }, [form]);

  const openEdit = useCallback(
    (server: McpServerDto) => {
      setEditing(server);
      form.setFieldsValue({
        name: server.name,
        command: server.command,
        argsText: formatArgs(server.args),
        cwd: server.cwd ?? "",
        envText: "",
      });
      setDrawerOpen(true);
    },
    [form],
  );

  const closeDrawer = useCallback(() => {
    setDrawerOpen(false);
    setEditing(null);
  }, []);

  const handleToggle = useCallback(
    async (server: McpServerDto, enabled: boolean) => {
      try {
        await store.setEnabled(server.id, enabled);
      } catch (e) {
        msg.error("settings.mcp.toggle_failed", e);
      }
    },
    [store, msg],
  );

  const handleDelete = useCallback(
    (server: McpServerDto) => {
      modal.confirm({
        title: t("settings.mcp.confirm_delete.title", { name: server.name }),
        content: t("settings.mcp.confirm_delete.content"),
        okButtonProps: { danger: true },
        onOk: async () => {
          try {
            await store.deleteServer(server.id);
          } catch (e) {
            msg.error("settings.mcp.delete_failed", e);
          }
        },
      });
    },
    [modal, store, t, msg],
  );

  const handleSave = useCallback(async () => {
    const values = await form.validateFields();
    const name = values.name.trim();
    const draft: LaunchDraft = {
      command: values.command,
      argsText: values.argsText,
      cwd: values.cwd ?? "",
      envText: values.envText ?? "",
    };

    if (editing) {
      if (requiresEnvRefill(draft, editing)) {
        msg.warning("settings.mcp.env_refill_required", {
          count: editing.envKeys.length,
        });
        return;
      }
      const dirty = isLaunchDirty(draft, editing);
      const parsed = parseEnv(draft.envText);
      if (dirty && parsed.invalidLines.length > 0) {
        msg.error("settings.mcp.env_invalid", {
          lines: parsed.invalidLines.join(" / "),
        });
        return;
      }
      try {
        await store.updateServer(
          dirty
            ? { id: editing.id, name, launch: buildLaunch(draft) }
            : { id: editing.id, name },
        );
        msg.success("settings.mcp.saved");
        closeDrawer();
      } catch (e) {
        msg.error("settings.mcp.save_failed", e);
      }
      return;
    }

    const parsed = parseEnv(draft.envText);
    if (parsed.invalidLines.length > 0) {
      msg.error("settings.mcp.env_invalid", {
        lines: parsed.invalidLines.join(" / "),
      });
      return;
    }
    try {
      await store.createServer({ name, launch: buildLaunch(draft) });
      msg.success("settings.mcp.created");
      closeDrawer();
    } catch (e) {
      msg.error("settings.mcp.save_failed", e);
    }
  }, [editing, form, store, msg, closeDrawer]);

  const filteredServers = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();
    if (!query) return store.servers;
    return store.servers.filter(
      (server) =>
        server.name.toLowerCase().includes(query) ||
        server.id.toLowerCase().includes(query) ||
        server.command.toLowerCase().includes(query),
    );
  }, [store.servers, searchQuery]);

  const renderServerRow = useCallback(
    (server: McpServerDto) => {
      const envSummary = server.envKeys.length
        ? maskEnvKeys(server.envKeys)
        : null;
      return (
        <div key={server.id} className="entity-list-row">
          <div className="entity-list-row-info">
            <Typography.Text strong className="entity-list-row-name">
              {server.name}
            </Typography.Text>
            <Tag
              color={statusTagColor(server.status)}
              style={{ flexShrink: 0 }}
            >
              {translateKey(statusLabelKey(server.status))}
            </Tag>
            <span className="entity-list-row-detail">
              {[server.command, ...server.args].join(" ")}
            </span>
            {server.cwd && (
              <span className="entity-list-row-detail">{server.cwd}</span>
            )}
            {envSummary && (
              <span className="entity-list-row-detail">{envSummary}</span>
            )}
            {server.lastError && (
              <Typography.Text
                type="danger"
                ellipsis
                className="entity-list-row-detail"
              >
                {server.lastError}
              </Typography.Text>
            )}
          </div>
          <Space size={4}>
            <Switch
              size="small"
              checked={server.enabled}
              onChange={(checked) => void handleToggle(server, checked)}
            />
            <Button
              type="text"
              size="small"
              icon={<EditOutlined />}
              onClick={() => openEdit(server)}
            />
            <Button
              type="text"
              size="small"
              danger
              icon={<DeleteOutlined />}
              onClick={() => handleDelete(server)}
            />
          </Space>
        </div>
      );
    },
    [handleDelete, handleToggle, openEdit],
  );

  const sections: SectionConfig<McpServerDto>[] = useMemo(
    () => [
      {
        label: t("settings.mcp.section.enabled"),
        items: filteredServers.filter((server) => server.enabled),
        renderRow: renderServerRow,
      },
      {
        label: t("settings.mcp.section.disabled"),
        items: filteredServers.filter((server) => !server.enabled),
        renderRow: renderServerRow,
      },
    ],
    [filteredServers, renderServerRow, t],
  );

  return (
    <>
      <SettingsPageShell
        wide
        extra={
          <ListHeaderActions
            searchQuery={searchQuery}
            onSearchChange={setSearchQuery}
            addLabel={t("settings.mcp.add")}
            onAdd={openCreate}
            searchPlaceholder={t("settings.mcp.search_placeholder")}
          />
        }
      >
        <Alert
          type="info"
          showIcon
          style={{ marginBottom: 16 }}
          title={t("settings.mcp.isolation_notice")}
        />
        <EntityListContent
          items={filteredServers}
          sections={sections}
          emptyDescription={t("settings.mcp.empty")}
        />
      </SettingsPageShell>
      <Drawer
        title={editing ? t("settings.mcp.edit") : t("settings.mcp.add")}
        open={drawerOpen}
        onClose={closeDrawer}
        size="large"
        extra={
          <Button type="primary" onClick={() => void handleSave()}>
            {t("common.actions.save")}
          </Button>
        }
      >
        {editing && (
          <Alert
            type="warning"
            showIcon
            style={{ marginBottom: 16 }}
            title={t("settings.mcp.env_masked_notice", {
              count: editing.envKeys.length,
            })}
          />
        )}
        <Form form={form} layout="vertical" initialValues={EMPTY_FORM}>
          {editing && (
            <Form.Item label={t("settings.mcp.field.id")}>
              <Input value={editing.id} disabled />
            </Form.Item>
          )}
          <Form.Item
            name="name"
            label={t("settings.mcp.field.name")}
            rules={[
              {
                required: true,
                whitespace: true,
                message: t("settings.mcp.field.name_required"),
              },
            ]}
          >
            <Input placeholder={t("settings.mcp.field.name_placeholder")} />
          </Form.Item>
          <Form.Item
            name="command"
            label={t("settings.mcp.field.command")}
            extra={t("settings.mcp.field.command_extra")}
            rules={[
              {
                required: true,
                whitespace: true,
                message: t("settings.mcp.field.command_required"),
              },
            ]}
          >
            <Input placeholder="npx" />
          </Form.Item>
          <Form.Item
            name="argsText"
            label={t("settings.mcp.field.args")}
            extra={t("settings.mcp.field.args_extra")}
          >
            <Input.TextArea
              rows={3}
              placeholder={"-y\n@modelcontextprotocol/server-filesystem"}
            />
          </Form.Item>
          <Form.Item
            name="cwd"
            label={t("settings.mcp.field.cwd")}
            extra={t("settings.mcp.field.cwd_extra")}
          >
            <Input />
          </Form.Item>
          <Form.Item
            name="envText"
            label={t("settings.mcp.field.env")}
            extra={t("settings.mcp.field.env_extra")}
          >
            <Input.TextArea rows={4} placeholder={"API_KEY=..."} />
          </Form.Item>
        </Form>
      </Drawer>
    </>
  );
};
