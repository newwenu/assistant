import { Button, Input, Space } from "antd";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import type { FieldRenderProps } from "../../core/types/fieldSchema";

/**
 * 目录选择输入框：手填与点选共用同一个值。
 *
 * 与 `AssetPickerField` 的区别：本组件只回填路径字符串，不做任何安装/复制，
 * 路径的合法性（绝对路径、目录存在）由后端 `AppConf::validate` 与
 * `workspace_root_from_conf` 双重把关，前端不重复校验。
 * 空串一律归一为 `null`：后端拒绝空串（`workspace_root must be a non-empty
 * absolute path`），而清空是合法操作。
 */
export function FolderPickerField({
  schema,
  value,
  onChange,
  onChangeComplete,
  disabled,
}: FieldRenderProps) {
  const { t } = useTranslation();
  const current = typeof value === "string" ? value : "";
  const placeholder =
    typeof schema.placeholder === "string" ? schema.placeholder : undefined;

  const commit = (next: string) => {
    const normalized = next.trim().length === 0 ? null : next.trim();
    onChange?.(normalized);
    onChangeComplete?.(normalized);
  };

  const handlePick = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: t("settings.general.agent_tools.workspace_root.pick_title"),
    });
    if (typeof selected !== "string") return;
    commit(selected);
  };

  return (
    <Space.Compact style={{ width: "100%" }}>
      <Input
        value={current}
        placeholder={placeholder}
        disabled={disabled}
        onChange={(e) => onChange?.(e.target.value)}
        onBlur={() => commit(current)}
        onPressEnter={() => commit(current)}
      />
      <Button
        disabled={disabled}
        onClick={() => {
          void handlePick();
        }}
      >
        {t("common.actions.browse")}
      </Button>
    </Space.Compact>
  );
}
