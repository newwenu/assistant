# I62 — 协议 crate 抽取与依赖边断开规格

| 项 | 值 |
|---|---|
| 状态 | 已实施（2026-11-21，T1–T5 全部落地；勾选与验收记录见 `I62-protocol-crate-tasklist.md`） |
| 日期 | 2026-11-20（事实数据复核 2026-11-21，差异见 §14） |
| 关联 | I61（存储管理器，行级豁免与 `owner` 语义修正，已落地）；`crate-architecture.md` §5 判据表；`CODE-SPLIT-PRINCIPLE.md`；`compile-time-attribution.md` |
| 影响范围 | `src-tauri/src/protocol/**`、`src-tauri/tests/{protocol_bindings,protocol_contract,protocol_dto_roundtrip}.rs`、5 个 owner crate、`src-tauri/Cargo.toml`、`src-tauri/crates/assistant-protocol/`（新建）、`package.json` 的 `contract:gen` 脚本 |
| 前端影响 | 零。`src/types/protocol.contract.ts` 路径与内容均不变 |

> 一句话：把 `src-tauri/src/protocol/dto/`（165 个 DTO、2,969 非空行）抽成叶子 crate `assistant-protocol`，依赖白名单收窄到 `serde` / `serde_json` / `ts-rs`；把 59 个域类型桥接 `impl From` 按所有者归属迁回各 owner crate；把契约生成测试随 crate 走。前端 82 个引用文件（其中 50 个用 `@/types/protocol.contract` 别名）与生成物字节零变化。

**编号约定**：`N` 现状事实、`G` 门禁、`V` 验收判据、`T` 实施切片、`X` 拒绝方案、`OQ` 开放问题。

---

## 1 定位与边界

### 1.1 本规格解决什么

1. **依赖边断开**：让 DTO 层在代码层面无法引用任何内部 crate（`assistant-agent` / `assistant-tools` / `assistant-storage` / `assistant-providers` / `assistant-config` / `assistant-vault`），使「wire 形状」与「领域模型」的耦合只剩显式桥接函数。
2. **契约生成代价的结构性下降**：当前 `npm run contract:gen` 必须编译主程序并链接集成测试二进制（N4），代价随主程序闭包增长而增长。抽 crate 后，生成只需编译一个只含 serde/ts-rs 的叶子 crate。
3. **把「边界清晰」从口头约束变成门禁**：现有 `assembly_contract` 只校验 workspace 成员与依赖方向，无法阻止 DTO 层回流引用内部 crate。

### 1.2 本规格不解决什么

- **不改前端契约消费形态**：`src/types/protocol.contract.ts` 仍是手写 `export type` 的单一事实源产物，路径与导入方式不变（N15）。不迁移到 `#[ts(export)]`（X3）。
- **不动 `src/types/events.ts` 的手写事件枚举**：事件名目前有 Rust `events.rs` 常量与前端手写两份，双写问题不属于本规格范围（OQ6）。
- **不重排 `src/protocol/events.rs`**：它依赖 `tauri::AppHandle` 与 `assistant_config::constant::WINDOW_ORB`（N14），必须留在主 app（X6）。
- **不改 DTO 的 wire 形状**：字段名、`serde` 属性、`Option` 的 `null` 语义一律不变（V4 强制）。

### 1.3 判据归位

是否值得为 `protocol/dto` 建立独立 crate，判据唯一来源是 `crate-architecture.md` §5 的四条（多消费者 / 可脱 tauri 独立测试 / 编译隔离 / 独立领域且 >300 行）。本规格只陈述事实与落地方案，不复述判据；判据命中情况见 §2 N16。

---

## 2 现状事实

