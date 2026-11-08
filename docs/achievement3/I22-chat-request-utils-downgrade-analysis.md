# I22 — `chat/request.rs` 与 `chat/utils.rs` 下放至 `assistant-agent` 分析

> 生成日期: 2026-10-26（同日评审修订）
> 前置: I21（`send_chat_message` Pipeline 重构，已完成）
> 范围: `cmd/chat/request.rs`（70 行）+ `cmd/chat/utils.rs`（47 行）→ `assistant-agent` crate；顺带收敛 `cmd/conversation_utils.rs` 第三份重复解析逻辑、统一 `build_provider_config` 三处调用
> 状态: **已完成（2026-10-26 实施并验证）**

---

## 0. 评审修订记录

初版文档经后端代码核查后修正以下问题：

| # | 初版论断 | 核查事实 | 修正 |
|---|---------|---------|------|
| 1 | `ModelConfig` 与 `ModelConfigDto` 字段 1:1，可等价替换 | 仅 Rust 字段层面成立，**serde 层面不兼容**：`ModelConfigDto` 带 `rename_all="camelCase"`（`protocol/dto/settings.rs:76`），`ModelConfig` 无 rename 且 `owned_by` 为必填字段（`assistant-providers/src/base.rs:168`）。存储的 `models_json` 由前端写入（`cmd/provider.rs:82-86` 存原始字符串），键为 camelCase（`ownedBy`/`contextWindow`/`supportVision`） | 用 `ModelConfig` 解析存储 JSON **必然整体失败**（`owned_by` 缺失）。初版推荐的 `resolve_context_window_from_stored`（`types/config.rs:107`）此前为**死代码**（0 调用方），snake_case 解析从未对真实数据验证。方案改为：先在 providers 内修复为 camelCase 解析（Step 0），再委托 |
| 2 | 风险缓解"两者均 `#[serde(deny_unknown_fields)]`，分化时编译报错" | 双重错误：`ModelConfig` 无该属性；且 `deny_unknown_fields` 是**运行时**反序列化检查，不是编译期检查 | 缓解手段改为契约测试（真实 camelCase 样本 round-trip），见 Step 0 |
| 3 | B1 理由"providers 不依赖 storage 以避免循环依赖" | `assistant-storage` 并不依赖 `assistant-providers`，加这条边**不会成环**；真实理由是保持 providers 分层纯净、控制依赖足迹 | 新增 **B4**（推荐）：`build_provider_config` 落位 `assistant-agent`（同时依赖两者），直接接受 `&ProviderRow`，避免 B1 的 4 参数脆签名 |
| 4 | 影响范围未提 `cmd/conversation_utils.rs` | 该文件 `resolve_context_window_from_models_json`（41 行）与 `utils.rs` 解析逻辑**完全重复**，为第三份拷贝 | 一并收敛至 providers 统一入口 |

**照初版实施会导致两类静默回归**：

- `resolve_context_window` 永远回退内置 catalog，用户配置的 context_window 丢失
- `resolve_support_vision` 恒为 `false` → 视觉模型永不 `rehydrate_images`，全部走文本降级投影，多模态静默失效且无任何报错

---

## 1. 动机

I21 重构后 `cmd/chat/` 仍保留 918 行代码，其中 `request.rs` 和 `utils.rs` 的职责本质上是 **agent 层的请求组装与结果解析**，不涉及 Tauri 命令边界或前端事件系统。下放后可获得：

| 收益 | 说明 |
|------|------|
| **架构内聚** | 操作 `StreamOutcome`/`PartialOutcome` 的函数与其类型定义共置 |
| **可测试性** | `assistant-agent` 可独立编写单元测试，无需 Tauri 运行时 |
| **职责清晰** | cmd 层仅保留 Tauri 命令入口 + 事件发射，agent 层承担业务逻辑 |
| **重复收敛** | models_json 解析从 3 份拷贝收敛为 providers 单一入口；`build_provider_config` 从 cmd 层私有变为 3 调用点统一入口 |
| **行数缩减** | `cmd/chat/` 从 918 行降至 ~800 行（-13%） |

---

## 2. 逐文件依赖审计

### 2.1 `cmd/chat/utils.rs`（47 行）— 3 个函数

