import React from "react";
import { Modal, Typography } from "antd";
import { PlusOutlined } from "@ant-design/icons";
import {
  API_FORMATS,
  PROVIDER_PRESETS,
  type ApiFormat,
  type ProviderPreset,
} from "@/stores/providerStore";

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
  return (
    <Modal
      title="添加 Provider"
      open={open}
      onCancel={onClose}
      footer={null}
      width={520}
      className="add-provider-dialog"
    >
      {API_FORMATS.map((fmt) => {
        const presets = PROVIDER_PRESETS.filter((p) => p.format === fmt.format);
        return (
          <div key={fmt.format} className="add-provider-section">
            <Typography.Text
              type="secondary"
              className="add-provider-section-title"
            >
              {fmt.label}
              <span className="add-provider-section-desc">
                {fmt.description}
              </span>
            </Typography.Text>
            <div className="add-provider-grid">
              {presets.map((preset) => (
                <button
                  key={preset.slug}
                  type="button"
                  className="provider-tile"
                  onClick={() => onPickPreset(preset)}
                >
                  <div className="provider-tile-info">
                    <span className="provider-tile-name">{preset.label}</span>
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
                onClick={() => onAddCustom(fmt.format)}
              >
                <div className="provider-tile-info">
                  <span className="provider-tile-name">自定义 {fmt.label}</span>
                  <span className="provider-tile-detail">
                    任意 {fmt.label} 端点
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