| 编号 | 事实 | 证据 |
|---|---|---|
| N1 | `src-tauri/src/protocol/dto/` 共 **24** 个 `.rs`（22 个 DTO 文件 + 2 个 `mod.rs`），非空行 **2,969** 行（物理 3,218 行） | `Get-ChildItem -Recurse -Filter *.rs src\protocol\dto` |
| N2 | 含 `TS` 的 `#[derive(...)]` **165** 个；`#[ts(...)]` 属性 **43** 处；其中 `#[ts(export...)]` **0** 处 | 同上，逐行计数 |
| N3 | `mod.rs` 无逻辑，只有 `pub type Millis = i64;`、**18** 个 `pub mod` 与 **18** 个 `pub use` 聚合 | `src/protocol/dto/mod.rs` |
| N4 | 生成物 `src/types/protocol.contract.ts` 有 **165** 个 `export type`、513 非空行（物理 514 行），由 `tests/protocol_bindings.rs` 的手写 `decl` 表 + `fs::write` 产出；同一测试还写 `src/types/shortcutDefaults.generated.ts`（依赖 `assistant_config::app_conf::DEFAULT_SHORTCUT_PATTERNS`） | `tests/protocol_bindings.rs` |
| N5 | `contract:gen` = `cargo test -p assistant-vault --lib export_bindings` + `cargo test -p assistant-app --test protocol_bindings`（脚本整体前置 `cd src-tauri &&`）；后者需编主程序 lib 并链接集成测试二进制，2026-11-19 实测约 **117 s** | `package.json`、`compile-time-attribution.md` |
| N6 | vault 侧走原生 `#[ts(export, export_to = "vault.contract.ts")]` 共 **48** 处，靠全局单值 `TS_RS_EXPORT_DIR` 定位输出目录 | `crates/assistant-vault/src/**`、`src/vault/core/contract/vault.contract.ts` |
| N7 | DTO 结构体字段**零**内部 crate 类型；对内部 crate 的依赖集中在 **61** 个 `impl From<...>`（含 2 个 `From<&str>` 无依赖）+ **1** 个固有方法 `AgentModeDto::from_row`；`use assistant_*` 仅 **13** 行 | 见 §4.4 桥接归属表 |
| N8 | 按所有者分布：`agent` 24、`tools` 15、`providers` 8、`storage` 8、`config` 4（合计 59 需搬迁）；其中 `tools` 的 15 条含 2 条反向桥接（`Dto → 领域类型`） | 同上 |
| N9 | `src/protocol/dto/` 内**零** `crate::` 引用（无对 app 其它模块的反向依赖），仅用 `super::` 做 crate 内相对引用（9 行，分布于 6 个文件） | `grep '^use crate::\|^use super::'` |
| N10 | 子 crate 对 `protocol::dto` 的引用数 **0**（单向：app → dto，dto → 内部 crate） | `grep -r 'protocol::dto' crates/` = 0 |
| N11 | app 侧 **52** 个文件、**84** 处引用 `crate::protocol`，其中 **9** 处是 `crate::protocol::dto::*` 子模块全路径 | 同上 |
| N12 | 3 个集成测试用 `assistant_app::protocol::*`：`protocol_bindings` / `protocol_contract` / `protocol_dto_roundtrip`；T3 后 `protocol_bindings` 迁入 crate，剩 2 个经垫片解析 | `src-tauri/tests/` |
| N13 | `ts-rs` 在 app 内仅被 `protocol/dto/**` 使用，非 dto 源码零引用（非 dto 的 `ts_rs` 命中数为 0，唯一使用者是待迁的 `tests/protocol_bindings.rs`） | `grep 'ts_rs\|derive(.*TS' src/` |
| N14 | `protocol/events.rs` 无 `use` 行，全路径引用 `tauri::AppHandle` 与 `assistant_config::constant::WINDOW_ORB` | `src/protocol/events.rs` |
| N15 | 前端 **82** 个 `.ts/.tsx` 文件、**84** 处引用 `protocol.contract`（其中 **50** 个文件用 `@/types/protocol.contract` 别名，其余为 `../../types/protocol.contract` 等相对路径）；`coreBoundary` 门禁禁止 `src/features/**` 引用 `@/types/protocol.contract` | `grep 'protocol.contract' src/`、`coreBoundary.test.ts` |
| N16 | `crate-architecture.md` §5 判据命中情况：① 多消费者迁移前**不命中**（仅 app），迁移后 5 个 owner crate 新增对 `assistant-protocol` 的依赖，**① 转为命中**；② 可脱 tauri 独立测试 **命中**；③ 编译隔离 **命中**（闭包 476 单位）；④ 独立领域 >300 行 **命中**（2,969 行）。迁移前命中三条，迁移后四条全中 | 本文 §2 + `crate-architecture.md` |
| N17 | `crate-architecture.md` §5 该行仍写「18 文件 2,112 行 / 维持不抽」，与 N1 已过期；`compile-time-attribution.md` 第 6 条称「ts-rs 类型生成仅服务于 vault DTO」，与 app `Cargo.toml` 已含 `ts-rs` 冲突 | 两文档对应行 |

---

## 3 目标结构

### 3.1 三个产物

```
src-tauri/
├── crates/assistant-protocol/          # 新建：叶子 crate
│   ├── Cargo.toml                      # 依赖白名单：serde / serde_json / ts-rs
│   ├── src/lib.rs                      # 原 dto/mod.rs 内容（Millis + 18 个 mod + pub use）
│   ├── src/{agent,agent_mode,branch,clock,config,error,mascot,mcp,orb,os_tool,provider,screenshot,shortcut,storage,theme,window,workspace}.rs
│   ├── src/chat/{mod,content,conversation,event,message,usage}.rs
│   └── tests/protocol_bindings.rs      # 从 app tests/ 迁入，路径改写
├── src/protocol/
│   ├── mod.rs                          # 垫片：pub use assistant_protocol as dto; pub use assistant_protocol::*; pub mod events;
│   └── events.rs                       # 原地不动（tauri 依赖）
└── crates/assistant-{agent,tools,storage,providers,config}/src/dto_bridge.rs   # 桥接落位
```

