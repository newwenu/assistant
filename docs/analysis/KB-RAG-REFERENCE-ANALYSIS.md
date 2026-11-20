# AI 知识库（RAG / 长期记忆）：参考源分析

> 日期：2026-11-18｜2026-11-20（结论改命题核对、事实改时点快照、加密只保留明文与混淆两档、移除选型与范围决议）
> 范围：`references/` 已检出项目、公开开源项目、闭源产品（行为观察）
> 性质：**纯参考分析**。只描述「哪些项目可参考、可参考到哪一层、与本项目约束的冲突点」，**不含取舍决议**——采用哪条路线、首期做到哪里、何时立项，属规格职责。
> 阅读约定：§1 事实核对、§7.1 加密档位对照、§9 范围外项讨论的是结构，不随任一项目代码演进失效；§2 事实快照与 §3–§7 的项目描述是 **2026-11-20 时点记录**，不随代码与上游项目演进追改，需要最新事实时以代码与上游仓库为准。许可证状态为撰写时核对，引用前须复核各仓库 LICENSE。
> 时效与终止：本文是**一次性决策输入**。知识库规格定案后整体归档冻结、不再修改；有效内容由规格承接，事实条目作为历史快照保留。

---

## 1 事实核对

「知识库」不是一个功能，是四条可独立成链的链路：

```
入库（源接入 + 解析） → 索引（切分 + 向量/全文） → 检索（召回 + 融合 + 重排） → 注入（回合前召回 / 工具调用 + 引用溯源）
```

参考项目之间的差异几乎全在「把哪条链路做深」，而不在「有没有知识库」。按这个切法，仓内已有参考覆盖到的是**契约层与策略层**，缺的是**索引层与注入层的实现**。

七条结构性事实：

1. **能抄代码的都是服务端项目，本项目是单进程桌面应用**。Onyx、RAGFlow、Dify、MaxKB 均为 Python + 独立向量库/搜索引擎；照搬架构会直接违背「Tauri + Rust + SQLite、不引入外部服务」的既有约束（见 [ARCHITECTURE.md](../ARCHITECTURE.md) §2 技术栈表）。可抄的是字段契约、检索参数、失败语义与工具面设计，不是进程拓扑。

2. **许可证决定可抄深度，且桌面端要分发二进制**。MIT / Apache-2.0 项目可读实现、可抄结构；AGPL / GPL 项目只能做行为参考（改代码 + 分发客户端有传染风险）。这条比项目清单更重要，见 §3 分栏。

3. **闭源产品（ima 类）只提供需求与体验参照，不提供实现参照**，且无法在 `references/` 中溯源。它擅长的两件事——**内容入口的完整清单**与**引用溯源的交互形态**——恰好是开源参考覆盖不到的空白，因此与「可抄代码」分栏记录，否则评审时会被误当成可照搬架构的项目。

4. **本项目已有三个可复用资产**：SQLite（`rusqlite` + `bundled-sqlcipher`）、前端 `fuse.js` 关键词检索、provider 层三家适配。**缺两个**：embeddings 端点、检索层。这意味着知识库是「新特性 crate + 两个新增能力」，不是给现有模块加功能。

5. **embedding 来源是上游变量，向量库选型是它的下游**。embedding 来源同时决定离线能力、安装包体积、单次入库成本与检索延迟；向量库只能在其后再定。见 §7。

6. **检索与 chat 主链路的关系有现成参照**。仓内最值得抄的注入形态是 OpenHuman 的两条腿：**回合前受门控、有预算的 auto-recall** + **agent 自主调用的记忆工具**。这样检索失败只降质、不阻断主流程，与既有的「错误必须用户可感知但不得阻断」语义一致（见 [PROJECT-DESIGN-GUIDELINES.md](./PROJECT-DESIGN-GUIDELINES.md) §4）。

