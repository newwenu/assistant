import React, { useCallback, useEffect, useMemo, useState } from "react";
import { Alert, Button, Typography, Modal, Dropdown } from "antd";
import { EditOutlined, DeleteOutlined, MoreOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { translateKey } from "@/i18n";
import { useProviderStore } from "@/stores/providerStore";
import {
  API_FORMATS,
  getApiFormatByType,
} from "@/settings/core/provider/providerLookup";
import type {
  Provider,
  ProviderInput,
  ProviderPreset,
  ApiFormat,
} from "./types";
import { FormatTag } from "./FormatTag";
import { AddProviderDialog } from "./AddProviderDialog";
import { ConnectDialog } from "./ConnectDialog";
import { SettingsPageShell } from "../shared/layout/SettingsPageShell";
import {
  EntityListContent,
  ListHeaderActions,
  type SectionConfig,
} from "../shared/EntityListContent";
import { getProviderFormatLabel } from "../core/provider/providerFormatRegistry";

export const ProviderManager: React.FC = () => {
  const { t } = useTranslation();
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
        title: t("settings.provider.confirm_delete.title"),
        content: t("settings.provider.confirm_delete.content"),
        onOk: () => providerStore.deleteProvider(id),
      });
    },
    [modal, providerStore, t],
  );

  const handleConnect = async (input: ProviderInput) => {
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

  const sections: SectionConfig<Provider>[] = useMemo(() => {
    return API_FORMATS.filter(
      (fmt) => (providersByFormat.get(fmt.format)?.length ?? 0) > 0,
    ).map((fmt) => {
      const allItems = providersByFormat.get(fmt.format)!;
      const filteredItems = searchQuery
        ? allItems.filter(
            (p) =>
              p.name.includes(searchQuery) || p.baseUrl.includes(searchQuery),
          )
        : allItems;
      return {
        label: translateKey(getProviderFormatLabel(fmt.format)),
        items: filteredItems,
        renderRow: (p: Provider) => (
          <div key={p.id} className="entity-list-row">
            <div className="entity-list-row-info">
              <Typography.Text strong className="entity-list-row-name">
                {p.name}
              </Typography.Text>
              <FormatTag type={p.type} size="small" />
              <span className="entity-list-row-detail">
                {p.apiKeyMasked
                  ? p.apiKeyMasked
                  : p.baseUrl.replace(/^https?:\/\//, "").split("/")[0]}
              </span>
              <span className="entity-list-row-model">
                {p.defaultModel || "—"}
              </span>
            </div>
            <Dropdown
              menu={{
                items: [
                  {
                    key: "edit",
                    icon: <EditOutlined />,
                    label: t("common.actions.edit"),
                    onClick: () => handleEdit(p),
                  },
                  {
                    key: "delete",
                    icon: <DeleteOutlined />,
                    label: t("common.actions.delete"),
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
  }, [providersByFormat, searchQuery, handleEdit, handleDelete, t]);

  return (
    <>
      {modalContextHolder}
      <SettingsPageShell
        wide
        extra={
          <ListHeaderActions
            searchQuery={searchQuery}
            onSearchChange={setSearchQuery}
            addLabel={t("settings.provider.add")}
            onAdd={handleAdd}
          />
        }
      >
        {providerStore.providers.length === 0 &&
        providerStore.providersError ? (
          <Alert
            type="error"
            showIcon
            title={providerStore.providersError}
            action={
              <Button size="small" onClick={() => void loadProviders()}>
                {t("common.actions.retry")}
              </Button>
            }
          />
        ) : (
          <EntityListContent
            items={providerStore.providers}
            sections={sections}
            emptyDescription={t("settings.provider.empty")}
          />
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
          onSubmit={(input) => void handleConnect(input)}
        />
      </SettingsPageShell>
    </>
  );
};