> 文件清单口径：I63 的 P1-2（`dto/settings.rs` 拆 5 文件）已落地，现网已为 `agent_mode` / `mascot` / `provider` / `shortcut` / `theme` 五文件形态，`settings.rs` 不复存在；另有 `window.rs` 与 `chat/content.rs`。上表按迁移前的实际 24 文件列出。

### 3.2 依赖方向

```
assistant-protocol  ──▶  serde / serde_json / ts-rs        （唯一允许的 3 条出边）
assistant-app       ──▶  assistant-protocol                （新增 1 条）
assistant-{agent,tools,storage,providers,config} ──▶ assistant-protocol   （桥接需要，新增）
```

- **禁止**：`assistant-protocol` 出边指向任何 `assistant-*` 内部 crate（G2）。
- **禁止**：owner crate 之间因桥接新增横向依赖；桥接只允许 `owner → protocol` 单向。
- **保持**：owner crate 之间既有依赖图不变。

### 3.3 垫片语义

`src/protocol/mod.rs` 替换为：

```rust
pub use assistant_protocol as dto;
pub use assistant_protocol::*;

pub mod events;
pub use events::*;
```

- `crate::protocol::AgentModeDto` 仍可解析（N11 的 75 处简路径零改动）。
- `crate::protocol::dto::chat::MessageDto` 仍可解析（N11 的 9 处全路径零改动）。
- `assistant_app::protocol::*` 对 2 个未迁出的集成测试仍可解析（N12 零改动）。
- `pub use assistant_protocol as dto` 使 `dto` 成为 crate 别名而非 module；路径解析等价，但 `pub use` 不导出 `dto::` 下的私有项（原本也无私有项）。

---

## 4 依赖边断开的三层

三层必须同时成立，缺一层都会留下回流通道。

### 4.1 L1 模块级：`dto/` 内零内部 crate 标识符

- 迁移后 `assistant-protocol/src/**` 内**不允许**出现任何 `assistant_` 前缀的 crate 路径（G1）。
- 现状 13 行 `use assistant_*`（N7）与 59 个依赖内部 crate 的 `impl From` 全部迁出；另 2 个 `From<&str>`（无内部依赖）随 DTO 留在 crate 内。迁移后内部 crate 标识符归零。
- **注释例外**：以下 doc 注释提到内部 crate 路径，属说明性文本，不构成依赖——`storage.rs:70`（`assistant_agent::multimodal::ATTACHMENT_LIMITS` 说明数值来源）、`agent.rs:17`（「与 `assistant_tools::ToolTier` 同形」）、`agent.rs:356`（「与 `assistant_tools::PermissionIntent` 同形」）。G1 的扫描器必须跳过 `//` 与 `///` 起始的行，否则误报（N7 的 2,969 行里注释占比高，不能靠全文匹配）。

### 4.2 L2 crate 级：`Cargo.toml` 依赖白名单

```toml
[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
ts-rs = { version = "12", features = ["serde-json-impl", "no-serde-warnings"] }

[dev-dependencies]
# 生成测试只写文件，不需要额外依赖；如需读路径常量再按需引入
```

- 白名单外的任何 `[dependencies]` / `[dev-dependencies]` 条目都视为违规（G2）。
- `ts-rs` 的 `serde-json-impl` 是 `#[ts(type = "unknown")]` 处理 `serde_json::Value` 的前提（N2 的 43 处属性里含此用法）。
- `no-serde-warnings` 与原 app 侧配置一致，避免新增编译告警。

### 4.3 L3 生成器级：生成测试随 crate 走

- `tests/protocol_bindings.rs` 迁入 `crates/assistant-protocol/tests/`。
- 其中「扫源码收集 `#[derive(TS)]`」的根目录从 `src/protocol/dto` 改为 `src`（crate 自身源码根）。迁移后 crate 的 `src/` 只含 DTO 文件，`events.rs` 仍留主 app，不会被误收。
- 输出路径由 `CARGO_MANIFEST_DIR/../src/types/protocol.contract.ts` 改为 `CARGO_MANIFEST_DIR/../../../src/types/protocol.contract.ts`：crate 位于 `src-tauri/crates/assistant-protocol`，三级上跳即仓库根；`assistant-config` 侧的 `shortcut_defaults_bindings.rs` 同理用 `../../../src/types/shortcutDefaults.generated.ts`。不得硬编码绝对路径。
- 迁入文件内 `use assistant_app::protocol::{...}` 必须改为 `use assistant_protocol::{...}`（165 个类型名单不变）；G3 断言消息里的 `src/protocol/dto` 文案同步改为 crate 源码根。
- **`shortcutDefaults.generated.ts` 的生成必须与 DTO 生成拆开**：它依赖 `assistant_config::app_conf::DEFAULT_SHORTCUT_PATTERNS`，进入 `assistant-protocol` 会直接破坏 L1/L2。落位方案见 §5.2。