| 函数 | 输入类型 | 输出类型 | 依赖 crate | Tauri 依赖 |
|------|---------|---------|-----------|-----------|
| `resolve_context_window(model, provider_row)` | `&str`, `&ProviderRow` | `u32` | `assistant_providers::base`, `serde_json` | ❌ 无 |
| `resolve_final_tokens(outcome)` | `&StreamOutcome` | `Option<i64>` | `assistant_agent::chat_runtime` | ❌ 无 |
| `resolve_partial_tokens(partial)` | `&PartialOutcome` | `Option<i64>` | `assistant_agent::chat_runtime` | ❌ 无 |

**关键发现**：

- `resolve_final_tokens` / `resolve_partial_tokens` 的参数/返回值全部是 `chat_runtime` 模块定义的类型，天然属于 `assistant-agent`
- `resolve_final_tokens` 内部 `estimate_tokens(...).unwrap()` 安全：`estimate_tokens` 仅对空串返回 `None`（`chat_runtime.rs:375`），而调用点有 `!outcome.full_content.is_empty()` 守卫
- `resolve_context_window` 通过 `crate::protocol::ModelConfigDto` 解析 `models_json`，是**前端写入格式（camelCase）与 Rust 内部类型混用的错位点**

**models_json 存储格式契约**（本次评审确认的核心事实）：

```
写入方:  前端 ProviderInput.models 原始 JSON 字符串（cmd/provider.rs:82-86 直接入库）
键格式:  camelCase —— {"id","name","ownedBy","contextWindow","supportVision"}
现解析:  ModelConfigDto（rename_all="camelCase" + deny_unknown_fields）× 4 处
         cmd/chat/utils.rs:12 / cmd/chat/request.rs:63 / cmd/provider.rs:57 / cmd/conversation_utils.rs:43
```

`assistant_providers::base::ModelConfig` 是**适配器发现层类型**（openai/anthropic/gemini `list_models` 程序构造，snake_case），不是存储格式，**不能**用于解析 models_json。

**方案**：在 `assistant-providers::types::config` 内用局部 camelCase 结构修复 `resolve_context_window_from_stored`，并新增 `resolve_support_vision_from_stored`，成为存储解析的单一入口；agent/cmd 全部委托该入口。

#### 下放后签名

```rust
// assistant-agent/src/chat_runtime.rs（追加）

pub fn resolve_final_tokens(outcome: &StreamOutcome) -> Option<i64>
pub fn resolve_partial_tokens(partial: &PartialOutcome) -> Option<i64>

// assistant-providers/src/types/config.rs（修复 + 新增，Step 0）
pub fn resolve_context_window_from_stored(model_id: &str, models_json: &str) -> u32
pub fn resolve_support_vision_from_stored(model_id: &str, models_json: &str) -> bool
```

agent 侧不再包装 `resolve_context_window`——`chat_request.rs` 构建请求时直接调用 providers 入口，`cmd/conversation_utils.rs` 亦然。

---

### 2.2 `cmd/chat/request.rs`（70 行）— 1 个 struct + 2 个函数

| 项 | 类型/签名 | 依赖 crate | Tauri 依赖 |
|----|----------|-----------|-----------|
| `ChatRequest` struct | `Vec<ChatMessage>`, `Box<dyn BaseProviderAdapter>`, `String`, `ChatOptions`, `u32`, `AgentKind` | `assistant_providers`, `assistant_agent::prompt_builder` | ❌ 无 |
| `build_chat_request(detail, mode, provider_row, model_id, attachment_store)` | async → `Result<ChatRequest, String>` | 见下方 | ⚠️ `tauri::State` |
| `resolve_support_vision(provider_row, model_id)` | → `bool` | `serde_json` | ❌ 无 |

**`build_chat_request` 内部调用链**：

```
build_chat_request()
├── prompt_builder::prepare_messages()          ← assistant-agent（已在目标 crate）
├── resolve_support_vision()                    ← 本文件私有
│   └── serde_json::from_str::<Vec<ModelConfigDto>>  ← camelCase 契约，须保留
├── multimodal::rehydrate_images()              ← assistant-agent（已在目标 crate）
├── multimodal::project_images_for_text_model() ← assistant-agent（已在目标 crate）
├── super::super::provider::build_provider_config() ← ⚠️ cmd 层
│   └── base::provider_kind_from_str() + ProviderConfig 构造
├── base::create_provider_adapter()             ← assistant-providers
├── resolve_context_window()                    ← utils.rs（同步下放）
├── AgentKind::from_mode_kind()                 ← assistant-agent
└── chat_options::build_chat_options()          ← assistant-agent
```

