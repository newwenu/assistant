use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::os_tool::ToolErrorKindDto;

/// 与内部 `ToolTier` 同形；内部类型不外泄到契约。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ToolTierDto {
    Sandbox,
    Os,
    External,
}

/// 取值与 I70 §3.1 的 `'none' | 'workspace_write' | 'dangerous_only' | 'all_os_tools'` 对齐；
/// 级别 → Policy 规则的映射不在本 DTO 承担。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalLevelDto {
    None,
    /// 读取与工作区写入自动执行；执行 / 删除 / MCP / 工具输出回读仍需确认（I70 §3.1）。
    WorkspaceWrite,
    DangerousOnly,
    AllOsTools,
}

impl From<&str> for ApprovalLevelDto {
    /// 未知值按 fail-closed 兜底到最严级别（全部 OS 工具都要审批）。
    fn from(value: &str) -> Self {
        match value {
            "none" => Self::None,
            "workspace_write" => Self::WorkspaceWrite,
            "dangerous_only" => Self::DangerousOnly,
            _ => Self::AllOsTools,
        }
    }
}

/// 用户规则效果（I70 §3.2）：`allow` 自动执行、`ask` 每次询问、`deny` 直接拒绝。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ApprovalRuleEffectDto {
    Allow,
    Ask,
    Deny,
}

/// 单条用户规则（I70 §3.2）：`action` 与 `resource` 用与内部规则同一套 glob 语义。
///
/// 输入 DTO：随 `ConversationToolConfigDto` 落库并回传，故 `deny_unknown_fields`
/// 只拦真正的多余键；`effect` 为封闭集合，未知值反序列化即失败（E6）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalRuleDto {
    pub action: String,
    pub resource: String,
    pub effect: ApprovalRuleEffectDto,
}

/// 空白名单即 deny-by-default：模型看不到任何工具。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationToolConfigDto {
    pub max_tool_tier: ToolTierDto,
    pub approval_level: ApprovalLevelDto,
    pub enabled_tools: Vec<String>,
    pub disabled_tools: Vec<String>,
    /// 用户规则层（I70 §3.2）：空列表 = 不额外干预；旧 JSON 无此键时按 `[]` 解析。
    #[serde(default)]
    pub approval_rules: Vec<ApprovalRuleDto>,
}

impl Default for ConversationToolConfigDto {
    /// 未下发配置时的 fail-closed 兜底（I38d §4.4）：名单留空，不自动全量注入。
    fn default() -> Self {
        Self {
            max_tool_tier: ToolTierDto::Sandbox,
            approval_level: ApprovalLevelDto::None,
            enabled_tools: Vec::new(),
            disabled_tools: Vec::new(),
            approval_rules: Vec::new(),
        }
    }
}

/// `ToolConfigPanel` 渲染用（I38d §4.7）：不下发 `inputSchema`；`companionOf` 非 null
/// 表示伴随工具（I58 §3.6），归「超级工具」分组、勾选镜像基工具。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDescriptorDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tier: ToolTierDto,
    pub companion_of: Option<String>,
}