### 4.4 桥接归属表

桥接按「被转换域类型的所有者」落位。依据是 Rust 孤儿规则（E0117）：`impl From<Domain> for Dto` 仅在 `Domain` 所在 crate 或 `Dto` 所在 crate 内合法。DTO 已迁入 `assistant-protocol`，因此除 `assistant-protocol` 自身外，唯一合法落点是 `Domain` 的 owner crate。

| 落位 crate | 条目数 | 覆盖类型（示例） |
|---|---|---|
| `assistant-agent` | 24 | `MessageRecord`、`Conversation`、`TokenUsage`、`ToolCallRecord`、事件与摘要类 |
| `assistant-tools` | 15 | `ToolDescriptor`、`ToolErrorKind`、工具装配条目；含 2 条反向桥接（见下） |
| `assistant-providers` | 8 | `ProviderError`、`ProviderErrorCode`、`TokenUsage`、模型能力描述 |
| `assistant-storage` | 8 | `MessageRow`、`AgentModeRow`、`TokenUsageSummary`、存储盘点行 |
| `assistant-config` | 4 | 配置组投影（含 `DEFAULT_SHORTCUT_PATTERNS` 相关） |
| 无依赖（留在 protocol） | 2 | `impl From<&str> for AgentModeKindDto` / `ApprovalLevelDto`（`ConversationQueryModeDto` 只有固有 `as_str`，无 `From`） |

每条桥接的落位判据：**写实现文件的位置 = 被转换类型的定义 crate**。禁止把桥接写进 `assistant-app`（X2）。

**反向桥接**：现网有 2 条 `impl From<*Dto> for 领域类型`——`impl From<ToolTierDto> for ToolTier`（`agent.rs:36`）、`impl From<ApprovalLevelDto> for assistant_tools::ApprovalLevel`（`agent.rs:68`），已计入 `assistant-tools` 的 15 条。它们同样受 E0117 约束，必须落在 `assistant-tools`；G6 需双向校验，否则这类桥接可静默回流。

### 4.5 固有 impl 的特例

Rust 规定 inherent impl（`impl SomeDto { fn ... }`）只能写在 `SomeDto` 的定义 crate 内。现状 6 个固有 impl 块：`CommandErrorCodeDto::is_retryable`、`CommandErrorDto::{new,internal,with_retry_after_ms}`、`MessageStatusDto::from_db`、`AgentModeKindDto::as_str`、`ConversationQueryModeDto::as_str`、`AgentModeDto::from_row`。其中前 **5** 个无内部 crate 依赖，随 DTO 一起进入 `assistant-protocol`。

唯一例外：`AgentModeDto::from_row(row: &AgentModeRow)`。它必须改造：

```rust
// crates/assistant-storage/src/dto_bridge.rs
impl From<&AgentModeRow> for AgentModeDto {
    fn from(row: &AgentModeRow) -> Self { /* 原 from_row 函数体 */ }
}
```

- 孤儿规则合法性：`&T` 是 fundamental type，`&AgentModeRow` 覆盖为本地类型，impl 合法。
- 调用点仅 **2 处**，同在 `src/cmd/agent_mode.rs`（`row_to_dto` 包装函数与一处 `.map`），改写为 `AgentModeDto::from(row)`。
- 原 `from_row` 内的语义注释随函数体一并搬迁，不得丢失。
- 备选（若不想改调用点）：在 `assistant-app` 内加自由函数 `row_to_agent_mode_dto`。**不采用**——它把桥接留在 app，违反 §4.4 判据。

---

## 5 生成流水线

### 5.1 命令形态

```json
"contract:gen": "npm run contract:gen:vault && npm run contract:gen:protocol && npm run contract:gen:shortcuts",
"contract:gen:vault": "cd src-tauri && cargo test -p assistant-vault --lib export_bindings",
"contract:gen:protocol": "cd src-tauri && cargo test -p assistant-protocol --test protocol_bindings",
"contract:gen:shortcuts": "cd src-tauri && cargo test -p assistant-config --test shortcut_defaults_bindings"
```

- 三段可独立重跑，单段失败不掩盖其它段。
- 总命令保持 `contract:gen` 名字不变，CI 与文档引用零改动。
- 三段均前置 `cd src-tauri &&`（与现网 `contract:gen` 一致），从仓库根直接可跑；段名由 `:app` 改为 `:protocol`，与目标 crate 对齐。