#### 阻碍 A：`tauri::State<'_, AttachmentStore>`

`assistant-agent` 不依赖 `tauri`，不能接受 `tauri::State`。

**解决方案**：改为接受 `&AttachmentStore`。`AttachmentStore` 定义在 `assistant-storage`（`#[derive(Clone)]`，内部 `Arc<AttachmentStoreConfig>`），`assistant-agent` 已依赖 `assistant-storage`；且 `multimodal::rehydrate_images` 本就接受 `&AttachmentStore`（当前 `&tauri::State` 依赖 `Deref` 隐式转换传入）。

```rust
// 下放后签名
pub async fn build_chat_request(
    detail: &ConversationDetail,
    mode: Option<&AgentModeRow>,
    provider_row: &ProviderRow,
    model_id: &str,
    attachment_store: &AttachmentStore,  // ← 去掉 tauri::State 包装
) -> Result<ChatRequest, String>
```

调用侧（`cmd/chat/mod.rs`）现状已经是 `app.state::<AttachmentStore>()` 后传引用，无需改动取值方式；`tauri::State<T>` 实现 `Deref<Target = T>`，语义等价。

#### 阻碍 B：`super::super::provider::build_provider_config()`

此函数在 `cmd/provider.rs:38`，从 `ProviderRow` 构造 `ProviderConfig`，是纯数据转换，无 Tauri 依赖。**调用点共 3 处**：`cmd/provider.rs:167`（list_models_cmd）、`cmd/compaction.rs:123`（execute_compaction）、`cmd/chat/request.rs:38`。

| 方案 | 位置 | 形态 | 评价 |
|------|------|------|------|
| B1: `assistant-providers::base` | providers | 4 个解构参数 | 保持 providers 不依赖 storage（分层纯净，但注意：**并非循环依赖问题**——storage 不依赖 providers，加边不成环）；缺点是逐字段解构签名脆，ProviderRow 增列即改 3 处 |
| B2: 内联 6 行 | request.rs | 复制 | 引入第 4 份拷贝，不可取 |
| B3: 闭包参数 | 签名 | 回调注入 | 签名复杂化，过度设计 |
| **B4（推荐）**: `assistant-agent::provider_config` | agent | `fn build_provider_config(&ProviderRow) -> ProviderConfig` | agent 已同时依赖 storage + providers，天然是两者的桥接层；3 个调用点全部直接传 `&row`，签名稳定 |

**B4 落位**：新建 `assistant-agent/src/provider_config.rs` 小模块（10 行 + 文档注释），`cmd/provider.rs`、`cmd/compaction.rs` 改为从 agent 导入，`chat_request.rs` 直接调用，cmd 层版本删除——统一入口，无残留。

注：`ProviderConfig` 中硬编码的 `max_tokens: 4096 / temperature: 0.7` 为存量行为（实际请求参数由 `ChatOptions` 驱动），本次原样平移，不做行为变更。

#### `resolve_support_vision` 的解析格式

下放后**不得**改用 `base::ModelConfig`（snake_case，见 §0 修订 #1），而是委托 Step 0 新增的 `assistant_providers::types::resolve_support_vision_from_stored`，保持 camelCase 契约：

```rust
fn resolve_support_vision(provider_row: &ProviderRow, model_id: &str) -> bool {
    types::resolve_support_vision_from_stored(model_id, &provider_row.models_json)
}
```

---

## 3. 影响范围