/// 渲染与历史重载的唯一事实源（I46 compact §3.4）。
///
/// 输出 DTO：`Option` 键恒在、值为 `null`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolResultCompactDto {
    pub head: String,
    pub truncated: Option<TruncatedDto>,
    pub persisted: Option<PersistedResultDto>,
    pub error_kind: Option<ToolErrorKindDto>,
    pub anchor: Option<DocAnchorDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum TruncatedUnitDto {
    Lines,
    Items,
    Bytes,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TruncatedDto {
    pub unit: TruncatedUnitDto,
    pub returned: Option<u64>,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistedResultDto {
    pub path: String,
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocAnchorDto {
    pub path: String,
    pub ranges: Vec<DocLineRangeDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocLineRangeDto {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentToolCallDto {
    pub conversation_id: String,
    pub turn_id: String,
    pub iteration: u32,
    pub call_id: String,
    pub name: String,
    pub payload: AgentToolCallPayloadDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum AgentToolCallPayloadDto {
    Started {
        /// 参数摘要（`Started` 首帧只带摘要，完整参数走 `ArgsDelta` 或终态）。
        #[serde(rename = "argsPreview")]
        args_preview: Option<String>,
    },
    ArgsDelta {
        delta: String,
    },
    Finished {
        result: String,
        #[serde(rename = "confirmationWaitMs")]
        confirmation_wait_ms: Option<u64>,
        compact: Option<ToolResultCompactDto>,
    },
    Failed {
        error: String,
        #[serde(rename = "errorKind")]
        error_kind: ToolErrorKindDto,
        #[serde(rename = "confirmationWaitMs")]
        confirmation_wait_ms: Option<u64>,
        compact: Option<ToolResultCompactDto>,
    },
    AwaitingApproval {
        approval: ToolApprovalRequestDto,
    },
    /// 等待被结束（超时或 turn 取消），卡片必须转为不可操作。
    ///
    /// drop 点无法区分两者，因此只表达「这张卡已失效」，不冒充「用户拒绝了」。
    ApprovalClosed {
        #[serde(rename = "timeoutMs")]
        timeout_ms: u64,
    },
    AwaitingAnswer {
        question: AgentQuestionRequestDto,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionIntentDto {
    pub action: String,
    pub resources: Vec<String>,
}

/// 卡片据此回答「能不能撤销」。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolBehaviorDto {
    pub read_only: bool,
    pub destructive: bool,
    pub open_world: bool,
}

/// 审批辅助信息的唯一包裹。新增审批信息只在此扩展，不再加 `ToolApprovalRequestDto` 顶层字段。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalContextDto {
    pub tier: ToolTierDto,
    pub behavior: ToolBehaviorDto,
    pub reasons: Vec<ApprovalReasonDto>,
    pub preview: Option<ApprovalPreviewDto>,
    pub extensions: Vec<ApprovalExtensionDto>,
}

/// 项目规则视图（I70 §7.5）：报告文件路径、解析结果与原文，并回传解析出的规则，
/// 供设置页用与模式级相同的 `ApprovalRulesEditor` 直接编辑后整体写回。
///
/// 三态由两个布尔合成：`exists = false` → 缺失；`exists && valid` → 存在且合法；
/// `exists && !valid` → 存在但非法（此时运行时整层退化为全拒绝，见 INV-4）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectPermissionStatusDto {
    /// `permissions.json` 的绝对路径；仅展示，前端不据此读写。
    pub path: String,
    pub exists: bool,
    pub valid: bool,
    /// 文件原文；缺失或不可读为 `null`。非法时用它展示用户自己写下的内容以便定位。
    pub content: Option<String>,
    /// 解析成功的规则；缺失或非法时为空列表（前端从空规则开始编辑并覆盖保存）。
    pub rules: Vec<ApprovalRuleDto>,
}

/// 「为什么问我」。封闭枚举：前端不得窄化，未知分支须能通用降级（`advisor` P6 恒不出现）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ApprovalReasonDto {
    DefaultPolicy {
        rule: String,
    },
    ApprovalLevel {
        level: ApprovalLevelDto,
    },
    ProjectRule {
        action: String,
        resource: String,
    },
    /// 用户规则命中（I70 §7.1）：与 `ProjectRule` 分开，用户规则不得伪装成项目规则。
    UserRule {
        action: String,
        resource: String,
    },
    TierCeiling {
        tier: ToolTierDto,
    },
    Advisor {
        rationale: String,
    },
}

/// 整体为 `null` 表示「未知」，与「零」可区分。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalPreviewDto {
    pub summary: String,
    pub truncated: bool,
    pub estimated: bool,
}

/// 开放检测结论：`kind` 开放、`severity` 封闭（仅用于着色）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalExtensionDto {
    pub kind: String,
    pub label: String,
    pub value: String,
    pub severity: ApprovalExtensionSeverityDto,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum ApprovalExtensionSeverityDto {
    Info,
    Caution,
    Danger,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolApprovalRequestDto {
    pub conversation_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub tool_name: String,
    #[ts(type = "unknown")]
    pub arguments: serde_json::Value,
    pub intents: Vec<PermissionIntentDto>,
    pub timeout_ms: u64,
    pub context: ApprovalContextDto,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum ToolApprovalKindDto {
    Approve,
    Deny,
    AllowSession,
    AllowAlways,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolApprovalDecisionDto {
    pub conversation_id: String,
    pub call_id: String,
    pub decision: ToolApprovalKindDto,
    #[ts(optional = nullable)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum GrantScopeDto {
    Session,
    Global,
}

/// 已授权操作（`permission_grants` 的一行）。
///
/// `grantKey` 是审批缓存主键（作用域前缀 + `ApprovalKey` 规范 JSON），撤销时原样回传；
/// 前端**不得**解析其内部结构，`scopeSummary` / `scope` / `conversationId` 均由后端归一。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionGrantDto {
    pub grant_key: String,
    pub tool: String,
    pub scope: GrantScopeDto,
    pub conversation_id: Option<String>,
    pub scope_summary: String,
    pub created_at: i64,
}

/// 工具权限审计的一行（`permission_audit`）。
///
/// 只承载决策事实：四个串字段是后端落库的原始串，前端按封闭词表映射、未命中原样展示
/// （I50 §6）。`id` 与自增序不外发（追溯面板按 `atMillis` 倒序）。`conversationId` /
/// `turnId` / `iteration` 为归属坐标，`null` = 不属于任何会话（ToolBox 直调、授权管理）。
/// `source` 为调用来源（`agent` / `toolbox` / `orchestration`），`null` = 非工具调用写入的行
/// （授权管理动作、工作区切换）；来源只用于归因，不参与任何决策判定。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct PermissionAuditEntryDto {
    pub tool: String,
    pub action: String,
    pub resource: String,
    pub outcome: String,
    pub at_millis: i64,
    pub conversation_id: Option<String>,
    pub turn_id: Option<String>,
    pub iteration: Option<u32>,
    pub source: Option<String>,
}

/// 权限审计的一页（`list_permission_audit` 的返回体）。
///
/// `hasMore` 必须由后端给出：本页条数 == `limit` 与「还有更早记录」同形，前端无法自行区分。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct PermissionAuditPageDto {
    pub entries: Vec<PermissionAuditEntryDto>,
    pub has_more: bool,
}

/// `scope = session` 时 `conversationId` 必填。
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetGrantScopeArgsDto {
    pub grant_key: String,
    pub scope: GrantScopeDto,
    #[ts(optional = nullable)]
    pub conversation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentQuestionRequestDto {
    pub conversation_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub questions: Vec<AgentQuestionDto>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentQuestionDto {
    pub question: String,
    pub header: String,
    pub options: Vec<AgentQuestionOptionDto>,
    pub multi_select: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentQuestionOptionDto {
    pub label: String,
    pub description: String,
}

/// `answers` 长度必须等于 `questions`。
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentQuestionAnswerDto {
    pub conversation_id: String,
    pub call_id: String,
    pub answers: Vec<AgentQuestionAnswerItemDto>,
}

/// 单题回答：单选也走数组（恒 0–1 项）；自由输入写 `other`，不塞进 `selected`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentQuestionAnswerItemDto {
    pub selected: Vec<String>,
    #[ts(optional = nullable)]
    pub other: Option<String>,
}

// ---------------------------------------------------------------------------
// turn 节点观测（I55 §4.2）：`agent-node-event` 的 wire 投影。
//
// 内部类型（`AgentNode` / `NodeState`）不外泄到契约，DTO 独立定义并显式转换。
// `parentTurnId` / `detail` 键恒在（值 `null`），`transition` 必填——三者都参与前端状态迁移判定。
// ---------------------------------------------------------------------------

/// 与内部 `AgentNode` 1:1（I55 §3.1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum AgentNodeDto {
    Admit,
    Prepare,
    Brake,
    Ladder,
    Model,
    PersistAssistant,
    Branch,
    BudgetSplit,
    Pipeline,
    AfterTool,
    PersistTools,
    AfterIteration,
}

/// 后端只发 `running` / `waiting` / 三个终态（I55 §4.3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum NodeStateDto {
    Pending,
    Running,
    Waiting,
    Finished,
    Error,
    Skipped,
    Cancelled,
}

/// 复用内部 `ToolStage`，不新造枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum PipelineStageDto {
    Find,
    Validate,
    Gateway,
    Execute,
    Persist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum BrakeVerdictDto {
    Continue,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum MiddlewareActionDto {
    Continue,
    Stop,
}

/// 剩余预算：每字段 `null` 表示「未配置」而非 0（I55 §4.4 末段）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BudgetRemainingDto {
    pub remaining_iterations: Option<u32>,
    pub remaining_tool_calls: Option<u32>,
    pub remaining_wall_clock_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BrakeDetailDto {
    pub verdict: BrakeVerdictDto,
    pub hook: Option<String>,
    pub budget: BudgetRemainingDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LadderDetailDto {
    pub applied: Vec<String>,
    pub dropped_messages: Option<u32>,
    pub trimmed_chars: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ModelDetailDto {
    pub attempt: u32,
    pub retry_reason: Option<String>,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub finish_reason: Option<String>,
    pub tool_call_count: u32,
}

/// 工具流水线 detail：过程形态带阶段与序号，终态形态全 `null`（I55 §4.4）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PipelineDetailDto {
    pub stage: Option<PipelineStageDto>,
    pub index: Option<u32>,
    pub total: Option<u32>,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BudgetSplitDetailDto {
    pub requested: u32,
    pub executed: u32,
    pub deferred: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BranchDetailDto {
    pub has_tool_calls: bool,
    pub wrap_up: bool,
    pub tool_call_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PersistDetailDto {
    pub rows: u32,
    pub message_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MiddlewareVerdictDto {
    pub middleware: String,
    pub action: MiddlewareActionDto,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AfterToolDetailDto {
    pub verdicts: Vec<MiddlewareVerdictDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AfterIterationDetailDto {
    pub nudges: Vec<String>,
    pub next_iteration: u32,
}

/// 按节点分派的 detail 闭集（I55 §4.4）：`admit` / `prepare` 无 detail。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AgentNodeDetailDto {
    Brake(BrakeDetailDto),
    Ladder(LadderDetailDto),
    Model(ModelDetailDto),
    Pipeline(PipelineDetailDto),
    BudgetSplit(BudgetSplitDetailDto),
    Branch(BranchDetailDto),
    Persist(PersistDetailDto),
    AfterTool(AfterToolDetailDto),
    AfterIteration(AfterIterationDetailDto),
}

/// 单条节点事件（I55 §4.2 全形）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentNodeEventDto {
    pub turn_id: String,
    pub conversation_id: String,
    pub seq: u32,
    pub node: AgentNodeDto,
    pub iteration: u32,
    pub state: NodeStateDto,
    /// `true` = 状态迁移（重置 `sinceMs`）；`false` = 过程事实（只进时间线）。
    pub transition: bool,
    pub at_ms: u64,
    pub agent_id: String,
    pub parent_turn_id: Option<String>,
    pub detail: Option<AgentNodeDetailDto>,
}