### 5.2 `shortcutDefaults` 双职责拆分

`tests/protocol_bindings.rs` 现同时产出两个文件（N4）。拆分后：

- DTO 部分 → `crates/assistant-protocol/tests/protocol_bindings.rs`
- 快捷键默认值部分 → `crates/assistant-config/tests/shortcut_defaults_bindings.rs`（新文件），只读 `DEFAULT_SHORTCUT_PATTERNS` 并写 `src/types/shortcutDefaults.generated.ts`

理由：`DEFAULT_SHORTCUT_PATTERNS` 属 `assistant-config` 领域，且 `assistant-protocol` 不得依赖 `assistant-config`（§4.2）。拆分后两个生成物各归其主，任一改动只触发对应一段编译。

### 5.3 生成物路径与哈希

- `src/types/protocol.contract.ts`、`src/types/shortcutDefaults.generated.ts` 路径不变。
- `contract-check.mjs` 的 3 个哈希条目不变（G5）——口径是 **2 个文件**（`src/types/protocol.contract.ts`、`src/types/shortcutDefaults.generated.ts`）**+ 1 个目录**（`src/vault/core/contract`）。
- `contract:verify` 的 `git diff --exit-code` 不变。

---

## 6 门禁

| 编号 | 门禁 | 落位 | 失败信息要求 |
|---|---|---|---|
| G1 | `assistant-protocol` 的 `src/**` 非注释行零 `assistant_` crate 路径（排除自身 crate 名 `assistant_protocol`，不扫 `tests/**`） | `crates/assistant-protocol/tests/no_internal_deps.rs`（源码文本扫描） | 打印违规文件:行号 |
| G2 | `assistant-protocol/Cargo.toml` 依赖白名单（`serde`/`serde_json`/`ts-rs`） | 同上，解析 `Cargo.toml` | 打印越界依赖名 |
| G3 | DTO 注册无漏无多：源码 `#[derive(TS)]` 集合 == `decl` 表集合 | `protocol_bindings.rs` 现有断言，随迁 | 保留现有双集合差集输出 |
| G4 | `#[ts(export)]` 禁止出现在 `assistant-protocol` | G1 同文件 | 防止与全局单值 `TS_RS_EXPORT_DIR` 冲突（N6） |
| G5 | 3 个生成物哈希比对 | `contract-check.mjs` 原地 | 不变 |
| G6 | 桥接归属（**双向**）：每个 `impl From<X> for *Dto` 与 `impl From<*Dto> for X` 都必须位于 `X` 的 owner crate | `src-tauri/tests/assembly_contract.rs` 新增子项 | 打印桥接与所在 crate |
| G7 | workspace 成员与依赖方向（含新 crate） | `assembly_contract` 现有 b11 + 依赖方向校验 | 扩展成员清单 |
| G8 | 前端 import 路径不变 | `coreBoundary` 现有断言自动覆盖（N15） | 无需新增 |

**G6 的必要性**：没有 G6，桥接可以合法地回流到 `assistant-app`（app 同时依赖 owner crate 与 protocol，任何 `impl From` 都能编译通过），L1 的断开会被静默绕过。G6 是 L1 的唯一执行者。现网已有 2 条反向桥接（§4.4），只查正向 `impl From<X> for *Dto` 会漏掉它们。

---

## 7 前端影响

| 项 | 结论 |
|---|---|
| `src/types/protocol.contract.ts` | 内容与路径不变（V4 字节比对） |
| 82 个引用文件（N15） | 零改动 |
| `src/types/shortcutDefaults.generated.ts` | 内容与路径不变 |
| `coreBoundary` 门禁 | 自动继续生效 |
| `npm run typecheck` / `npm run test` | 应全绿（V6） |

---

## 8 迁移成本与收益

### 8.1 改动面

| 类别 | 规模 |
|---|---|
| 文件移动 | 24 个 `.rs`（`git mv`，保留历史） |
| 桥接搬家 | 59 个 `impl From` → 5 个 `dto_bridge.rs`（每个文件需 `use assistant_protocol::*;`，桥接体引用的其它 DTO 类型由此解析） |
| 固有 impl 改造 | 1 个（`from_row` → `From<&AgentModeRow>`）+ 2 处调用点 |
| 垫片 | `src/protocol/mod.rs` 重写（4 行） |
| 生成测试迁移 | 1 个文件迁入（含 `use assistant_app::protocol::*` → `use assistant_protocol::*` 改写）+ 1 个文件新拆 |
| `Cargo.toml` | 新建 1 个；app 移除 `ts-rs`（N13）；5 个 owner crate 加 `assistant-protocol` 依赖 |
| app 侧 `use` 行 | 零改动（垫片保证，N11） |

### 8.2 收益

