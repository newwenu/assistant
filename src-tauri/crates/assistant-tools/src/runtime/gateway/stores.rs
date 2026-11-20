use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::approval_key::ApprovalKey;
use super::decision::AuditOutcome;

/// 授权作用域：与 [`ApprovalKey`] 正交，只回答「这份批准对谁有效」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GrantScope {
    /// 仅 `conversationId` 相同的会话。
    Session,
    /// 所有会话。
    Global,
}

/// 绝对路径判定：Unix 根、UNC（`\\\\`）、盘符（`X:\` / `X:/`）。
fn is_absolute_path(value: &str) -> bool {
    if value.starts_with('/') || value.starts_with("\\\\") {
        return true;
    }
    let bytes = value.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

/// 写入前的资源摘要：绝对路径只留末段，其余原样。
///
/// 主库是明文 SQLite（只有 vault 的 `security.db` 走 SQLCipher），把绝对路径原文
/// 落进 `permission_audit` 等于明文持久化用户目录结构。折叠成 `…/<basename>`
/// 既去掉目录层级，又保留「被操作的是哪个文件」这一追溯要素。
pub fn redact_resource(resource: &str) -> String {
    if !is_absolute_path(resource) {
        return resource.to_string();
    }
    match resource
        .rsplit(['/', '\\'])
        .find(|segment| !segment.is_empty())
    {
        Some(base) => format!("…/{}", base),
        None => resource.to_string(),
    }
}

/// 敏感变量名的子串标记（大小写不敏感），与 §7.4 的不透传词表同源。
const SECRET_NAME_MARKERS: [&str; 5] = ["KEY", "TOKEN", "SECRET", "PASSWORD", "CREDENTIAL"];

/// 需要打码「后一个 token」的开关（§10.3）。
///
/// **不含 `-p`**：`docker run -p 8080:80`、`ssh -p 2222` 里的 `-p` 是端口，
/// 打码会把审计记录毁成不可读。
const SECRET_FLAGS: [&str; 4] = ["--password", "--passwd", "--token", "--api-key"];

/// 命令行的词法级打码（I56 §10.3）：`resource` 会整条落库，密钥不得跟着进库。
///
/// 只认两种形态，不做引号解析（审计要的是「大致可读 + 不含密钥」，
/// 不是可回放的命令行）：
/// - `NAME=value` 且 `NAME` 命中敏感词表 → `value` 换成 `<redacted>`；
/// - `--password` / `--passwd` / `--token` / `--api-key` 的后一个 token → `<redacted>`。
///
/// 只作用于审计与日志；实际执行用的是原始命令行，两者在调用点分叉。
pub fn redact_secrets(resource: &str) -> String {
    let mut tokens: Vec<String> = Vec::new();
    let mut redact_next = false;
    for token in resource.split_whitespace() {
        if redact_next {
            tokens.push("<redacted>".to_string());
            redact_next = false;
            continue;
        }
        // `--token=abc` 这类内联形式先按赋值处理，否则会被误当成「开关」并吞掉下一个 token。
        if let Some((name, value)) = token.split_once('=') {
            tokens.push(if value.is_empty() || !is_secret_name(name) {
                token.to_string()
            } else {
                format!("{name}=<redacted>")
            });
            continue;
        }
        if SECRET_FLAGS
            .iter()
            .any(|flag| token.eq_ignore_ascii_case(flag))
        {
            tokens.push(token.to_string());
            redact_next = true;
            continue;
        }
        tokens.push(token.to_string());
    }
    tokens.join(" ")
}

fn is_secret_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    SECRET_NAME_MARKERS
        .iter()
        .any(|marker| upper.contains(marker))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub tool: String,
    pub action: String,
    pub resource: String,
    pub outcome: AuditOutcome,
    pub at_millis: i64,
    /// 归属会话：`None` 表示无会话的写入（ToolBox 直调、授权管理动作）。
    ///
    /// 审计要能回答「哪次对话、哪一轮」被批准，因此会话坐标必须与决策同行落库，
    /// 而不是靠时间戳事后关联（时间戳既非唯一标识也不单调）。
    #[serde(default)]
    pub conversation_id: Option<String>,
    /// 归属轮次：与 `conversation_id` 一起定位到具体一轮。
    #[serde(default)]
    pub turn_id: Option<String>,
    /// 归属迭代序号：同一轮内的第几次工具调用。
    #[serde(default)]
    pub iteration: Option<u32>,
}

impl AuditEntry {
    /// 管理动作（撤销授权 / 清除授权 / 改作用域）的固定形态：`outcome = Allowed`。
    ///
    /// 管理动作不改写业务结果，一律记 `allowed`；**禁止**用 `denied` / `bypassed`
    /// 表示管理动作。时间源与网关内部审计同一处 `now_millis()`，
    /// 命令层不得另造毫秒函数、也不得硬编码 outcome 串。
    pub fn allowed(tool: &str, action: &str, resource: &str) -> Self {
        Self {
            tool: tool.to_string(),
            action: action.to_string(),
            resource: redact_resource(resource),
            outcome: AuditOutcome::Allowed,
            at_millis: now_millis(),
            conversation_id: None,
            turn_id: None,
            iteration: None,
        }
    }

