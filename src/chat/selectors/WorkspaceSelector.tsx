import React, { useCallback, useEffect, useState, type RefObject } from "react";
import { Dropdown, Tag, Tooltip } from "antd";
import type { MenuProps } from "antd";
import {
  DownOutlined,
  ExclamationCircleOutlined,
  FolderOpenOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import {
  workspaceService,
  type WorkspaceEntry,
} from "../../services/desktop/workspaceService";
import {
  workspaceDisplay,
  stripVerbatimPrefix,
} from "../core/workspaceDisplay";
import { switchWorkspace } from "../core/workspaceSwitch";
import { onOrbCloseOverlays } from "../../orb/orbOverlays";
import { notifyError, notifyInfo } from "../../stores/system/notifyStore";
import { createLogger } from "../../services/system/logger";

const log = createLogger("WorkspaceSelector");

const PATH_KEY_PREFIX = "path:";

interface WorkspaceSelectorProps {
  /** 当前会话绑定的工作区根；空串 = 未配置（I38h §8.1）。 */
  workspaceRoot: string;
  /** turn 进行中禁用（C6.6）。 */
  disabled?: boolean;
  /** 菜单展开态回传，供面板统一关闭菜单 / Escape 拦截。 */
  openRef?: RefObject<boolean>;
  /** 切换绑定：返回 `false` 表示失败，此时保留原值（C6.7）。 */
  onSwitch: (path: string) => Promise<boolean>;
  /** 清除绑定（写回空串）。 */
  onClear: () => Promise<boolean>;
}

/**
 * 会话级工作区 chip（I38h §9.1 ~ §9.3）。
 *
 * 三态展示：未配置（警示「选择工作区」）/ 有效（显示名）/ 失效（显示名 + 失效标记）。
 * 选中后先 `register_workspace` 登记目录簿，再 `patch_conversation` 写入绑定；
 * 不做乐观更新，失败时保留原值并由 store 弹出后端错误。
 */
export const WorkspaceSelector: React.FC<WorkspaceSelectorProps> = ({
  workspaceRoot,
  disabled = false,
  openRef,
  onSwitch,
  onClear,
}) => {
  const { t } = useTranslation();
  const [entries, setEntries] = useState<WorkspaceEntry[]>([]);
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);

  const display = workspaceDisplay(workspaceRoot, entries);
  const configured = display.state !== "unconfigured";
  const displayName = configured ? display.name : t("workspace.chip.select");
  const missing = display.state === "missing";

  const refresh = useCallback(async () => {
    try {
      setEntries(await workspaceService.list());
    } catch (e) {
      log.warn("Failed to list workspaces", e);
    }
  }, []);

  const handleOpenChange = useCallback(
    (next: boolean) => {
      setOpen(next);
      if (openRef) openRef.current = next;
      if (next) void refresh();
    },
    [openRef, refresh],
  );

  /** 面板收起 / 失焦时统一关闭菜单，避免 portal 菜单留在屏幕上。 */
  useEffect(
    () =>
      onOrbCloseOverlays(() => {
        setOpen(false);
        if (openRef) openRef.current = false;
      }),
    [openRef],
  );

  const registerAndSwitch = useCallback(
    async (path: string) => {
      setBusy(true);
      try {
        const result = await switchWorkspace(
          {
            register: (p) => workspaceService.register(p),
            patch: onSwitch,
          },
          path,
        );
        if (result.kind === "register_failed") {
          log.error("register_workspace failed", result.error);
          notifyError(
            "workspace.error.invalid.title",
            "workspace.error.invalid.description",
            undefined,
            typeof result.error === "string" && result.error.trim()
              ? result.error
              : undefined,
          );
          return;
        }
        if (result.kind === "switched") {
          notifyInfo(
            "workspace.switch_ok",
            "workspace.switch_cleared_session_grants",
          );
          void refresh();
        }
      } finally {
        setBusy(false);
      }
    },
    [onSwitch, refresh],
  );

  const handleChoose = useCallback(async () => {
    try {
      const picked = await workspaceService.chooseDir();
      if (picked) await registerAndSwitch(picked);
    } catch (e) {
      log.error("choose_workspace_dir failed", e);
    }
  }, [registerAndSwitch]);

  const handleClear = useCallback(async () => {
    const ok = await onClear();
    if (ok)
      notifyInfo(
        "workspace.switch_ok",
        "workspace.switch_cleared_session_grants",
      );
  }, [onClear]);

  const handleMenuClick: MenuProps["onClick"] = useCallback(
    ({ key, domEvent }) => {
      domEvent.stopPropagation();
      if (key === "choose") {
        void handleChoose();
        return;
      }
      if (key === "clear") {
        void handleClear();
        return;
      }
      if (key.startsWith(PATH_KEY_PREFIX)) {
        void registerAndSwitch(key.slice(PATH_KEY_PREFIX.length));
      }
    },
    [handleChoose, handleClear, registerAndSwitch],
  );

  const items: MenuProps["items"] = [
    ...entries.map((entry) => ({
      key: `${PATH_KEY_PREFIX}${entry.path}`,
      label: (
        <div className="orb-workspace-option">
          <div className="orb-workspace-option-head">
            <span className="orb-workspace-option-name">{entry.name}</span>
            {entry.missing && (
              <Tag color="warning">{t("workspace.chip.missing")}</Tag>
            )}
          </div>
          <span className="orb-workspace-option-path">
            {stripVerbatimPrefix(entry.path)}
          </span>
        </div>
      ),
    })),
    ...(entries.length > 0 ? [{ type: "divider" as const }] : []),
    {
      key: "choose",
      icon: <FolderOpenOutlined />,
      label: t("workspace.action.choose"),
    },
    {
      key: "clear",
      label: t("workspace.action.clear"),
      disabled: !configured,
    },
  ];

  const chip = (
    <Dropdown
      menu={{ items, onClick: handleMenuClick }}
      trigger={["click"]}
      disabled={disabled || busy}
      open={open}
      onOpenChange={handleOpenChange}
      classNames={{ root: "orb-dropdown orb-workspace-dropdown" }}
    >
      <button
        type="button"
        className={`orb-workspace-chip${configured ? "" : " orb-workspace-chip-unset"}`}
        title={configured ? display.displayPath : undefined}
        disabled={disabled || busy}
      >
        {missing && (
          <ExclamationCircleOutlined className="orb-workspace-chip-warn" />
        )}
        <span className="orb-workspace-chip-name">{displayName}</span>
        <DownOutlined className="orb-workspace-chip-caret" />
      </button>
    </Dropdown>
  );

  if (disabled) {
    return (
      <Tooltip title={t("workspace.error.busy")}>
        <span className="orb-workspace-chip-wrap">{chip}</span>
      </Tooltip>
    );
  }
  return chip;
};