7. **加密不是知识库该自己解决的问题，且它的前提常被误述**。本项目存在四个存储档位而非一套策略：`assistant.db` **明文**、provider / MCP 密钥**字段级**（`machine_id` 派生）、`tool-data.db` **混淆**（I27 仅为 Draft 规格、尚未实现）、vault **真加密**。`bundled-sqlcipher` 只是编译选项，不设 key 的库就是普通 SQLite 文件。知识库与聊天历史同域，实际只涉及**明文与混淆两档**；真加密档属另一量级，不在本文范围。混淆档下派生索引库必须同档，否则混淆白做。见 §7.1。

---

## 2 本项目现状事实快照（2026-11-20）

以下为 2026-11-20 时点在仓库内核实的事实，作为「还缺什么」的核对依据（时点记录，不随代码演进追改）。

| 编号 | 事实 | 证据 |
|---|---|---|
| N1 | 规划中已登记，但明确「无规格，暂不立项」：长期记忆 / RAG 归**新层**，参考 OpenHuman `experience` / `learning` | [I36a-agent-kernel-baseline.md](../store/I36a-agent-kernel-baseline.md) §2.5 演进能力表 |
| N2 | 相邻阶段是 P8.3 Router（会话之上的全局协调、跨会话记忆），参考 DeepSeek Harness Super-agent 的 Router / Worker | [I38-agent-roadmap.md](../store/I38-agent-roadmap.md) 候选表 |
| N3 | 全仓无 FTS5 / 向量检索 / embedding 调用实现。`assistant-providers/src` 内搜 `embed` 无匹配；仅在 `assets/models-dev.json` 有 `text-embedding` 系列的模型元数据，无对应端点代码 | `assistant-providers/src`、`assistant-providers/assets/models-dev.json` |
| N4 | provider 层只有 chat / responses / messages 三类适配（openai、anthropic、gemini），**没有 embeddings 端点** | `assistant-providers/src` 模块划分见 [crate-architecture.md](../store/crate-architecture.md) §2.5 |
| N5 | SQLite 走 `rusqlite` + `bundled-sqlcipher`；主 app 自身也直接依赖 `rusqlite`，该成本不可通过关闭特性省掉 | `src-tauri/Cargo.toml` 工作区依赖声明；[crate-architecture.md](../store/crate-architecture.md) §2.4 重要事实 |
| N6 | 工具数据存储的关键词检索用的是 `LIKE`，不是 FTS；混淆只能整库（`PRAGMA key` 作用于连接） | [I27-tool-data-store-spec.md](../store/I27-tool-data-store-spec.md) §6.2 与 §「为什么整库而不是字段级」 |
| N7 | 前端已有 `fuse.js`，工具箱搜索为其先例（规格 I3（已归档））；它只做内存模糊匹配，不落盘、不跨进程 | `package.json` 依赖项 |
| N8 | 全仓 docs 无 `sqlite-vec` / `tantivy` / `fastembed` 提及，检索基建路线目前是空白 | docs 全文检索 |
| N9 | 会话/附件/工具历史/agent 模式的 SQLite 持久化已在 `assistant-storage`；会话历史本身**没有全文检索面** | [crate-architecture.md](../store/crate-architecture.md) §2.4 模块表 |
| N10 | 主库 `assistant.db` **未加密**：`init_db` 只 `Connection::open` 后建表，无 `PRAGMA key`；`bundled-sqlcipher` 仅是编译选项 | `src-tauri/crates/assistant-storage/src/db.rs` |
| N11 | 混淆档已有规格先例：固定常量密钥 + 无交互（I27，**Draft 未实现**）。vault 另走主密码派生 `DbKey` + 解锁（**已实现**），与知识库不同量级（§7.1） | [I27](../store/I27-tool-data-store-spec.md) §3、`assistant-vault/src/security_db/schema.rs` |

推论：知识库要落地，最小闭环需补齐 **embeddings 端点（N4）** 与 **检索层（N3/N8）** 两块，其余（存储、IPC、可选特性、配置组机制、混淆档位）都已有现成范式。档位约束见 §7.1。

---

## 3 参考源分栏

