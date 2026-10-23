import { useState } from "react";
import { Button, Input, Typography, Space, Radio, Alert, App } from "antd";
import {
  DownloadOutlined,
  UploadOutlined,
  FolderOpenOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { save, open } from "@tauri-apps/plugin-dialog";

interface ImportExportTabProps {
  loading: boolean;
  exportVault: (format: "json" | "csv", path: string) => Promise<string | null>;
  importVault: (path: string, format: "json" | "csv") => Promise<number | null>;
}

export default function ImportExportTab({
  loading,
  exportVault,
  importVault,
}: ImportExportTabProps) {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [exportFormat, setExportFormat] = useState<"json" | "csv">("json");
  const [importFormat, setImportFormat] = useState<"json" | "csv">("json");
  const [importFilePath, setImportFilePath] = useState("");

  const handleExport = async () => {
    const ext = exportFormat === "json" ? "json" : "csv";
    const filterName = exportFormat === "json" ? "JSON" : "CSV";
    const filePath = await save({
      defaultPath: `vault_export.${ext}`,
      filters: [
        { name: filterName, extensions: [ext] },
        { name: "All Files", extensions: ["*"] },
      ],
    });
    if (!filePath) return;

    const result = await exportVault(exportFormat, filePath);
    if (result) {
      message.success(
        t("vault.import_export.export_success", { path: result }),
      );
    }
  };

  const handlePickImportFile = async () => {
    const ext = importFormat === "json" ? "json" : "csv";
    const filterName = importFormat === "json" ? "JSON" : "CSV";
    const selected = await open({
      multiple: false,
      filters: [
        { name: filterName, extensions: [ext] },
        { name: "All Files", extensions: ["*"] },
      ],
    });
    if (selected && typeof selected === "string") {
      setImportFilePath(selected);
    }
  };

  const handleImport = async () => {
    if (!importFilePath.trim()) {
      message.warning(t("vault.import_export.import_file_placeholder"));
      return;
    }
    const count = await importVault(importFilePath.trim(), importFormat);
    if (count !== null) {
      if (count > 0) {
        message.success(t("vault.import_export.import_success", { count }));
      } else {
        message.info(t("vault.import_export.import_no_entries"));
      }
      setImportFilePath("");
    }
  };

  return (
    <div>
      <Typography.Title level={5}>
        {t("vault.import_export.export_title")}
      </Typography.Title>
      <Typography.Paragraph type="secondary">
        {t("vault.import_export.export_description")}
      </Typography.Paragraph>

      <Alert
        type="warning"
        showIcon
        title={t("vault.import_export.export_warning")}
        style={{ marginBottom: 16 }}
      />

      <Space orientation="vertical" style={{ width: "100%" }}>
        <Radio.Group
          value={exportFormat}
          onChange={(e) => setExportFormat(e.target.value as "json" | "csv")}
        >
          <Radio value="json">
            {t("vault.import_export.export_format_json")}
          </Radio>
          <Radio value="csv">
            {t("vault.import_export.export_format_csv")}
          </Radio>
        </Radio.Group>
        <Button
          type="primary"
          icon={<DownloadOutlined />}
          loading={loading}
          onClick={() => void handleExport()}
        >
          {t("vault.import_export.export_button")}
        </Button>
      </Space>

      <Typography.Title level={5} style={{ marginTop: 24 }}>
        {t("vault.import_export.import_title")}
      </Typography.Title>
      <Typography.Paragraph type="secondary">
        {t("vault.import_export.import_description")}
      </Typography.Paragraph>

      <Space orientation="vertical" style={{ width: "100%" }}>
        <Radio.Group
          value={importFormat}
          onChange={(e) => setImportFormat(e.target.value as "json" | "csv")}
        >
          <Radio value="json">
            {t("vault.import_export.import_format_json")}
          </Radio>
          <Radio value="csv">
            {t("vault.import_export.import_format_csv")}
          </Radio>
        </Radio.Group>
        <Space.Compact style={{ width: "100%" }}>
          <Input
            value={importFilePath}
            onChange={(e) => setImportFilePath(e.target.value)}
            placeholder={t("vault.import_export.import_file_placeholder")}
            readOnly
          />
          <Button
            icon={<FolderOpenOutlined />}
            onClick={() => void handlePickImportFile()}
          >
            {t("common.actions.browse")}
          </Button>
        </Space.Compact>
        <Button
          type="primary"
          icon={<UploadOutlined />}
          loading={loading}
          disabled={!importFilePath.trim()}
          onClick={() => void handleImport()}
        >
          {t("vault.import_export.import_button")}
        </Button>
      </Space>
    </div>
  );
}
