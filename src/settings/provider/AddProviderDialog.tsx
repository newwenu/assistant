import React from "react";
import { Modal, Typography } from "antd";
import { PlusOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { type ApiFormat, type ProviderPreset } from "@/stores/providerStore";
import { getAllProviderFormats } from "../core/providerFormatRegistry";
import { getPresetsByFormat } from "../core/providerPresetRegistry";

interface AddProviderDialogProps {
  open: boolean;
  onClose: () => void;
  onPickPreset: (preset: ProviderPreset) => void;
  onAddCustom: (format: ApiFormat) => void;
}

export const AddProviderDialog: React.FC<AddProviderDialogProps> = ({
  open,
  onClose,
  onPickPreset,
  onAddCustom,
}) => {
  const { t } = useTranslation();
  const formats = getAllProviderFormats();

  return (
    <Modal
      title={t("provider.add_title")}
      open={open}
      onCancel={onClose}
      footer={null}
      width="var(--modal-width-select)"
      className="add-provider-dialog"
    >
      {formats.map((fmt) => {
        const presets = getPresetsByFormat(fmt.id);
        return (
          <div key={fmt.id} className="add-provider-section">
            <Typography.Text
              type="secondary"
              className="add-provider-section-title"
            >
              {t(fmt.label)}
              <span className="add-provider-section-desc">
                {t(fmt.description)}
              </span>
            </Typography.Text>
            <div className="add-provider-grid">
              {presets.map((preset) => (
                <button
                  key={preset.slug}
                  type="button"
                  className="provider-tile"
                  onClick={() =>
                    onPickPreset({
                      slug: preset.slug,
                      label: t(preset.label),
                      format: preset.format as ApiFormat,
                      defaultBaseUrl: preset.defaultBaseUrl,
                    })
                  }
                >
                  <div className="provider-tile-info">
                    <span className="provider-tile-name">
                      {t(preset.label)}
                    </span>
                    <span className="provider-tile-detail">
                      {
                        preset.defaultBaseUrl
                          .replace(/^https?:\/\//, "")
                          .split("/")[0]
                      }
                    </span>
                  </div>
                  <PlusOutlined className="provider-tile-icon" />
                </button>
              ))}
              <button
                type="button"
                className="provider-tile provider-tile-custom"
                onClick={() => onAddCustom(fmt.id as ApiFormat)}
              >
                <div className="provider-tile-info">
                  <span className="provider-tile-name">
                    {t("provider.custom_format", { format: t(fmt.label) })}
                  </span>
                  <span className="provider-tile-detail">
                    {t("provider.custom_format_detail", {
                      format: t(fmt.label),
                    })}
                  </span>
                </div>
                <PlusOutlined className="provider-tile-icon" />
              </button>
            </div>
          </div>
        );
      })}
    </Modal>
  );
};