### 3.1 A 栏：可抄代码（MIT / Apache-2.0）

| 项目 | 形态 | 最值得拿的东西 | 与本项目的冲突 |
|---|---|---|---|
| **Dify**（已在仓内） | 知识库 + 工作流，Python + TS | **检索契约字段**：`score_threshold`、`reranking_mode`（`weighted` / `reranking_model`）、`reranking_enable`、`weights`；`Source` 带 `score` 与子块；索引节点与检索节点分离 | 实现层（`api/core/rag` 的切分/向量库）未检出，也多为 Python 生态 |
| **Onyx**（原 Danswer） | 团队自托管知识库，Python + React + Postgres/Vespa | **连接器体系**（数十种源）、权限模型、引用溯源的组织方式 | 服务端多进程 + 外部搜索引擎；权限模型对本项目过重 |
| **AnythingLLM** | 桌面版 + Docker，Node + LanceDB | **单机怎么打包向量库**：桌面端内置嵌入式向量库、工作区隔离 | Node 生态；LanceDB 的 Rust 原生版（`lancedb`）才是本项目可用的部分 |
| **RAGFlow** | 团队知识库，Python + 自研深度文档解析 | **入库质量**：PDF / 表格 / 扫描件的版面解析、可视化 chunk 预览 | 解析器为 Python 重栈；若本项目要吃复杂文档，这块绕不过，但需独立评估 |
| **Open WebUI** | 最小 RAG 闭环，Python | 文档切分 → 向量 → 引用展示的最短链路，适合做 MVP 基线 | 前端为 Svelte；仅作链路对照 |
| **Jan** | 本地 LLM 桌面端，带 RAG | 桌面端 embedding 模型的管理与下载体验 | Electron |
| **PrivateGPT / LocalGPT** | 本地 RAG 最小闭环 | 最小可跑链路 | 无桌面形态 |

> 许可证：Onyx / AnythingLLM / RAGFlow / Jan / PrivateGPT 撰写时为 MIT 或 Apache-2.0；Open WebUI 现为带品牌条款的自定义许可（早期记录为 MIT），引入前须复核。RAGFlow / FastGPT 类项目常带附加条款，逐条读。

### 3.2 B 栏：仅看行为（AGPL-3.0 / GPL-3.0）

| 项目 | 形态 | 值得看的行为 | 不能做的事 |
|---|---|---|---|
| **Khoj** | 自托管/桌面个人知识库 | Obsidian / Notion / Markdown / GitHub 接入；**语义 + 全文混合检索的配比** | 改代码 + 分发客户端有传染风险 |
| **Reor** | 纯本地笔记 + 内置嵌入 | **完全离线**形态：本地嵌入模型 + 本地向量库的组合 | 同上 |
| **MaxKB** | 团队知识库 + 应用编排 | 中文生态下「知识库与应用绑定」的产品结构 | GPL-3.0 |
| **FastGPT** | 知识库 + 工作流 | 中文场景切分与问答链路 | Apache-2.0 + 附加条款，多租户商用受限 |
| **QAnything** | 团队知识库 | 中文文档检索质量、两阶段检索 | 许可证需核实 |

### 3.3 C 栏：闭源产品（仅行为观察，无源码可溯源）

| 产品 | 值得参照的部分 | 明确不参照的部分 |
|---|---|---|
| **ima（腾讯 ima.copilot）** | 产品边界（个人库 / 团队库 / AI 记忆三者分开）；内容入口清单（网页、公众号文章、本地文件、聊天内文件）；**引用溯源交互**（每条结论可点回原文片段） | 检索参数、数据契约、存储与索引方案（闭源，反推架构只能得到猜测） |

C 栏写入文档时须标注「仅行为观察，无源码可溯源」，避免后续被误读为可实现参考。

### 3.4 已在 `references/` 检出的直接可用参考