### 3.1 需修改的文件

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `assistant-providers/src/types/config.rs` | 修复 + 新增 + 测试 | `resolve_context_window_from_stored` 改 camelCase 局部结构解析；新增 `resolve_support_vision_from_stored`；补契约测试 |
| `assistant-agent/src/lib.rs` | 新增 2 个 mod | `pub mod chat_request;` `pub mod provider_config;` |
| `assistant-agent/src/chat_request.rs` | **新建** | 从 `cmd/chat/request.rs` 搬入 `ChatRequest`/`build_chat_request`/`resolve_support_vision` |
| `assistant-agent/src/provider_config.rs` | **新建** | `build_provider_config(&ProviderRow)`（方案 B4） |
| `assistant-agent/src/chat_runtime.rs` | 追加 2 个 pub fn | `resolve_final_tokens`, `resolve_partial_tokens` |
| `cmd/chat/mod.rs` | 修改 imports | `ChatRequest`/`build_chat_request` ← agent::chat_request；`resolve_final_tokens` ← agent::chat_runtime |
| `cmd/chat/stream.rs` | 修改 imports | `resolve_partial_tokens` ← agent::chat_runtime |
| `cmd/provider.rs` | 删除本地 `build_provider_config` | 改用 `assistant_agent::provider_config::build_provider_config` |
| `cmd/compaction.rs` | 修改调用 | `super::provider::build_provider_config` → `assistant_agent::provider_config::build_provider_config` |
| `cmd/conversation_utils.rs` | **删除重复函数** | 私有 `resolve_context_window_from_models_json` 收敛至 providers 入口 |
| `cmd/chat/request.rs` | **删除** | 逻辑已迁入 agent |
| `cmd/chat/utils.rs` | **删除** | 逻辑已迁入 agent |

### 3.2 不受影响的文件

| 文件 | 原因 |
|------|------|
| `cmd/chat/insert.rs` | 无 `super::`/`crate::` 对 request/utils 的引用（已核查） |
| `cmd/mod.rs` | `mod chat;` 声明不变 |
| `assistant-storage` | 无需改动 |
| `assistant-i18n` | 无需改动 |

### 3.3 models_json 解析点收敛前后

| 解析点 | 收敛前 | 收敛后 |
|--------|--------|--------|
| `cmd/chat/utils.rs` → `resolve_context_window` | ModelConfigDto 解析 + catalog 回退 | 删除，agent `chat_request.rs` 直接调 providers 入口 |
| `cmd/chat/request.rs` → `resolve_support_vision` | ModelConfigDto 解析 | 委托 `resolve_support_vision_from_stored` |
| `cmd/conversation_utils.rs` → `resolve_context_window_from_models_json` | ModelConfigDto 解析 + catalog 回退（重复实现） | 删除，直接调 `resolve_context_window_from_stored` |
| `cmd/provider.rs:57` → `list_providers` | ModelConfigDto 解析返回 DTO | **保留**（DTO 输出是前端协议职责，属于 cmd 层合法逻辑） |
| `assistant-providers/types/config.rs` | 死代码，snake_case 解析（错误） | **唯一权威入口**（camelCase + 契约测试） |

---

## 4. 下放后架构

```
assistant-agent/src/
├── lib.rs                  # +pub mod chat_request; +pub mod provider_config;
├── chat_runtime.rs         # +resolve_final_tokens(), +resolve_partial_tokens()
├── chat_request.rs         # ← 新建（从 cmd/chat/request.rs）
│   ├── pub struct ChatRequest
│   ├── pub async fn build_chat_request()
│   └── fn resolve_support_vision()         # 委托 providers
├── provider_config.rs      # ← 新建（从 cmd/provider.rs）
│   └── pub fn build_provider_config(&ProviderRow) -> ProviderConfig
├── chat_tasks.rs           # 不变
├── compaction.rs           # 不变
├── multimodal.rs           # 不变
└── prompt_builder.rs       # 不变

cmd/chat/
├── mod.rs                  # ChatRequest/build_chat_request ← agent::chat_request
│                           # resolve_final_tokens ← agent::chat_runtime
├── insert.rs               # 不变
├── stream.rs               # resolve_partial_tokens ← agent::chat_runtime
├── (request.rs 删除)
└── (utils.rs 删除)
```

### 行数变化（实测，2026-10-26 实施完成）

