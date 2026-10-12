import React, { useCallback, useEffect, useMemo, useState } from "react";
import { Button, Typography, Modal, Dropdown } from "antd";
import { EditOutlined, DeleteOutlined, MoreOutlined } from "@ant-design/icons";
import {
  useProviderStore,
  type Provider,
  type ProviderPreset,
  type ApiFormat,
  API_FORMATS,
  getApiFormatByType,
} from "@/stores/providerStore";
import { FormatTag } from "./FormatTag";
import { AddProviderDialog } from "./AddProviderDialog";
import { ConnectDialog } from "./ConnectDialog";
import {
  EntityListPageShell,
  type EntityListSectionConfig,
} from "../shared/EntityListPageShell";

export const ProviderManager: React.FC = () => {
  const providerStore = useProviderStore();
  const [modal, modalContextHolder] = Modal.useModal();
  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [connectPreset, setConnectPreset] = useState<ProviderPreset | null>(
    null,
  );
  const [connectCustomFormat, setConnectCustomFormat] =
    useState<ApiFormat | null>(null);
  const [editProvider, setEditProvider] = useState<Provider | null>(null);
  const [connectOpen, setConnectOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");

  const loadProviders = useProviderStore((s) => s.loadProviders);

  useEffect(() => {
    void loadProviders();
  }, [loadProviders]);

  const handleAdd = () => {
    setAddDialogOpen(true);
  };

  const handlePickPreset = (preset: ProviderPreset) => {
    setAddDialogOpen(false);
    setConnectPreset(preset);
    setConnectCustomFormat(null);
    setEditProvider(null);
    setConnectOpen(true);
  };

  const handleAddCustom = (format: ApiFormat) => {
    setAddDialogOpen(false);
    setConnectPreset(null);
    setConnectCustomFormat(format);
    setEditProvider(null);
    setConnectOpen(true);
  };

  const handleEdit = useCallback((provider: Provider) => {
    setConnectPreset(null);
    setConnectCustomFormat(null);
    setEditProvider(provider);
    setConnectOpen(true);
  }, []);

  const handleDelete = useCallback(
    (id: string) => {
      modal.confirm({
        title: "确认删除此 Provider？",
        content:
          "删除后，使用此 Provider 的对话将保留，但需要重新选择 Provider 才能继续对话",
        onOk: () => providerStore.deleteProvider(id),
      });
    },
    [modal, providerStore],
  );

  const handleConnect = async (input: Record<string, unknown>) => {
    await providerStore.saveProvider(input);
    setConnectOpen(false);
    setConnectPreset(null);
    setConnectCustomFormat(null);
    setEditProvider(null);
  };

  const providersByFormat = useMemo(() => {
    const map = new Map<ApiFormat, Provider[]>();
    for (const p of providerStore.providers) {
      const fmt = getApiFormatByType(p.type);
      const list = map.get(fmt) ?? [];
      list.push(p);
      map.set(fmt, list);
    }
    return map;
  }, [providerStore.providers]);

  const sections: EntityListSectionConfig<Provider>[] = useMemo(() => {
    return API_FORMATS.filter(
      (fmt) => (providersByFormat.get(fmt.format)?.length ?? 0) > 0,
    ).map((fmt) => {
      const allItems = providersByFormat.get(fmt.format)!;
      const filteredItems = searchQuery
        ? allItems.filter(
            (p) =>
              p.name.includes(searchQuery) || p.base_url.includes(searchQuery),
          )
        : allItems;
      return {
        label: fmt.label,
        items: filteredItems,
        renderRow: (p: Provider) => (
          <div key={p.id} className="entity-list-row">
            <div className="entity-list-row-info">
              <Typography.Text strong className="entity-list-row-name">
                {p.name}
              </Typography.Text>
              <FormatTag type={p.type} size="small" />
              <span className="entity-list-row-detail">
                {p.api_key_masked
                  ? p.api_key_masked
                  : p.base_url.replace(/^https?:\/\//, "").split("/")[0]}
              </span>
              <span className="entity-list-row-model">
                {p.default_model || "—"}
              </span>
            </div>
            <Dropdown
              menu={{
                items: [
                  {
                    key: "edit",
                    icon: <EditOutlined />,
                    label: "编辑",
                    onClick: () => handleEdit(p),
                  },
                  {
                    key: "delete",
                    icon: <DeleteOutlined />,
                    label: "删除",
                    danger: true,
                    onClick: () => handleDelete(p.id),
                  },
                ],
              }}
              trigger={["click"]}
            >
              <Button type="text" size="small" icon={<MoreOutlined />} />
            </Dropdown>
          </div>
        ),
      };
    });
  }, [providersByFormat, searchQuery, handleEdit, handleDelete]);

  return (
    <>
      {modalContextHolder}
      <EntityListPageShell<Provider>
        title="AI Provider 设置"
        items={providerStore.providers}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        addLabel="添加 Provider"
        onAdd={handleAdd}
        emptyDescription="尚未配置 Provider"
        sections={sections}
      >
        <AddProviderDialog
          open={addDialogOpen}
          onClose={() => setAddDialogOpen(false)}
          onPickPreset={handlePickPreset}
          onAddCustom={handleAddCustom}
        />

        <ConnectDialog
          provider={editProvider}
          preset={connectPreset}
          customFormat={connectCustomFormat}
          open={connectOpen}
          onClose={() => {
            setConnectOpen(false);
            setConnectPreset(null);
            setConnectCustomFormat(null);
            setEditProvider(null);
          }}
          onSubmit={(input) => void handleConnect(input)}
        />
      </EntityListPageShell>
    </>
  );
};