    /// ToolBox 直调（未过策略）的固定形态：`action = "none"`、`outcome = Bypassed`。
    ///
    /// 时间源与网关内部审计同一处 `now_millis()`，命令层不得另造毫秒函数。
    pub fn bypassed(tool: &str, resource: &str) -> Self {
        Self {
            tool: tool.to_string(),
            action: "none".to_string(),
            resource: redact_resource(resource),
            outcome: AuditOutcome::Bypassed,
            at_millis: now_millis(),
            conversation_id: None,
            turn_id: None,
            iteration: None,
        }
    }
}

#[async_trait]
pub trait PermissionGrantStore: Send + Sync {
    /// `conversation_id` 用于命中会话级授权；`None` 时只命中全局授权。
    async fn is_granted(&self, key: &ApprovalKey, conversation_id: Option<&str>) -> bool;
    /// 会话级授权必须带 `conversation_id`，缺失时应返回错误（不能静默落成全局）。
    async fn save_grant(
        &self,
        key: &ApprovalKey,
        scope: GrantScope,
        conversation_id: Option<&str>,
    ) -> Result<(), String>;
}

#[async_trait]
pub trait PermissionAuditStore: Send + Sync {
    async fn log_decision(&self, entry: AuditEntry) -> Result<(), String>;
}

pub(crate) fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ScopedGrant {
    scope: GrantScope,
    conversation_id: Option<String>,
    key: ApprovalKey,
}

#[derive(Debug, Default)]
pub struct InMemoryGrantStore {
    grants: Mutex<HashSet<ScopedGrant>>,
}

impl InMemoryGrantStore {
    pub fn len(&self) -> usize {
        self.grants.lock().map(|g| g.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[async_trait]
impl PermissionGrantStore for InMemoryGrantStore {
    async fn is_granted(&self, key: &ApprovalKey, conversation_id: Option<&str>) -> bool {
        let global = ScopedGrant {
            scope: GrantScope::Global,
            conversation_id: None,
            key: key.clone(),
        };
        let session = conversation_id.map(|id| ScopedGrant {
            scope: GrantScope::Session,
            conversation_id: Some(id.to_string()),
            key: key.clone(),
        });
        self.grants
            .lock()
            .map(|grants| grants.contains(&global) || session.is_some_and(|s| grants.contains(&s)))
            .unwrap_or(false)
    }

    async fn save_grant(
        &self,
        key: &ApprovalKey,
        scope: GrantScope,
        conversation_id: Option<&str>,
    ) -> Result<(), String> {
        if scope == GrantScope::Session && conversation_id.is_none() {
            return Err("session grant requires conversation id".to_string());
        }
        let grant = ScopedGrant {
            scope,
            conversation_id: conversation_id.map(str::to_string),
            key: key.clone(),
        };
        self.grants
            .lock()
            .map_err(|_| "grant store poisoned".to_string())?
            .insert(grant);
        Ok(())
    }
}

/// 子 Agent 的授权视图：读穿透到父库，写入只落在自己的局部库。
/// 委托产生的批准不得污染父会话的授权集合。
pub struct ChildGrantStore {
    parent: Arc<dyn PermissionGrantStore>,
    local: InMemoryGrantStore,
}

impl ChildGrantStore {
    pub fn new(parent: Arc<dyn PermissionGrantStore>) -> Self {
        Self {
            parent,
            local: InMemoryGrantStore::default(),
        }
    }

    pub fn local(&self) -> &InMemoryGrantStore {
        &self.local
    }
}

#[async_trait]
impl PermissionGrantStore for ChildGrantStore {
    async fn is_granted(&self, key: &ApprovalKey, conversation_id: Option<&str>) -> bool {
        self.local.is_granted(key, conversation_id).await
            || self.parent.is_granted(key, conversation_id).await
    }

    async fn save_grant(
        &self,
        key: &ApprovalKey,
        scope: GrantScope,
        conversation_id: Option<&str>,
    ) -> Result<(), String> {
        self.local.save_grant(key, scope, conversation_id).await
    }
}

#[derive(Debug, Default)]
pub struct InMemoryAuditStore {
    entries: Mutex<Vec<AuditEntry>>,
}

impl InMemoryAuditStore {
    pub fn entries(&self) -> Vec<AuditEntry> {
        self.entries.lock().map(|e| e.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl PermissionAuditStore for InMemoryAuditStore {
    async fn log_decision(&self, entry: AuditEntry) -> Result<(), String> {
        self.entries
            .lock()
            .map_err(|_| "audit store poisoned".to_string())?
            .push(entry);
        Ok(())
    }
}
