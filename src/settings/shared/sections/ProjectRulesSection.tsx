import { useEffect, useRef, useState } from "react";
import { App, Button, Empty, Select, Space, Spin, Tag, Typography } from "antd";
import { ReloadOutlined, SaveOutlined, UndoOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { createLogger } from "@/services/system/logger";
import { notifyError, notifyInfo } from "@/stores/system/notifyStore";
import { stripVerbatimPrefix } from "@/chat/core/workspaceDisplay";
import { useChatStore } from "@/stores/chat/chatStore";
import {
  permissionService,
  type ProjectPermissionStatus,
} from "@/services/tool/permissionService";
import {
  workspaceService,
  type WorkspaceEntry,
} from "@/services/desktop/workspaceService";
import { ApprovalRulesEditor } from "@/chat/tool/ApprovalRulesEditor";
import {
  areApprovalRulesEqual,
  type ApprovalRuleError,
} from "@/chat/core/toolConfig";
import type { ApprovalRuleDto } from "@/types/protocol.contract";

const log = createLogger("ProjectRulesSection");

/** 空规则：缺失状态共用同一引用，避免每次渲染都产生新数组。 */
const EMPTY_RULES: ApprovalRuleDto[] = [];

type StatusKind = "unbound" | "missing" | "valid" | "invalid";

/**
 * 已加载的项目规则：状态与所属工作区绑成一份。
 *
 * 切换工作区时请求尚未返回，若只看 `status` 会把上一个工作区的数据当成新工作区的
 * 数据显示，并允许把 A 的规则写进 B 的 `permissions.json`。
 */
interface LoadedProject {
  root: string;
  status: ProjectPermissionStatus | null;
}

/** 三态 + 未绑定：由 `exists` / `valid` 两个布尔合成，不引入额外枚举（I70 §7.5）。 */
function statusKind(
  workspaceRoot: string,
  status: ProjectPermissionStatus | null,
): StatusKind {
  if (!workspaceRoot) return "unbound";
  if (!status) return "missing";
  if (!status.exists) return "missing";
  return status.valid ? "valid" : "invalid";
}

/**
 * 项目规则编辑（I70 §7.5）。
 *
 * 与模式级共用同一个 `ApprovalRulesEditor`：两处的规则语法、校验判据与错误文案
 * 完全一致，用户不必在两套 UI 之间切换语义。区别只在于落点——这里写入工作区的
 * `.assistant/permissions.json`，文件不存在时连同目录一起新建，因此不再依赖
 * 「先用外部编辑器打开文件」这条在缺文件时无从下手的路径。
 *
 * 项目规则仍是约束层：合并时取最严，`allow` 不会放宽任何调用（§4.3）。
 */
export const ProjectRulesSection: React.FC = () => {
  const { t } = useTranslation();
  const { modal } = App.useApp();
  const [workspaces, setWorkspaces] = useState<WorkspaceEntry[]>([]);
  const [selected, setSelected] = useState<string>("");
  const [loaded, setLoaded] = useState<LoadedProject | null>(null);
  const [draft, setDraft] = useState<ApprovalRuleDto[]>(EMPTY_RULES);
  const [ruleError, setRuleError] = useState<ApprovalRuleError | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [failed, setFailed] = useState(false);
  const [tick, setTick] = useState(0);
  const selectedRef = useRef(selected);
  /**
   * 当前会话绑定的工作区。
   *
   * 设置页默认选目录簿第一个可用目录时，用户很可能编辑的并不是 orb / 主窗口
   * 当前会话实际绑定的那个工作区，规则写进了 A、会话读的是 B，表现为「规则不生效」。
   * 因此优先跟随当前会话的绑定，未绑定时才退回目录簿顺序。
   */
  const conversationRoot = useChatStore((s) => s.workspaceRoot);

  useEffect(() => {
    selectedRef.current = selected;
  }, [selected]);

  useEffect(() => {
    void workspaceService
      .list()
      .then((list) => {
        setWorkspaces(list);
        // 已失效的目录无法写入，默认选第一个仍然存在的；全失效时退回第一个。
        setSelected((current) => {
          if (current) return current;
          const alive = list.find((entry) => !entry.missing);
          const bound = conversationRoot
            ? list.find(
                (entry) =>
                  !entry.missing &&
                  stripVerbatimPrefix(entry.path) ===
                    stripVerbatimPrefix(conversationRoot),
              )
            : undefined;
          return (bound ?? alive ?? list[0])?.path ?? "";
        });
      })
      .catch(() => setWorkspaces([]));
  }, [conversationRoot]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setFailed(false);
    void permissionService
      .projectStatus(selected || null)
      .then((result) => {
        if (cancelled) return;
        setLoaded({ root: selected, status: result });
        // 缺失与非法都从空规则开始编辑：非法文件无法解析出规则，
        // 只能由用户重写后整体覆盖，因此草稿必须以空列表为基线。
        setDraft(result?.rules ?? EMPTY_RULES);
      })
      .catch(() => {
        if (!cancelled) {
          setLoaded({ root: selected, status: null });
          setFailed(true);
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [selected, tick]);

  /**
   * 只有状态与当前选中项对得上才可编辑：切换工作区的加载窗口里旧数据不可用，
   * 此时既不该展示旧文件路径，也不该允许把旧草稿写进新工作区。
   */
  const project = loaded !== null && loaded.root === selected ? loaded : null;
  const ready = project !== null;
  const status = project?.status ?? null;
  const kind = statusKind(selected, status);
  /**
   * 后端返回的是 canonical 原样路径，Windows 上带 `\\?\` 前缀。
   * 该前缀是存储与比较用的 IO 形态，只在展示侧剥离（I44 §10.2），否则界面会出现
   * `\\?\G:\...\.assistant\permissions.json` 这类不可读路径。
   */
  const displayPath = status?.path ? stripVerbatimPrefix(status.path) : null;
  const stateTag = {
    unbound: {
      color: "default",
      label: t("settings.agent_mode.project_rules.state.unbound"),
    },
    missing: {
      color: "default",
      label: t("settings.agent_mode.project_rules.state.missing"),
    },
    valid: {
      color: "success",
      label: t("settings.agent_mode.project_rules.state.valid"),
    },
    invalid: {
      color: "error",
      label: t("settings.agent_mode.project_rules.state.invalid"),
    },
  }[kind];

  const savedRules = status?.rules ?? EMPTY_RULES;
  /**
   * 非法文件解析不出规则，草稿必然等于「空规则」，但它是**待覆盖**状态：
   * 若不把它算作改动，保存按钮永久禁用，用户就没有「用空规则把全拒绝救回来」的路径。
   */
  const dirty = kind === "invalid" || !areApprovalRulesEqual(draft, savedRules);
  /**
   * 编辑器里的非法草稿不会上报（见 `ApprovalRulesEditor`），父级草稿可能仍与已保存值相等；
   * 它同样属于「有未保存的改动」，提示与撤销都应按它算，否则用户只剩手工改回去一条路。
   */
  const unsaved = dirty || ruleError !== null;
  const reload = () => setTick((value) => value + 1);

  /** 整体写入：后端先校验再落盘，成功后回传最新状态，草稿据此对齐（不再多读一次）。 */
  const writeRules = (root: string, rules: ApprovalRuleDto[]) => {
    setSaving(true);
    void permissionService
      .saveProjectRules(root, rules)
      .then((next) => {
        // 写入期间用户可能已切到别的工作区，旧结果不能覆盖新选中项的状态。
        if (selectedRef.current !== root) return;
        setLoaded({ root, status: next });
        setDraft(next.rules);
        notifyInfo(
          "settings.agent_mode.project_rules.save_ok_title",
          "settings.agent_mode.project_rules.save_ok_desc",
        );
      })
      .catch((e: unknown) => {
        log.error("saveProjectRules failed", e);
        notifyError(
          "settings.agent_mode.project_rules.save_failed_title",
          "settings.agent_mode.project_rules.save_failed_desc",
          undefined,
          typeof e === "string" ? e : String(e),
        );
      })
      .finally(() => setSaving(false));
  };

  /**
   * 保存前确认：非法文件在运行时是「全部拒绝」，整体覆盖是唯一的界面内修复路径，
   * 但原文件内容无法解析、也就无法在界面里还原，因此必须先让用户确认这次覆盖。
   */
  const save = () => {
    if (!selected || !ready) return;
    const root = selected;
    const rules = draft;
    if (kind === "invalid") {
      modal.confirm({
        title: t("settings.agent_mode.project_rules.overwrite_invalid.title"),
        content: t(
          "settings.agent_mode.project_rules.overwrite_invalid.content",
        ),
        okButtonProps: { danger: true },
        onOk: () => writeRules(root, rules),
      });
      return;
    }
    writeRules(root, rules);
  };

  return (
    <div style={{ marginTop: 32 }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 4,
        }}
      >
        <Typography.Title level={5} style={{ margin: 0 }}>
          {t("settings.agent_mode.project_rules.title")}
        </Typography.Title>
        <Space size={4}>
          <Button
            size="small"
            icon={<SaveOutlined />}
            type="primary"
            disabled={!selected || !ready || saving || ruleError !== null}
            loading={saving}
            onClick={save}
          >
            {t("settings.agent_mode.project_rules.save")}
          </Button>
          <Button
            size="small"
            icon={<UndoOutlined />}
            disabled={!ready || kind === "invalid" || !unsaved || saving}
            onClick={() => setDraft([...savedRules])}
          >
            {t("settings.agent_mode.project_rules.revert")}
          </Button>
          <Button
            size="small"
            icon={<ReloadOutlined />}
            loading={loading}
            onClick={reload}
          >
            {t("settings.agent_mode.project_rules.refresh")}
          </Button>
        </Space>
      </div>
      <Typography.Text
        type="secondary"
        style={{ fontSize: 12, display: "block", marginBottom: 8 }}
      >
        {t("settings.agent_mode.project_rules.description")}
      </Typography.Text>

      {workspaces.length === 0 ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={t("settings.agent_mode.project_rules.no_workspace")}
        />
      ) : (
        <>
          <Select
            size="small"
            value={selected || undefined}
            onChange={setSelected}
            placeholder={t(
              "settings.agent_mode.project_rules.workspace_placeholder",
            )}
            options={workspaces.map((entry) => ({
              value: entry.path,
              label: entry.name || stripVerbatimPrefix(entry.path),
            }))}
            style={{ width: "100%", marginBottom: 8 }}
          />

          {!ready ? (
            <div
              style={{
                display: "flex",
                justifyContent: "center",
                padding: "12px 0",
              }}
            >
              <Spin size="small" />
            </div>
          ) : (
            <>
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                  padding: "6px 0",
                  borderBottom: "1px solid var(--color-border-secondary)",
                }}
              >
                <Tag color={stateTag.color} style={{ marginInlineEnd: 0 }}>
                  {stateTag.label}
                </Tag>
                {unsaved && (
                  <Tag color="warning" style={{ marginInlineEnd: 0 }}>
                    {t("settings.agent_mode.project_rules.unsaved")}
                  </Tag>
                )}
                <Typography.Text
                  style={{
                    flex: 1,
                    minWidth: 0,
                    fontFamily: "var(--font-mono)",
                    fontSize: 12,
                  }}
                  ellipsis={{
                    tooltip:
                      displayPath ??
                      t("settings.agent_mode.project_rules.path_unknown"),
                  }}
                >
                  {displayPath ??
                    t("settings.agent_mode.project_rules.path_unknown")}
                </Typography.Text>
              </div>

              <Typography.Text
                type="secondary"
                style={{ fontSize: 12, display: "block", marginTop: 6 }}
              >
                {failed
                  ? t("settings.agent_mode.project_rules.load_failed")
                  : t(`settings.agent_mode.project_rules.note.${kind}`)}
              </Typography.Text>

              {/* 非法文件解析不出规则，只能靠原文定位错误；合法/缺失时不需要再看原文。 */}
              {kind === "invalid" && (
                <pre
                  style={{
                    marginTop: 8,
                    marginBottom: 0,
                    padding: "8px 10px",
                    maxHeight: 200,
                    overflow: "auto",
                    border: "1px solid var(--color-border-secondary)",
                    borderRadius: 8,
                    background: "var(--color-bg-elevated)",
                    fontFamily: "var(--font-mono)",
                    fontSize: 12,
                    lineHeight: 1.5,
                    whiteSpace: "pre",
                  }}
                >
                  {status?.content ??
                    t("settings.agent_mode.project_rules.content_empty")}
                </pre>
              )}

              <div style={{ marginTop: 8 }}>
                <ApprovalRulesEditor
                  rules={draft}
                  onChange={setDraft}
                  disabled={!selected || saving}
                  onValidationChange={setRuleError}
                />
              </div>
            </>
          )}
        </>
      )}
    </div>
  );
};