| 项目 | 检出的部分 | 可参考到的层 | 限制 |
|---|---|---|---|
| **OpenHuman** | `crates/openhuman-core/src/memory/`（README 与子模块文档：`tree`、`tools`、`sources`、`health`、`auto_recall`、`safety.rs`、`source_scope.rs`） | **分层结构、RPC 面、工具面、策略门、失败分类** | 真正的引擎已抽出 `tinymemory`，`vendor/` 未检出该包；向量算法与存储实现拿不到 |
| **Dify** | `api/core/entities/knowledge_entities.py`、`api/core/workflow/nodes/knowledge_retrieval/`、`knowledge_index/` | **契约字段与检索参数** | `api/core/rag` 未检出 |
| **DeepSeek Harness** | `packages/session-query/session-query-sqlite` | **FTS5 派生索引库**：独立库路径、单进程 owner、opt-in 打开时机、literal phrase 查询（FTS5 语法当数据处理）、元数据过滤先于排序、snippet 长度上限 | 面向会话历史，不是知识库；可作为「索引库与主库分离」的现成论证 |
| **memos** | `filter/` 过滤引擎、`core/memoexport/`、`proto/api/ai_service.proto` | 本地笔记的**数据模型、过滤 DSL、可导出格式**；AI 能力放在独立 service + per-user 限流 + inline 内容上限的契约习惯 | 无向量 / 无 embedding，不能作检索参考 |
| **AIRI** | `packages/memory-pgvector/src/index.ts` | **仅形式参考**：记忆作为独立模块 + 事件驱动接入（`module:configure`） | 该文件目前是骨架（只注册事件、无实现），不能当实现参考 |

> 基础设施设计笔记（已归档，第 10 章）曾记录 Open WebUI 与 Cherry Studio 的对比；作为历史结论可读，但其中的星数、许可证与能力矩阵已过期，不复用。

---

## 4 形态对照：ima 类产品 → 开源对应物

按「与目标形态的距离」排序，而不是按项目热度：

| 目标形态要素 | 最接近的开源对应物 | 说明 |
|---|---|---|
| 团队共享库 + 多源连接器 + 引用溯源 | **Onyx** | 形态最像，且 MIT 可读实现 |
| 桌面单机 + 内置向量库 | **AnythingLLM**（Node） / **Reor**（AGPL） | 回答「单机怎么带向量库」；Reor 回答「完全离线怎么带嵌入模型」 |
| 本地优先个人知识库 + 混合检索 | **Khoj** | 语义与全文的配比策略最有参考价值 |
| 文档入库质量 | **RAGFlow** | PDF / 表格 / 扫描件解析与 chunk 可视化 |
| 最小可用闭环 | **Open WebUI** | 切分 → 向量 → 引用的最短链路 |
| 契约与检索参数 | **Dify**（已在仓内） | 直接对齐字段，省一轮设计评审 |

形态结论：**Onyx 最像「ima 的开源对应物」，AnythingLLM / Reor 最贴「本项目形态」，但实现层要抄的是 Rust 基建（§5），不是它们的服务端架构。**

---

## 5 基建层候选对照

上表全是 Python / Node 服务端项目，本项目是 Tauri + Rust + SQLite 单进程。可落地的候选：

| 层 | 候选 | 优点 | 代价 / 风险 |
|---|---|---|---|
| 全文检索 | **SQLite FTS5** | 与现有 `rusqlite` 同库同事务，零新增依赖 | 需实测 `bundled-sqlcipher` 是否编入 FTS5（F1）；SQLCipher 下虚表与影子表的加密行为需验证（F3 / F10） |
| 全文检索 | **tantivy** | 纯 Rust、Lucene 风格，大数据量强于 FTS5 | 自管索引目录与段合并；多一个独立索引存储 |
| 向量检索 | **sqlite-vec** | 单文件扩展，可编进 SQLite，最贴「零外部服务」 | 与 `bundled-sqlcipher` 的静态链接兼容性需验证（F2）；生态较新 |
| 向量检索 | **lancedb** | Rust 原生，AnythingLLM 在用 | 独立存储格式，与主库不同事务域 |
| 向量检索 | qdrant / milvus | 能力最强 | **需独立进程**，与「单进程桌面应用」冲突 |
| 本地 embedding | **fastembed-rs** / **candle** / `ort` | 离线可用，无 API 成本 | 模型文件体积（百 MB 级）、首启加载、打包分发（F5） |
| 远端 embedding | provider 新增 embeddings 端点 | 无体积成本、质量随模型升级 | 联网依赖；`assistant-providers` 目前无该端点（N4） |
| 融合与重排 | RRF / 加权融合 + rerank | 参数形状可对齐 Dify | 重排模型又是一次「本地 vs 远端」的同型分叉 |