| 位置 | 下放前 | 下放后 | 变化 |
|------|--------|--------|------|
| `cmd/chat/mod.rs` | 279 | 276 | -3（import 收敛） |
| `cmd/chat/stream.rs` | 227 | 227 | 0（仅调用路径变更） |
| `cmd/chat/insert.rs` | 295 | 295 | 0 |
| `cmd/chat/request.rs` | 70 | **0（删除）** | -70 |
| `cmd/chat/utils.rs` | 47 | **0（删除）** | -47 |
| `cmd/provider.rs` | 209 | 199 | -10（本地 fn 删除，改用 agent 版本） |
| `cmd/conversation_utils.rs` | 52 | 40 | -12（重复函数删除） |
| `assistant-agent/chat_request.rs` | 0 | 68 | 新建 |
| `assistant-agent/provider_config.rs` | 0 | 16 | 新建 |
| `assistant-agent/chat_runtime.rs` | 380 | 409 | +29（2 个 pub fn + log 导入） |
| `assistant-providers/types/config.rs` | 117 | 199 | +82（camelCase 修复 + vision 入口 + 6 契约测试） |
| **cmd/chat/ 合计** | **918** | **798** | **-120（-13%）** |

**验证结果**：`cargo check --workspace` 通过（无新增代码警告，存量 warning 不变）；`cargo test -p assistant-providers` 新增 6 个契约测试全过；`cargo test -p assistant-agent` 47 个测试全过；改动文件 rustfmt 合规（mod.rs:274 EOF 空行与 stream.rs:205 换行为存量格式差异，未顺手全量格式化）。

---

## 5. 风险与缓解

| 风险 | 概率 | 缓解 |
|------|------|------|
| `models_json` 前端契约（camelCase）与 providers 解析再度分化 | 中 | providers 内**单一解析入口** + 契约测试（真实 camelCase 样本，覆盖 context_window/vision/回退/未知字段容忍）；分化属运行时问题，靠测试而非编译期防护 |
| `build_provider_config` 逻辑残留多版本不一致 | 低 | B4 后 cmd 层版本删除，3 调用点统一走 agent，无残留 |
| `AttachmentStore` 通过 `Deref` 传递语义偏差 | 无 | `tauri::State<T>` 实现 `Deref<Target = T>`，且 `rehydrate_images` 本就接受 `&AttachmentStore`，语义等价 |
| `chat_runtime.rs` 行数膨胀（380→~410） | 低 | 可接受；若后续继续增长可拆 `chat_runtime/outcome.rs` |
| 存量 models_json 含未知字段导致解析失败 | 低 | 新局部解析结构**不带** `deny_unknown_fields`，比旧 ModelConfigDto 路径更宽容（旧路径遇未知字段整表解析失败直接回退） |

---

## 6. 实施步骤

### Step 0: providers 存储格式修复（前置，阻塞后续全部委托）

1. `types/config.rs` 新增局部 `#[derive(Deserialize)] #[serde(rename_all = "camelCase")] struct StoredModelEntry`（`id` + `#[serde(default)] context_window` + `#[serde(default)] support_vision`，不带 `deny_unknown_fields`）
2. `resolve_context_window_from_stored` 改用该结构解析
3. 新增 `pub fn resolve_support_vision_from_stored(model_id, models_json) -> bool`
4. 补 `#[cfg(test)] mod tests`：camelCase 样本命中、catalog 回退、`context_window: 0` 过滤、vision true/false/模型缺失、未知字段容忍

### Step 1: `utils.rs` 下放 → `chat_runtime.rs`

1. `resolve_final_tokens` + `resolve_partial_tokens` 追加到 `chat_runtime.rs` 末尾（含 `use log::info;`）
2. `cmd/chat/stream.rs`：`super::utils::resolve_partial_tokens` → `assistant_agent::chat_runtime::resolve_partial_tokens`
3. `cmd/chat/mod.rs`：`use utils::resolve_final_tokens` → 并入 agent::chat_runtime use 行
4. 删除 `cmd/chat/utils.rs`，`mod.rs` 移除 `mod utils;`

### Step 2: `request.rs` 下放 + B4

1. 新建 `assistant-agent/src/chat_request.rs`：`ChatRequest` + `build_chat_request`（签名去 `tauri::State`）+ `resolve_support_vision`（委托 Step 0 入口）
2. 新建 `assistant-agent/src/provider_config.rs`：`build_provider_config(&ProviderRow)`（B4）
3. `lib.rs` 注册两个新模块
4. `cmd/provider.rs` 删除本地 fn，`list_models_cmd` 改用 agent 版本
5. `cmd/compaction.rs:123` 改用 agent 版本

