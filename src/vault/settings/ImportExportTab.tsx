import { useState } from "react";
import { Button, Input, Typography, Space, Radio, Alert, App } from "antd";
import { DownloadOutlined, UploadOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";

interface ImportExportTabProps {
  loading: boolean;
  exportVault: (format: "json" | "csv") => Promise<string | null>;
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
    const filePath = await exportVault(exportFormat);
    if (filePath) {
      message.success(
        t("vault.import_export.export_success", { path: filePath }),
      );
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
        <Input
          value={importFilePath}
          onChange={(e) => setImportFilePath(e.target.value)}
          placeholder={t("vault.import_export.import_file_placeholder")}
        />
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