索引库拓扑上，DeepSeek Harness 的 `session-query-sqlite` 给出了一个可直接引用的先例：**派生索引库单独一条路径、单进程 owner、可 opt-in 打开、查询语法当数据处理**。与「把 FTS5 虚表塞进主库」相比，它的差别是索引可重建、失败可丢弃、不污染用户数据事务。

---

## 6 机制与契约抽取

### 6.1 OpenHuman `memory` 分层（结构可抄）

| 层 | 职责 | 对应本项目落点 |
|---|---|---|
| RPC 面 | 记忆操作的对外接口（注册在独立命名空间） | 新特性 crate 的 IPC 命令层 |
| tools | 11 个 `memory_*` 工具（store / recall / forget / vector_search / hybrid_search / chunk_context / doctor 等），**按 driver 能力裁剪工具集**：未声明的能力族直接不注册 | `assistant-tools` 的工具注册；能力门控与既有「特性可选」机制天然一致 |
| guard / policy | `safety.rs`（PII / secret 擦洗）、`source_scope.rs`（按轮次限定可读知识源） | 入库前擦洗 + 检索时限定源范围；这是知识库最容易被漏掉的安全面 |
| auto_recall | **受门控、有预算的回合前召回** | 注入层的一种形态：预算化、可关闭、失败降质 |
| tree / ingest | `ingest → chunk → markdown 时间树 summarize`；检索原语 `query_source / cover_window / search_entities / drill_down / fetch_leaves` | 「分层摘要 + 叶子块」的检索模型，比扁平 top-k 更适合长文档 |
| health | `FailureCode` / `FailureClass` / `DegradedState`，`pipeline_status / doctor / retry_failed` | 入库流水线的失败分类与运维面；本项目已有 crash / 日志体系可对接 |
| sources | Notion / Obsidian 等外部源的同步与增量 reconcile | 若做外部源接入，参考其增量对账，而不是全量重建 |

### 6.2 Dify 检索契约（字段可抄）

| 字段 | 语义 | 备注 |
|---|---|---|
| `score_threshold` | 分数阈值 | 与硬编码的差别：可运行期调整 |
| `reranking_mode` | `weighted`（多路加权） / `reranking_model`（模型重排） | 与本项目「本地 vs 远端」分叉一致 |
| `reranking_enable` | 是否启用重排 | 与 mode 分离，便于降级 |
| `weights` | 语义 / 关键词权重 | 混合检索的核心旋钮 |
| `Source`（含 `score`、子块、元数据） | 返回结构 | 引用溯源的载体；UI 需要 chunk 级定位，文档级不够 |

索引节点与检索节点分离，对应本项目的「入库链路」与「查询链路」两条独立 IPC 面，而不是合成一个命令。

---

## 7 存储与 embedding 路线对照

四条路线的事实对照（不含实施计划）：