### Step 3: cmd 适配 + 重复收敛

1. `cmd/chat/mod.rs`：`mod request;` 移除；import 改 agent 路径；调用点 `request::build_chat_request` → `chat_request::build_chat_request`
2. 删除 `cmd/chat/request.rs`
3. `cmd/conversation_utils.rs`：删除 `resolve_context_window_from_models_json`，改调 `resolve_context_window_from_stored`

### Step 4: 编译 + 测试验证

```sh
cargo check
cargo test --lib
```

重点：providers 契约测试、agent 全量单测、workspace 编译零 warning（关注 unused import）。

### Step 5: 文档收尾

- 本文档状态改 **已完成**，行数表回填实测值
- `REFACTOR-SPLIT-PLAN.md` 追加 I22 记录

---

## 7. 已实施方案详细设计

### 7.1 providers：存储格式统一入口（Step 0）

```rust
// assistant-providers/src/types/config.rs

/// models_json 由前端写入（cmd/provider.rs save_provider 直接入库），
/// 键为 camelCase（ownedBy/contextWindow/supportVision），与 base::ModelConfig
/// （适配器发现层的 snake_case Rust 内部类型）不同，故使用局部解析结构。
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredModelEntry {
    id: String,
    #[serde(default)]
    context_window: u32,
    #[serde(default)]
    support_vision: bool,
}

fn parse_stored_models(models_json: &str) -> Vec<StoredModelEntry> {
    serde_json::from_str(models_json).unwrap_or_default()
}

pub fn resolve_context_window_from_stored(model_id: &str, models_json: &str) -> u32 {
    let stored = parse_stored_models(models_json)
        .iter()
        .find(|m| m.id == model_id)
        .map(|m| m.context_window)
        .filter(|&c| c > 0);
    resolve_context_window(model_id, stored, None)
}

pub fn resolve_support_vision_from_stored(model_id: &str, models_json: &str) -> bool {
    parse_stored_models(models_json)
        .iter()
        .find(|m| m.id == model_id)
        .map(|m| m.support_vision)
        .unwrap_or(false)
}
```

与旧 `ModelConfigDto` 路径的语义差异仅一处（更优）：旧路径 `deny_unknown_fields` 遇前端新增字段会整表解析失败→回退 catalog；新结构忽略未知字段，仍能命中已存条目。

### 7.2 agent：`provider_config.rs`（B4）

```rust
use assistant_providers::base::{self, ProviderConfig};
use assistant_storage::providers::ProviderRow;

/// `ProviderRow`（storage）→ `ProviderConfig`（providers）桥接。
/// assistant-agent 同时依赖两者，是该转换的自然宿主；
/// providers 保持不依赖 storage（分层纯净考虑，非循环依赖约束）。
pub fn build_provider_config(row: &ProviderRow) -> ProviderConfig {
    ProviderConfig {
        kind: base::provider_kind_from_str(&row.type_, row.base_url.clone()),
        api_key_encrypted: row.api_key_encrypted.clone(),
        base_url: Some(row.base_url.clone()),
        default_model: row.default_model.clone(),
        max_tokens: 4096,
        temperature: 0.7,
    }
}
```

### 7.3 agent：`chat_request.rs`

`ChatRequest`/`build_chat_request` 原样搬入，三处适配：

- `attachment_store: &AttachmentStore`（去 `tauri::State`）
- `super::super::provider::build_provider_config` → `crate::provider_config::build_provider_config`
- `super::utils::resolve_context_window` 与 `resolve_support_vision` → 委托 `assistant_providers::types::{resolve_context_window_from_stored, resolve_support_vision_from_stored}`

### 7.4 cmd：`conversation_utils.rs` 收敛

```rust
// 删除私有 resolve_context_window_from_models_json，调用点改为：
use assistant_providers::types::resolve_context_window_from_stored;
// ...
return resolve_context_window_from_stored(model_id, &provider_row.models_json);
```

`base::get_context_window_tokens(model_id)` 兜底保留在 `resolve_context_window_for_conversation` 的 provider 缺失分支（providers 入口内部同样含 catalog 回退，两层兜底语义不变）。