| 项 | 前 | 后 |
|---|---|---|
| `contract:gen:protocol` 编译闭包 | 主程序全量（476 单位）+ 集成测试链接 | 叶子 crate（serde/serde_json/ts-rs） |
| 契约生成 wall time | 约 117 s（N5） | 待 V3 实测 |
| DTO 层回流内部 crate | 靠人工约定 | G1/G2/G6 三重门禁 |
| 前端改动 | — | 0 |

**与早期结论的关系**：此前「抽 crate 无编译收益」的判断，前提是桥接留在 `assistant-protocol` 内（即 §11 X1）。该前提下闭包必然包含 476 单位的 agent 闭包，抽 crate 只是把编译位置换了个地方。本规格先执行 §4 的依赖边断开，把 59 个桥接按所有者迁回 owner crate，叶子 crate 才真正只含 serde/ts-rs —— **断开依赖边是抽 crate 有收益的前置条件，不是可选优化**。

### 8.3 风险

| 风险 | 缓解 |
|---|---|
| 桥接落位判错（域类型被两个 crate 定义/重导出） | 迁移前逐条核对 `impl From` 的源类型定义位置；G6 双向兜底 |
| 垫片别名与 `pub use *` 名字冲突 | 迁移后 `cargo check` 全量验证；`events.rs` 只有 3 个函数 + 1 个 module，无与 DTO 同名项（N14） |
| `#[ts(type = "unknown")]` 在新 crate 内解析失败 | `ts-rs` 带 `serde-json-impl` feature（§4.2） |
| 生成测试的源码扫描根目录改错导致漏注册 | G3 双集合断言，漏注册直接失败 |
| 生成测试迁入后 `use assistant_app::protocol::*` 未改写 | T3 明确列为必改动作；漏改直接编译失败，不会静默 |

---

## 9 实施切片

切片按「先断开、再抽取、后收口」排序。`T1 + T2` 合并为一个可编译提交（见 T2 说明），`T3` / `T4` / `T5` 各自独立可编译、可回滚。

### T1 — 模块级断开（源码侧归零）

1. 从 `dto/**` 删除 13 行 `use assistant_*` 与 **59** 个依赖内部 crate 的 `impl From`；2 个 `From<&str>`（无内部依赖）保留原位。
2. 把固有方法 `AgentModeDto::from_row` 移出 DTO（该类型不能再持有内部 crate 知识），待 T2 以 `impl From<&AgentModeRow>` 落位 `assistant-storage`。
3. 产出「59 条桥接清单」（源类型 → 目标 DTO → 所有者 crate），作为 T2 逐条落位的核对依据。

### T2 — 一次性落位（crate 抽取 + 桥接归位）

> **T1 与 T2 必须是同一个提交**。T1 结束后桥接处于「已删除未落位」状态，`cargo check` 必然失败；T2 在同提交内补齐落位。T1 的价值是把改动拆成两个可独立核对的动作（先归零、再归位），而非两个可独立发布的提交。

1. `git mv src-tauri/src/protocol/dto crates/assistant-protocol/src`，`mod.rs` → `lib.rs`（`pub mod` 与 `pub use` 原样保留，`pub type Millis` 一并迁入）。
   - 与 I63 的 P1-2（`dto/settings.rs` 拆 5 文件）已合并执行：抽取前的 `dto/` 已为 `agent_mode` / `mascot` / `provider` / `shortcut` / `theme` 五文件形态，`settings.rs` 不复存在；本次 `git mv` 搬的是这五个已拆分文件，无额外切分动作。
2. 新建 `crates/assistant-protocol/Cargo.toml`（§4.2 白名单），加入 workspace `members`。
3. 5 个 owner crate 各新建 `src/dto_bridge.rs`，按 T1 清单逐条落位 59 个 `impl From` + 1 个 `From<&AgentModeRow>`，并在各自 `lib.rs` 加 `mod dto_bridge;`（不 `pub use`：桥接是 trait impl，无需导出）。每个 `dto_bridge.rs` 需 `use assistant_protocol::*;`，桥接函数体引用的其它 DTO 类型（如 `ContentBlockDto` 内的 `ImageAttachmentRefDto`）由此解析。
4. `src/cmd/agent_mode.rs` 的 2 处 `AgentModeDto::from_row(row)` 改写为 `AgentModeDto::from(row)`。
5. `src/protocol/mod.rs` 重写为垫片（§3.3）。
6. app `Cargo.toml` 移除 `ts-rs`（N13 确认无其它使用），新增 `assistant-protocol` 依赖。

### T3 — 生成器拆分