| 路线 | 组成 | 离线能力 | 安装包增量 | 检索质量 | 主要风险 |
|---|---|---|---|---|---|
| **R1 纯全文** | FTS5（+ 前端 `fuse.js` 做即时过滤） | 完全离线 | 0 | 关键词命中好，语义弱 | 同义词/改写查询召回差；对中文需分词方案 |
| **R2 全文 + 本地向量** | FTS5 + sqlite-vec + 本地 embedding | 完全离线 | 模型文件百 MB 级 | 语义 + 关键词混合，最强 | 首启加载、模型分发、体积（F5） |
| **R3 全文 + 远端向量** | FTS5 + sqlite-vec + provider embeddings | 需联网入库 | 0 | 取决于所选模型 | provider 端点缺失（N4）、批量入库的 API 成本与限流 |
| **R4 外挂向量服务** | 独立向量库进程 | — | 大 | 强 | **与单进程桌面形态冲突** |

路线间的依赖顺序：embedding 来源（本地 / 远端）是上游变量，向量库与检索质量预期都是它的下游；R1 不含 embedding，因此能覆盖「引用溯源 + 关键词召回」这两个体验主干，同时语义召回为空。

---

## 7.1 加密档位对照

「知识库要不要额外加密」常被误述为「主库已经加密，知识库要不要再加密」。本项目并不存在统一的加密策略；但对知识库这一域，实际只有两个候选档位——**明文**与**整库混淆**。真加密档（主密码派生 + 解锁）与知识库不在同一量级，不在本文讨论范围。

| 档位 | 密钥来源 | 用户交互 | 实际保护强度 | 现有实例与证据 |
|---|---|---|---|---|
| **明文** | 无 | 无 | 无 | `assistant.db`：`init_db` 只 `Connection::open` 后建表，**没有 `PRAGMA key`**（[db.rs](../../src-tauri/crates/assistant-storage/src/db.rs)） |
| **混淆** | 二进制内固定常量 | 无 | 挡住「随手 sqlite3 打开 / 字符串扫描」，不抵抗逆向与内存读取 | **规格先例**：`tool-data.db`（[I27](../store/I27-tool-data-store-spec.md) §3）；该规格为 Draft 且**尚未实现**，仓内无 `tool_data` 相关代码 |

需要明确的一条事实：`rusqlite` 虽以 `bundled-sqlcipher` 编译（[Cargo.toml](../../src-tauri/Cargo.toml) 工作区声明），但**不设 key 的库就是普通 SQLite 文件**。因此会话、消息、附件引用目前均为明文落盘——这是既有且有意的事实（vault 之外的域不进加密信任边界），不是遗漏。

### 7.1.1 两档差异

| 维度 | 明文 | 混淆 |
|---|---|---|
| 密钥来源 | 无 | 二进制内固定常量 |
| 用户交互 | 无 | 无 |
| 受门控召回的可用性 | 恒可用 | 恒可用（无解锁前置） |
| 抵抗「随手 sqlite3 打开 / 字符串扫描」 | 否 | 是 |
| 抵抗逆向与内存读取 | 否 | 否 |
| 与聊天历史档位的一致性 | 一致 | 一致（聊天库取混淆时同档） |

知识库与聊天历史同域（同为主业务数据、同受门控召回），因此档位与聊天历史保持一致是唯一自洽的选项。真加密档与知识库不相容的两条事实：一是受门控召回可能在**无任何 UI 交互时**触发，未解锁时召回只能是不可用（不是降质，而是多数场景默认失效）；二是 vault 是可选特性（`config/features.json` 只有 `vault` 一条），复用其派生逻辑会让 `--no-default-features` 构建多出一条跨可选特性的依赖边（I27 曾为同样理由明确拒绝引入 `keyring`）。

### 7.1.2 三条比「档位本身」更易被漏掉的约束

| # | 约束 | 说明 |
|---|---|---|
| E4 | **派生索引库必须与主库同档** | 若按 §5 的派生索引库拓扑，则「主库混淆 + 索引明文」等于混淆白做：FTS5 倒排索引存的就是原文 token，向量库存的是可反推原文的向量。索引库通常被当作「可重建的临时产物」而被低估，是最容易漏的一环 |
| E5 | **向量化不是脱敏** | 已有 vec2text 一类工作能从向量反推出相当比例的原文。「只存向量不存原文」不构成安全措施，加密与向量化是两个不同维度 |
| E6 | **敏感内容在入库前处理，比落盘后保护更直接** | 对应 OpenHuman 的 `safety.rs`（PII / secret 擦洗）与 `source_scope.rs`（按轮次限定可读源）。敏感内容根本不进索引，比任何加密都有效 |

