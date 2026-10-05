import React, { useEffect, useState } from "react";
import { Button, Typography, Modal, Empty, Dropdown } from "antd";
import {
  PlusOutlined,
  EditOutlined,
  DeleteOutlined,
  MoreOutlined,
} from "@ant-design/icons";
import {
  useProviderStore,
  type Provider,
  type ProviderPreset,
  type ApiFormat,
  API_FORMATS,
  getApiFormatByType,
} from "../stores/providerStore";
import { FormatTag } from "./ProviderSwatch";
import { AddProviderDialog } from "./AddProviderDialog";
import { ConnectDialog } from "./ConnectDialog";

export const ProviderManager: React.FC = () => {
  const providerStore = useProviderStore();
  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [connectPreset, setConnectPreset] = useState<ProviderPreset | null>(
    null,
  );
  const [connectCustomFormat, setConnectCustomFormat] =
    useState<ApiFormat | null>(null);
  const [editProvider, setEditProvider] = useState<Provider | null>(null);
  const [connectOpen, setConnectOpen] = useState(false);

  useEffect(() => {
    providerStore.loadProviders();
  }, []);

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

  const handleEdit = (provider: Provider) => {
    setConnectPreset(null);
    setConnectCustomFormat(null);
    setEditProvider(provider);
    setConnectOpen(true);
  };

  const handleDelete = (id: string) => {
    Modal.confirm({
      title: "确认删除此 Provider？",
      content:
        "删除后，使用此 Provider 的对话将保留，但需要重新选择 Provider 才能继续对话",
      onOk: () => providerStore.deleteProvider(id),
    });
  };

  const handleConnect = async (input: Record<string, unknown>) => {
    await providerStore.saveProvider(input);
    setConnectOpen(false);
    setConnectPreset(null);
    setConnectCustomFormat(null);
    setEditProvider(null);
  };

  const providersByFormat = new Map<ApiFormat, Provider[]>();
  for (const p of providerStore.providers) {
    const fmt = getApiFormatByType(p.type);
    const list = providersByFormat.get(fmt) ?? [];
    list.push(p);
    providersByFormat.set(fmt, list);
  }

  return (
    <div className="provider-manager">
      <div className="provider-manager-header">
        <Typography.Title level={4}>AI Provider 设置</Typography.Title>
        <Button type="primary" icon={<PlusOutlined />} onClick={handleAdd}>
          添加 Provider
        </Button>
      </div>

      {providerStore.providers.length === 0 ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description="尚未配置 Provider"
        >
          <Button type="primary" onClick={handleAdd}>
            添加 Provider
          </Button>
        </Empty>
      ) : (
        API_FORMATS.filter(
          (fmt) => (providersByFormat.get(fmt.format)?.length ?? 0) > 0,
        ).map((fmt) => (
          <div key={fmt.format} style={{ marginBottom: 16 }}>
            <div className="provider-section-label">
              {fmt.label}
              <span className="provider-section-count">
                {providersByFormat.get(fmt.format)!.length}
              </span>
            </div>
            <div className="provider-list">
              {providersByFormat.get(fmt.format)!.map((p) => (
                <div key={p.id} className="provider-row">
                  <div className="provider-row-info">
                    <span className="provider-row-name">{p.name}</span>
                    <FormatTag type={p.type} size="small" />
                    <span className="provider-row-detail">
                      {p.api_key_masked
                        ? p.api_key_masked
                        : p.base_url.replace(/^https?:\/\//, "").split("/")[0]}
                    </span>
                    <span className="provider-row-model">
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
              ))}
            </div>
          </div>
        ))
      )}

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
        onSubmit={handleConnect}
      />
    </div>
  );
};