1. `git mv tests/protocol_bindings.rs crates/assistant-protocol/tests/`，改扫描根目录与输出路径（§4.3）；文件内 `use assistant_app::protocol::{...}` 改为 `use assistant_protocol::{...}`（165 个类型名单不变）。
2. 新建 `crates/assistant-config/tests/shortcut_defaults_bindings.rs`，承接快捷键生成（§5.2），输出路径用 `CARGO_MANIFEST_DIR/../../../src/types/shortcutDefaults.generated.ts`。
3. 更新 `package.json` 为三段脚本（§5.1）。
4. `tests/protocol_contract.rs` / `protocol_dto_roundtrip.rs` 保持原位（靠垫片解析，N12）。

### T4 — 门禁收口

1. 新增 `crates/assistant-protocol/tests/no_internal_deps.rs`（G1/G2/G4）。
2. `assembly_contract` 扩展：成员清单含新 crate、新增 G6 桥接归属断言（双向）。
3. `contract-check.mjs` 确认 3 条目哈希仍对齐（G5）。

### T5 — 文档与基线更新

1. `crate-architecture.md` §5 该行更新为实测数据（24 文件 / 2,969 行 / 165 DTO），并把结论从「维持不抽」改为「已抽为 `assistant-protocol`」，附判据命中情况（N16，迁移后四条全中）。
2. `compile-time-attribution.md` 第 6 条修正「ts-rs 仅服务 vault DTO」的过期表述（N17），补 `contract:gen` 新口径与实测 wall time（V3）。
3. `ARCHITECTURE.md` 的 crate 清单补 `assistant-protocol`。

---

## 10 验收判据

| 编号 | 判据 | 验证方式 |
|---|---|---|
| V1 | `assistant-protocol` 的编译闭包只含 `serde`/`serde_json`/`ts-rs` 及其传递依赖 | `cargo tree -p assistant-protocol` 人工核对，无 `assistant-*` |
| V2 | `cargo test -p assistant-protocol --test protocol_bindings` 通过，且生成物与迁移前**字节相同** | 迁移前后 `git hash-object` 比对 |
| V3 | `npm run contract:gen` 三段全部通过；记录新的 wall time 到 `compile-time-attribution.md` | 计时 |
| V4 | `src/types/protocol.contract.ts` 与 `shortcutDefaults.generated.ts` 在 `git diff` 中零变化 | `git --no-pager diff --stat` |
| V5 | G1/G2/G4 门禁在注入违规样例时失败、在正常状态通过 | 临时加一行 `use assistant_config::...` 验证报错 |
| V6 | `npm run typecheck`、`npm run test`、`cargo test --test assembly_contract` 全绿 | 直接运行 |
| V7 | app 侧 52 个引用文件（N11）除 `AgentModeDto::from_row` 的 2 处调用点外零改动 | `git --no-pager diff --stat src/` |
| V8 | 前端 82 个引用文件零改动（N15） | 同上 |
| V9 | `cargo test -p assistant-agent -p assistant-tools -p assistant-storage -p assistant-providers -p assistant-config` 全绿（桥接行为等价） | 直接运行 |
| V10 | `protocol_contract` / `protocol_dto_roundtrip` 两个集成测试零改动通过 | 直接运行 |

---

## 11 拒绝方案

| 编号 | 方案 | 拒绝理由 |
|---|---|---|
| X1 | 桥接留在 `assistant-protocol` 内（crate 依赖 5 个内部 crate） | 编译闭包直接含 476 单位的 agent 闭包，`contract:gen` 代价不降；且语义倒挂（叶子 crate 反向依赖主干），L1/L2 全部失效 |
| X2 | 桥接搬 `assistant-app`，写成自由函数 `fn to_message_dto(row: &MessageRow)` | E0117 禁止 `impl From`；退化为自由函数后 52 个引用文件需改调用点，且自由函数无法被 G6 机械校验，回流不可防 |
| X3 | DTO 全量改 `#[ts(export, export_to = ...)]`，放弃手写 `decl` 表 | `TS_RS_EXPORT_DIR` 是全局单值，165 个类型无法分目录导出；且丢失 G3 的「漏注册/多注册」双集合断言 |
| X4 | Node 侧解析 Rust 源码直接生成 `.ts` | 需重新实现 serde 的 `rename_all` / `Option` / 枚举标记语义，等价于自建 ts-rs；维护成本远高于收益 |
| X5 | 前端契约目录搬家（如 `src/protocol/`） | 82 个引用文件纯噪音改动，收益为 0（V8 明确要求零改动） |
| X6 | 把 `src/protocol/events.rs` 一并搬进 `assistant-protocol` | 依赖 `tauri::AppHandle` 与 `assistant_config::constant::WINDOW_ORB`（N14），会把 tauri 拖进叶子 crate |
| X7 | 保持现状，只加 G1 文本门禁不抽 crate | 门禁无编译收益；`contract:gen` 的 117 s 结构性问题不变（§1.1 目标 2 未达成） |

---

## 12 开放问题