---

## 8 与本项目架构的接点

只列既有范式可复用的部分，不设计具体结构：

| 接点 | 既有范式 | 说明 |
|---|---|---|
| 形态 | 新特性 crate（如 `assistant-kb`），配置组 struct 放 crate 的 `conf.rs`、DTO 放该特性契约域，改完跑 `npm run contract:gen` | 与 vault 的配置归属规则一致（`.trae/rules/project_rules.md` 类型安全规范） |
| 可选性 | `config/features.json` + Cargo feature + `feature_mounts.rs` 挂载表 | vault 已有完整先例，见 [feature-optional-binary-spec.md](../store/feature-optional-binary-spec.md) |
| 存储 | `rusqlite`（bundled-sqlcipher）+ 派生索引库独立路径 | 参考 DeepSeek Harness 的「派生库 + 单 owner」；混淆只能整库，无法字段级（N6） |
| 混淆 | 整库 `PRAGMA key`，固定常量密钥、无交互 | 与 [I27-tool-data-store-spec.md](../store/I27-tool-data-store-spec.md) 一致；档位对照见 §7.1，索引库必须同档（E4） |
| 注入 | 回合前受门控 auto-recall + agent 工具两条腿 | 与 chat 主链路分离；检索失败降质不阻断 |
| 工具面 | `assistant-tools` 注册，按能力裁剪 | 对应 OpenHuman `tools` 的「未声明能力不注册」 |
| 契约 | 检索请求/响应字段对齐 Dify | 输出 `Option` 恒在、输入 `Option` 可空 + `deny_unknown_fields` |
| 失败语义 | 入库失败分类 + `doctor` 运维面 | 复用现有 crash / 日志体系 |

---

## 9 本文范围外的项

以下项与知识库不在同一范围，本文不展开，也不作为参考对照：

| 项 | 不在范围的原因 |
|---|---|
| 多租户 / 权限体系 | 单用户桌面应用；Onyx 的权限模型在此无对应需求 |
| 团队共享库 | 需要服务端与同步语义，与本地优先冲突（跨设备同步属既有 vault 同步范式的范围） |
| 复杂版面解析（PDF 表格、扫描件 OCR） | Python 重栈（RAGFlow 类）；属独立解析器评估 |
| GraphRAG / 实体图谱 | 属检索模型演进，前置是稳定的检索质量 |
| 云端托管知识库 | 与「本地优先、可离线」定位冲突 |
| 真加密档（主密码 / 系统凭据库） | 与知识库不在同一量级；知识库只涉及明文与混淆两档（§7.1） |

---

## 10 待验证问题

级别：**前置判定**＝结论未定则对应路线不成立；**待验证**＝影响选型但不改变成立判断；**登记**＝无需技术验证，只需人工确认。

| 编号 | 级别 | 问题 | 验证方式 |
|---|---|---|---|
| F1 | 前置判定 | `bundled-sqlcipher` 构建是否编入 FTS5 | `SELECT sqlite_compileoption_used('ENABLE_FTS5')`；若未编入，需评估加 feature 或改用 tantivy |
| F2 | 前置判定 | sqlite-vec 与 bundled-sqlcipher 能否静态链接共存 | 最小 PoC：同连接内建虚表 + `PRAGMA key`。混淆档下此项不成立即意味着向量只能明文落盘，破坏 E4 |
| F3 | 前置判定 | SQLCipher 下 FTS5 虚表、影子表与 `%_content` 表是否随整库加密 | 写入含唯一标记串的内容 → 建 FTS5 索引 → 对落盘文件做字节搜索，**不得命中明文标记** |
| F10 | 前置判定 | 混淆档下的旁路残留：临时表、journal / WAL、`dbstat` 是否可能留下明文 | 复用 vault 的 `PRAGMA temp_store = MEMORY` + `cipher_memory_security = ON` 硬化先例；派生索引库若用 WAL（DeepSeek Harness 默认 `wal`），需单独验证 WAL 文件同为页级加密 |
| F4 | 待验证 | provider 新增 embeddings 端点的契约成本 | 对照现有 openai/anthropic/gemini 适配结构，评估 `capability` 与 `model_catalog` 的改动面 |
| F5 | 待验证 | 本地 embedding 模型的体积、首启耗时与分发方式 | 选定 1 个候选模型做体积与冷启测量 |
| F6 | 待验证 | 各参考项目许可证复核 | 逐仓库读 LICENSE；AGPL/GPL 项降级为行为参考 |
| F7 | 待验证 | 中文分词与 FTS5 的配合 | 评估 `simple` 分词器与 trigram 方案在中文下的召回 |
| F8 | 待验证 | 增量索引的一致性（文档更新 / 删除 / 版本） | 参考 OpenHuman `sources` 的 reconcile 与 `health` 的失败重试语义 |
| F9 | 登记 | `references/` 是否补检 Dify `api/core/rag` 与 Onyx / AnythingLLM / RAGFlow | 按 [INDEX.md](../../references/0_INDEX/INDEX.md) 的稀疏检出惯例，按组补检 |

> F1–F3、F10 是同一组问题：它们共同决定「FTS5 + sqlite-vec + 整库混淆」这条组合能否成立。若 F2 失败，向量检索只能退回独立存储（lancedb）并单独决定档位；若 F1 失败，全文检索改用 tantivy，同样需要单独论证索引目录的混淆方式。

---

## 11 `references/` 补检候选对照

按「补检成本由低到高」排列：

| 顺序 | 项目 | 可补检的部分 | 理由 |
|---|---|---|---|
| 1 | **Onyx** | 连接器目录 + 检索/引用溯源主链路 | ima 类产品最像的开源对应物，MIT |
| 2 | **AnythingLLM** | 桌面端 + 内置向量库的装配方式 | 直接回答「单机怎么带向量库」 |
| 3 | **RAGFlow** | 文档解析与 chunk 相关模块 | 入库质量的决定性环节 |
| 4 | **Khoj 或 Reor**（二选一） | 本地嵌入 + 混合检索配比 | 本地优先形态的对照，AGPL 仅行为参考 |

与 Dify 高度重叠的 MaxKB / FastGPT / QAnything：中文产品形态相近，Dify 已在仓内，补检收益递减。

ima 类闭源产品无可检出内容，不进 `references/`，只在本文件 C 栏记录行为观察结论。

---

## 12 待定案项

以下三项的答案会直接决定后续路线的可行性；定案属规格职责，本文只列选项与下游影响。

| # | 待定案项 | 两种答案 | 下游影响 |
|---|---|---|---|
| D1 | **embedding 来源** | 本地模型（离线、+百 MB 包体、首启加载） / provider 新增 embeddings 端点（零包体、需联网、批量入库有 API 成本） | 决定 §7 走 R2 还是 R3，决定 F5 是否需要做，决定入库能否离线 |
| D2 | **文档摄入范围** | 仅 Markdown / 纯文本 / 用户手写笔记 / 网页剪藏 / 附件 PDF（含表格与扫描件） | 决定是否需要 RAGFlow 类解析栈；若需要，入库质量将成为项目主成本项 |
| D3 | **`references/` 补检范围** | 按 §11 补检 4 项 / 只补前 2 项 / 全部不补 | 影响后续能检索到的实现证据量；不补则本文件的 A 栏结论只能停留在契约与行为层 |

两条事实：**D1 与 D2 相互独立**——离线 embedding 不等于离线解析，吃 PDF 也可以只走远端 embedding；反之只吃 Markdown 也可以走本地 embedding。**D2 决定入库链路与检索链路是否同批做**：需要解析栈时，两者成本量级不同。