| 编号 | 问题 | 建议 |
|---|---|---|
| OQ1 | `ts-rs` 是否提升到 `[workspace.dependencies]` 统一版本（现 app 与 vault 各写 `version = "12"`） | 建议统一，但可作为独立小提交，不阻塞本规格 |
| OQ2 | 是否给 `assistant-protocol` 引入 `feature = "ts"` 门控 `ts-rs`，让生产构建不编译 ts-rs | 暂不做：ts-rs 是纯 derive，编译代价已计入叶子 crate；引入 feature 会让 G2 白名单复杂化 |
| OQ3 | `protocol_contract` / `protocol_dto_roundtrip` 两个集成测试是否随桥接迁入 owner crate | 暂留 app（V10 要求零改动）；后续若 app 测试二进制变重再评估 |
| OQ4 | `Millis` 类型别名是否随 DTO 迁入 `assistant-protocol` | 是，随 `lib.rs` 一并迁移（`clock.rs` 依赖 `super::Millis`） |
| OQ5 | `crate-architecture.md` §5 判据是否补第 ⑤ 条「契约生成编译隔离」 | 建议补：本次抽取的直接动机正是这条，现有四条判据无法表达「生成链路代价」 |
| OQ6 | `src/types/events.ts` 手写事件枚举与 `events.rs` 常量的双写问题 | 不在本规格范围，另立规格 |

---

## 13 对既有文档的更新

| 文档 | 更新内容 | 时点 |
|---|---|---|
| `docs/store/crate-architecture.md` | §5 该行改为「已抽为 `assistant-protocol`」，附 N1/N16 实测数据 | T5 |
| `docs/store/compile-time-attribution.md` | 修正第 6 条过期表述；补 `contract:gen` 三段口径与实测 wall time | T5 |
| `docs/ARCHITECTURE.md` | crate 清单补 `assistant-protocol` 及其依赖白名单 | T5 |
| `docs/principle/CODE-SPLIT-PRINCIPLE.md` | 不修改（判据唯一来源，本规格只引用不复述） | — |

---

## 14 事实复核记录（2026-11-21）

设计决策（§1 目标、§9 切片顺序、§11 拒绝方案）不变；本节只记录对现状事实与规格缺口的修正，便于追溯数字变化。

| 项 | 原值 | 复核值 | 影响 |
|---|---|---|---|
| N1 文件数 / 行数 | 19 / 2,949 | 24（22 DTO + 2 `mod.rs`）/ 2,969 非空（物理 3,218） | §3.1 目录树、§8.1 改动面同步 |
| N3 `pub mod` 数 | 13 | 18（另有 18 个 `pub use`） | 仅数据修正 |
| N4 生成物行数 | 513 行 | 513 非空 / 物理 514 行 | 口径澄清 |
| N11 app 引用 | 51 文件 / 83 行，8 处全路径 | 52 文件 / 84 处，9 处 `crate::protocol::dto` | V7、X2 同步 |
| N15 前端引用 | 72 个文件（`@/` 口径） | 82 文件 / 84 处（50 个用 `@/` 别名，其余相对路径） | §7、V8、X5 同步 |
| N16 判据 ① | 不命中 | 迁移后命中（5 个 owner crate 新增依赖） | 结论强化，不改设计 |
| §3.1 目录树 | 含 `settings.rs`，缺 6 个文件 | 按 I63 P1-2 落地后的实际 24 文件重列 | 实施清单对齐 |
| §4.1 注释例外 | 仅 `storage.rs:70` | 另有 `agent.rs:17`、`agent.rs:356` | G1 扫描器需跳过 3 处 |
| §4.4 无依赖桥接 | 误写 `ConversationQueryModeDto` | 实为 `ApprovalLevelDto`（前者只有固有 `as_str`） | 落位清单修正 |
| §4.4 反向桥接 | 未提及 | 现网 2 条 `Dto → 领域类型`，已计入 tools 的 15 条 | G6 扩为双向 |
| G6 | 仅正向 `impl From<X> for *Dto` | 双向校验 | 否则反向桥接无门禁 |
| §5.1 脚本 | 缺 `cd src-tauri &&`；段名 `:app` | 三段均前置 `cd`，段名改 `:protocol` | 从仓库根直接可跑 |
| §5.3 哈希条目 | 「3 个哈希条目」 | 2 个文件 + 1 个目录（`src/vault/core/contract`） | 口径澄清 |
| T3 | 未列 import 改造 | 需把 `use assistant_app::protocol::{...}` 改为 `use assistant_protocol::{...}`，并改 G3 断言文案 | 否则迁入后编译失败 |
| T2 桥接落位 | 未列导入要求 | 每个 `dto_bridge.rs` 需 `use assistant_protocol::*;` | 否则桥接体内 DTO 引用无法解析 |