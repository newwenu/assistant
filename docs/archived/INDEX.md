# 归档文档索引

> **只读**：本目录存放已冻结的历史阶段文档，不再修改（维护规则见 `.trae/rules/project_rules.md`）。
> 组织方式：按域分目录，文件名保持原样（`Ixx` 为原阶段编号）；原 `docs/achievement*/` 批次已合并，批次信息保留在下表。
> 目录内相对链接已按新位置一次性修正；指向源码/已删除文件的失效链接属归档时的历史原貌，不再回填。

## 域总览

| 域 | 内容 | 文件数 |
|---|---|---|
| [agent/](./agent/) | Agent 运行时、工具框架与 MCP、Agent Loop、审批、压缩与节点布局、身份与能力事实、运行时审计、沙箱与代码规模审计、Turn 观测面、长行读取与超级工具、工具三阶段状态 | 69 |
| [vault/](./vault/) | 密码箱：密钥体系、条目加密、同步、审计、生物识别、恢复与 IPC 契约 | 36 |
| [chat/](./chat/) | 对话与消息链路：服务拆分、流事件、历史、分支、重试、附件/多模态、截图提问、Token Usage | 35 |
| [ui/](./ui/) | Orb、主窗口与 mini 模式、设置系统、i18n、Mascot、时钟与布局 | 36 |
| [infra/](./infra/) | 编译与依赖、crate 架构、DTO 与协议、配置归属、命令层、日志、应用目录、拆分规划、横切分析 | 35 |

## 文档清单

### agent/ (69)

| 文档 | 原批次 | 标题 |
|---|---|---|
| [`I6-spec.md`](./agent/I6-spec.md) | achievement | I6 — Agent 自定义模式 + Prompt 模板合并 规格说明 |
| [`I6-tasklist.md`](./agent/I6-tasklist.md) | achievement | I6 — Agent 自定义模式 + Prompt 模板合并 任务清单 |
| [`I7-context-window-refactor.md`](./agent/I7-context-window-refactor.md) | achievement | I7 前置 — 上下文窗口获取改造方案 |
| [`I7-spec.md`](./agent/I7-spec.md) | achievement | I7 — Context Compaction 规格说明 |
| [`I7-tasklist.md`](./agent/I7-tasklist.md) | achievement | I7 — Context Compaction 任务清单 |
| [`I25-reasoning-override-spec.md`](./agent/I25-reasoning-override-spec.md) | achievement3 | I25 — 思考深度工具栏覆盖（Agent 模式作为默认） |
| [`agent-code-inventory.md`](./agent/agent-code-inventory.md) | achievement4 | Agent 前四阶段（P0–P4）代码规模与职责分布 |
| [`I18-agent-tool-architecture-spec.md`](./agent/I18-agent-tool-architecture-spec.md) | achievement4 | I18 — Agent Tool 框架层规格（Step 0-3、8） |
| [`I18b-tool-unified-bridge-spec.md`](./agent/I18b-tool-unified-bridge-spec.md) | achievement4 | I18b ToolBox OS 交互桥接规格 |
| [`I30-phase1-toolcall-separation-spec.md`](./agent/I30-phase1-toolcall-separation-spec.md) | achievement4 | I30 — 阶段一：ToolCall 从 Chat 构建剥离（已归档） |
| [`I31-phase1-tool-framework-spec.md`](./agent/I31-phase1-tool-framework-spec.md) | achievement4 | I31 — 阶段一工具框架规格（纯 Rust，不接入前后端，不新增工具） |
| [`I31-phase1-tool-framework-tasklist.md`](./agent/I31-phase1-tool-framework-tasklist.md) | achievement4 | I31 — P1 框架收口（T3）任务清单 |
| [`I36-agent-runtime-design.md`](./agent/I36-agent-runtime-design.md) | achievement4 | I36 — Agent 运行时架构决策记录（ADR） |
| [`I36-agent-runtime-spec.md`](./agent/I36-agent-runtime-spec.md) | achievement4 | I36 — Agent 运行时落地规格 |
| [`I36-agent-runtime-tasklist.md`](./agent/I36-agent-runtime-tasklist.md) | achievement4 | I36 — P3 运行时内核任务清单 |
| [`I38-agent-code-gaps.md`](./agent/I38-agent-code-gaps.md) | achievement4 | I38 — Agent 代码现状与缺口审计 |
| [`I38a-tool-control-layer-spec.md`](./agent/I38a-tool-control-layer-spec.md) | achievement4 | I38a — P2：工具控制层规格 |
| [`I38a-tool-control-layer-tasklist.md`](./agent/I38a-tool-control-layer-tasklist.md) | achievement4 | I38a — P2 工具控制层任务清单 |
| [`I38b-agent-loop-impl-guide.md`](./agent/I38b-agent-loop-impl-guide.md) | achievement4 | I38b Agent Loop 实施指南 |
| [`I38b-agent-loop-row-chain-fix-spec.md`](./agent/I38b-agent-loop-row-chain-fix-spec.md) | achievement4 | I38b-row-chain-fix — 行链单链不变量与多工具批次修复规格 |
| [`I38b-agent-loop-row-chain-fix-tasklist.md`](./agent/I38b-agent-loop-row-chain-fix-tasklist.md) | achievement4 | I38b-row-chain-fix — 行链单链修复任务清单 |
| [`I38b-agent-loop-spec.md`](./agent/I38b-agent-loop-spec.md) | achievement4 | I38b — P4：Agent Loop 闭环规格 |
| [`I38b-agent-loop-tasklist.md`](./agent/I38b-agent-loop-tasklist.md) | achievement4 | I38b — P4 Agent Loop 闭环任务清单 |
| [`I38c-delete-spec.md`](./agent/I38c-delete-spec.md) | achievement4 | I38c-delete — `delete`（破坏性工具） |
| [`I38c-edit-spec.md`](./agent/I38c-edit-spec.md) | achievement4 | I38c-edit — `edit`（修改工具） |
| [`I38c-glob-spec.md`](./agent/I38c-glob-spec.md) | achievement4 | I38c-glob — `glob`（只读工具） |
| [`I38c-grep-spec.md`](./agent/I38c-grep-spec.md) | achievement4 | I38c-grep — `grep`（只读工具） |
| [`I38c-ls-spec.md`](./agent/I38c-ls-spec.md) | achievement4 | I38c-ls — `ls`（只读工具） |
| [`I38c-mkdir-spec.md`](./agent/I38c-mkdir-spec.md) | achievement4 | I38c-mkdir — `mkdir`（修改工具） |
| [`I38c-move-spec.md`](./agent/I38c-move-spec.md) | achievement4 | I38c-move — `move`（修改工具） |
| [`I38c-os-tools-spec.md`](./agent/I38c-os-tools-spec.md) | achievement4 | I38c — P5：OS 工具能力规格 |
| [`I38c-os-tools-tasklist.md`](./agent/I38c-os-tools-tasklist.md) | achievement4 | I38c — P5 OS 工具能力任务清单 |
| [`I38c-read-spec.md`](./agent/I38c-read-spec.md) | achievement4 | I38c-read — `read`（只读工具） |
| [`I38c-write-spec.md`](./agent/I38c-write-spec.md) | achievement4 | I38c-write — `write`（修改工具） |
| [`I38d-agent-frontend-modes-spec.md`](./agent/I38d-agent-frontend-modes-spec.md) | achievement4 | I38d — P6：前端集成与 Agent 模式规格 |
| [`I38d-agent-frontend-modes-tasklist.md`](./agent/I38d-agent-frontend-modes-tasklist.md) | achievement4 | I38d — P6 前端集成与 Agent 模式任务清单 |
| [`I38h-workspace-spec.md`](./agent/I38h-workspace-spec.md) | achievement4 | I38h — P12：工作区根目录（Workspace Root）规格 |
| [`I38h-workspace-tasklist.md`](./agent/I38h-workspace-tasklist.md) | achievement4 | I38h — P12 工作区任务清单 |
| [`I39-compaction-chat-merge-spec.md`](./agent/I39-compaction-chat-merge-spec.md) | achievement4 | I39 压缩与 chat 的共享边界规格 |
| [`I41-cancel-consistency-patch-spec.md`](./agent/I41-cancel-consistency-patch-spec.md) | achievement4 | I41 — 用户取消的跨链路一致性补丁规格 |
| [`I43-assistant-identity-spec.md`](./agent/I43-assistant-identity-spec.md) | achievement5 | I43 — Assistant 消息身份指示规格（头像 / 模式 / 模型） |
| [`MODEL-CAPABILITY-EVOLUTION-ANALYSIS.md`](./agent/MODEL-CAPABILITY-EVOLUTION-ANALYSIS.md) | achievement5 | 模型能力描述与演进兼容性分析 |
| [`I45-model-capability-facts-spec.md`](./agent/I45-model-capability-facts-spec.md) | achievement6 | I45 — 模型能力事实规格（止损、三态与主体 key） |
| [`I45-model-capability-facts-tasklist.md`](./agent/I45-model-capability-facts-tasklist.md) | achievement6 | I45 — 模型能力事实任务清单 |
| [`I49-delete-trash-spec.md`](./agent/I49-delete-trash-spec.md) | achievement6 | I49-delete-trash — `delete` 默认移入回收站 + 显式永久删除 |
| [`I18-agent-tool-architecture-design.md`](./agent/I18-agent-tool-architecture-design.md) | achievement7 | I18 — Agent 工具架构决策记录（ADR） |
| [`I38e-mcp-tools-spec.md`](./agent/I38e-mcp-tools-spec.md) | achievement7 | I38e — P7：MCP 外部工具规格 |
| [`I38e-mcp-tools-tasklist.md`](./agent/I38e-mcp-tools-tasklist.md) | achievement7 | I38e — P7 MCP 外部工具任务清单 |
| [`I46-agent-compact-message-format-design.md`](./agent/I46-agent-compact-message-format-design.md) | achievement7 | I46 — 工具结果紧凑消息格式设计（参考记录） |
| [`I46-agent-compact-message-format-tasklist.md`](./agent/I46-agent-compact-message-format-tasklist.md) | achievement7 | I46 — 工具结果紧凑信封任务清单 |
| [`I46-agent-node-layout-design.md`](./agent/I46-agent-node-layout-design.md) | achievement7 | I46 — Agent 节点布局设计（ADR） |
| [`I46-agent-node-layout-spec.md`](./agent/I46-agent-node-layout-spec.md) | achievement7 | I46 — Agent 节点布局实施规格 |
| [`I46-agent-node-layout-tasklist.md`](./agent/I46-agent-node-layout-tasklist.md) | achievement7 | I46 — Agent 节点布局任务清单 |
| [`I46-tool-result-preview-spec.md`](./agent/I46-tool-result-preview-spec.md) | achievement7 | I46 — 工具结果落盘回填预览规格 |
| [`I50-runtime-audit-spec.md`](./agent/I50-runtime-audit-spec.md) | achievement7 | I50 — 运行时审计链路规格 |
| [`I50-runtime-audit-tasklist.md`](./agent/I50-runtime-audit-tasklist.md) | achievement7 | I50 — 运行时审计任务清单 |
| [`audit-coverage-audit-1.md`](./agent/audit-coverage-audit-1.md) | achievement7 | 运行时审计覆盖盘点（后端缺口与观察项） |
| [`audit-coverage-audit-2.md`](./agent/audit-coverage-audit-2.md) | achievement9 | 运行时审计覆盖盘点（前端缺口与观察项） |
| [`I53-sandbox-preparation-spec.md`](./agent/I53-sandbox-preparation-spec.md) | achievement10 | I53 — P14-a 沙箱准备规格（后端抽象 / 链式探测 / 三平台实现；Linux/macOS 实机用例待 CI 验证） |
| [`I53-sandbox-preparation-tasklist.md`](./agent/I53-sandbox-preparation-tasklist.md) | achievement10 | I53 — P14-a 沙箱准备任务清单（3 项实机验证未勾选，随归档保留原貌） |
| [`agent-sandbox-research.md`](./agent/agent-sandbox-research.md) | achievement10 | Agent 命令行隔离环境开源项目调研（后端选型 ADR-1–ADR-4） |
| [`I55-agent-turn-observability-spec.md`](./agent/I55-agent-turn-observability-spec.md) | achievement | I55 — Agent Turn 图观测规格（节点级事件与三投影；P15 V0–V2 已交付，V3 回放受闸未开工） |
| [`I55-agent-turn-observability-tasklist.md`](./agent/I55-agent-turn-observability-tasklist.md) | achievement | I55 — Agent Turn 观测面任务清单（V1 / V2 全部勾选，V3 保留未开工原貌） |
| [`I57-long-line-read-spec.md`](./agent/I57-long-line-read-spec.md) | achievement | I57 — P16 长行读取规格（字节口径扫描宽限 / 整行计费 / 截断报告 / 回执与 `edit` 闸门） |
| [`I57-long-line-read-tasklist.md`](./agent/I57-long-line-read-tasklist.md) | achievement | I57 — P16 长行读取任务清单（T1–T4 全部勾选） |
| [`I58-super-tool-spec.md`](./agent/I58-super-tool-spec.md) | achievement | I58 — P17 超级工具规格（能力发现 / 索引型读取 / 行地址编辑 / 门禁 G1–G23） |
| [`I58-super-tool-tasklist.md`](./agent/I58-super-tool-tasklist.md) | achievement | I58 — P17 超级工具任务清单（T1–T8 全部勾选） |
| [`I59-chat-error-null-contract-spec.md`](./agent/I59-chat-error-null-contract-spec.md) | achievement | I59 — 前端 ChatError null 语义与守卫对齐（含同类判据合并） |
| [`I60-tool-running-status-spec.md`](./agent/I60-tool-running-status-spec.md) | achievement | I60 — 工具调用三阶段状态（preparing / running / 终态与全局活动指示器） |

### vault/ (36)

| 文档 | 原批次 | 标题 |
|---|---|---|
| [`I9-vault-issues.md`](./vault/I9-vault-issues.md) | achievement1 | Vault 代码审查问题清单 |
| [`I9-vault-responsibility-split.md`](./vault/I9-vault-responsibility-split.md) | achievement1 | I9 — Vault 职责拆分规格 |
| [`I23-vault-ipc-contract-tasklist.md`](./vault/I23-vault-ipc-contract-tasklist.md) | achievement2 | I23 — vault IPC 契约收口 任务清单 |
| [`I23-vault-ipc-contract.md`](./vault/I23-vault-ipc-contract.md) | achievement2 | Vault IPC 契约边界审查与收口方案 |
| [`I9-vault-async-command-migration.md`](./vault/I9-vault-async-command-migration.md) | achievement2 | Vault Tauri 命令同步→异步迁移 |
| [`I9-vault-spec-1-foundation.md`](./vault/I9-vault-spec-1-foundation.md) | achievement2 | I9-1 — 密码管理箱：安全基础与总览 |
| [`I9-vault-spec-2-crud-and-ui.md`](./vault/I9-vault-spec-2-crud-and-ui.md) | achievement2 | I9-2 — 密码管理箱：数据交互与前端 |
| [`I9-vault-spec-3-security-enhance.md`](./vault/I9-vault-spec-3-security-enhance.md) | achievement2 | I9-3 — 密码管理箱：安全增强与运行时守护 |
| [`I9-vault-spec-3a-biometric-platform.md`](./vault/I9-vault-spec-3a-biometric-platform.md) | achievement2 | I9-3a — 密码管理箱：生物识别平台真实实现 |
| [`I9-vault-spec-3b-biometric-crypto-binding-fix.md`](./vault/I9-vault-spec-3b-biometric-crypto-binding-fix.md) | achievement2 | I9-3b — 生物识别密码学绑定修复设计 |
| [`I9-vault-spec-3c-native-unlock-dialog.md`](./vault/I9-vault-spec-3c-native-unlock-dialog.md) | achievement2 | I9-3c — 原生解锁对话框（Native Unlock Dialog） |
| [`I9-vault-spec-4-git-sync.md`](./vault/I9-vault-spec-4-git-sync.md) | achievement2 | I9-4 — 密码管理箱：Git 同步 |
| [`I9-vault-spec-5-ssh-and-recovery.md`](./vault/I9-vault-spec-5-ssh-and-recovery.md) | achievement2 | I9-5 — 密码管理箱：SSH Agent + 恢复密钥 |
| [`I9-vault-spec.md`](./vault/I9-vault-spec.md) | achievement2 | I9 — 密码管理箱（Password Vault）规格说明 |
| [`I9-vault-tasklist-biometric.md`](./vault/I9-vault-tasklist-biometric.md) | achievement2 | I9 — 密码管理箱（Password Vault）P1-F 遗留任务清单 |
| [`I9-vault-tasklist-p2.md`](./vault/I9-vault-tasklist-p2.md) | achievement2 | I9 — 密码管理箱（Password Vault）P2 任务清单 |
| [`I9-vault-tasklist-p3.md`](./vault/I9-vault-tasklist-p3.md) | achievement2 | I9 密码箱 P3 任务清单（P2 残留闭环） |
| [`I9-vault-tasklist.md`](./vault/I9-vault-tasklist.md) | achievement2 | I9 — 密码管理箱（Password Vault）任务清单 |
| [`I32-vault-git-sync-spec.md`](./vault/I32-vault-git-sync-spec.md) | achievement3 | I32 保险库 Git 同步：语义修正与远程感知规格 |
| [`I33-vault-home-ui-audit.md`](./vault/I33-vault-home-ui-audit.md) | achievement3 | I33 保险库首页 UI 审计与设置入口语义整合 |
| [`I34-vault-totp-config-design.md`](./vault/I34-vault-totp-config-design.md) | achievement3 | I34 保险库 TOTP 配置：数据归属与持久化规格 |
| [`I9-vault-deadcode.md`](./vault/I9-vault-deadcode.md) | achievement3 | Vault 死代码问题与修复方案 |
| [`I9-vault-audit-spec.md`](./vault/I9-vault-audit-spec.md) | achievement5 | I9 密码箱 — 审计与可观测性规格（原 key-hierarchy-spec §8 迁出） |
| [`I9-vault-biometric-spec.md`](./vault/I9-vault-biometric-spec.md) | achievement5 | I9 密码箱 — 生物识别凭据规格（原 key-hierarchy-spec §5 迁出） |
| [`I9-vault-capability-gating-spec.md`](./vault/I9-vault-capability-gating-spec.md) | achievement5 | I9 密码箱 — 能力、配置与版本门禁规格（S6） |
| [`I9-vault-crypto-surface-inventory.md`](./vault/I9-vault-crypto-surface-inventory.md) | achievement5 | I9 密码箱加密体系全景清单 |
| [`I9-vault-entry-cipher-spec.md`](./vault/I9-vault-entry-cipher-spec.md) | achievement5 | I9 密码箱 — 条目层加密规格（S2） |
| [`I9-vault-implementation-tasks.md`](./vault/I9-vault-implementation-tasks.md) | achievement5 | I9 密码箱 — 实施任务清单（v1.5） |
| [`I9-vault-key-hierarchy-design.md`](./vault/I9-vault-key-hierarchy-design.md) | achievement5 | I9 密码箱 密钥体系设计（v1.5：`dek` 解耦 + 纸面即密钥） |
| [`I9-vault-key-hierarchy-spec.md`](./vault/I9-vault-key-hierarchy-spec.md) | achievement5 | I9 密码箱密钥体系 — 核心实施规格（v1.5） |
| [`I9-vault-password-model-comparison.md`](./vault/I9-vault-password-model-comparison.md) | achievement5 | I9 密码箱 密码模型横向对比（当前实现 / 目标设计 / 参考项目） |
| [`I9-vault-plaintext-egress-spec.md`](./vault/I9-vault-plaintext-egress-spec.md) | achievement5 | I9 密码箱 — 明文出口与辅助密钥规格（S5） |
| [`I9-vault-recovery-channel-spec.md`](./vault/I9-vault-recovery-channel-spec.md) | achievement5 | I9 密码箱 — 恢复通道规格（原 key-hierarchy-spec §3.8 + §6 迁出） |
| [`I9-vault-runtime-session-spec.md`](./vault/I9-vault-runtime-session-spec.md) | achievement5 | I9 密码箱 — 运行时密钥与会话驻留规格（S4） |
| [`I9-vault-sync-crypto-spec.md`](./vault/I9-vault-sync-crypto-spec.md) | achievement5 | I9 密码箱 — 仓库与同步完整性规格（S3） |
| [`I9-vault-v15-tasklist.md`](./vault/I9-vault-v15-tasklist.md) | achievement5 | I9 — 密码箱 v1.5 任务清单 |

### chat/ (35)

| 文档 | 原批次 | 标题 |
|---|---|---|
| [`I12-history-dedup-spec.md`](./chat/I12-history-dedup-spec.md) | achievement | I12 — HistoryManager 与 chatStore 代码优化 规格说明 |
| [`I12-history-dedup-tasklist.md`](./chat/I12-history-dedup-tasklist.md) | achievement | I12 — HistoryManager 与 chatStore 代码优化 任务清单 |
| [`I2-history-spec.md`](./chat/I2-history-spec.md) | achievement | I2-History — 聊天消息历史管理系统 规格说明 |
| [`I2-history-tasklist.md`](./chat/I2-history-tasklist.md) | achievement | I2-History — 聊天消息历史管理 任务清单 |
| [`I2-spec.md`](./chat/I2-spec.md) | achievement | I2 — Orb AI 对话 规格说明 |
| [`I2-tasklist.md`](./chat/I2-tasklist.md) | achievement | I2 — Orb AI 对话 任务清单 |
| [`I51-history-detail-inspector-spec.md`](./chat/I51-history-detail-inspector-spec.md) | achievement | I51 — 历史页检视能力规格（身份口径 / 元数据 / 行内原文面板 / 导出） |
| [`I51-history-detail-inspector-tasklist.md`](./chat/I51-history-detail-inspector-tasklist.md) | achievement | I51 — 历史页检视能力 任务清单 |
| [`I13-chat-service-split-spec.md`](./chat/I13-chat-service-split-spec.md) | achievement1 | I13 — Chat 服务层分离 规格说明 |
| [`I13-chat-split-ext-spec.md`](./chat/I13-chat-split-ext-spec.md) | achievement1 | I13-Ext — Chat Store 职责细粒度拆分 规格说明 |
| [`I13-chat-split-ext-tasklist.md`](./chat/I13-chat-split-ext-tasklist.md) | achievement1 | I13-Ext — Chat Store 职责细粒度拆分 任务清单 |
| [`I15-chat-runtime-spec.md`](./chat/I15-chat-runtime-spec.md) | achievement1 | I15 — Chat Runtime 提取规格说明 |
| [`I15-chat-runtime-tasklist.md`](./chat/I15-chat-runtime-tasklist.md) | achievement1 | I15 — Chat Runtime 提取任务清单 |
| [`I16-frontend-backend-protocol-spec.md`](./chat/I16-frontend-backend-protocol-spec.md) | achievement1 | I16 — 前后端消息协议统一设计规格说明 |
| [`I16-stream-attempt-identity-spec.md`](./chat/I16-stream-attempt-identity-spec.md) | achievement1 | I16 — 流尝试身份对齐规格说明 |
| [`I17-chat-retry-recovery-spec.md`](./chat/I17-chat-retry-recovery-spec.md) | achievement1 | I17 — Chat 重发/重试恢复体系重构规格说明 |
| [`I17b-conversation-branch-spec.md`](./chat/I17b-conversation-branch-spec.md) | achievement1 | I17b — 对话分支（Conversation Branch）规格说明 |
| [`I2a-type-system-spec.md`](./chat/I2a-type-system-spec.md) | achievement1 | I2a — 消息来源追踪规格说明 |
| [`I2e-MULTIMODAL-ATTACHMENTS-ANALYSIS.md`](./chat/I2e-MULTIMODAL-ATTACHMENTS-ANALYSIS.md) | achievement1 | 多模态附件能力引入分析 |
| [`I2e-vision-resolution-design.md`](./chat/I2e-vision-resolution-design.md) | achievement1 | I2e Vision Resolution — 模型视觉能力解析设计 |
| [`I21-send-chat-message-refactor-design.md`](./chat/I21-send-chat-message-refactor-design.md) | achievement2 | I21 — `send_chat_message` 上帝函数重构设计 |
| [`I2e-phase1-spec.md`](./chat/I2e-phase1-spec.md) | achievement2 | I2e Phase 1 — 前端可用 + 后端 AttachmentStore 规格说明 |
| [`I2e-phase1-tasklist.md`](./chat/I2e-phase1-tasklist.md) | achievement2 | I2e Phase 1 — 任务清单 |
| [`I2f-attachment-refs-orphan-management.md`](./chat/I2f-attachment-refs-orphan-management.md) | achievement2 | I2f — 附件关联索引与孤儿文件管理设计 |
| [`I28-send-chat-message-args-convergence-analysis.md`](./chat/I28-send-chat-message-args-convergence-analysis.md) | achievement3 | I28 — `send_chat_message` 参数收敛设计 |
| [`I29-token-usage-analysis.md`](./chat/I29-token-usage-analysis.md) | achievement3 | Token Usage 语义分析 |
| [`I29-token-usage-semantics-spec.md`](./chat/I29-token-usage-semantics-spec.md) | achievement3 | Token Usage 语义与计算规范 |
| [`I29-token-usage-tasklist.md`](./chat/I29-token-usage-tasklist.md) | achievement3 | Token Usage 改造 — 任务清单 |
| [`I29-token-usage-ui-spec.md`](./chat/I29-token-usage-ui-spec.md) | achievement3 | Token Usage 语义契约与展示规范 |
| [`I2e-multimodal-attachments-spec.md`](./chat/I2e-multimodal-attachments-spec.md) | achievement3 | I2e — 多模态附件规格说明 |
| [`I2g-remaining-features-spec.md`](./chat/I2g-remaining-features-spec.md) | achievement3 | I2g 剩余功能规格 — I2e Phase 2/3 + I2f 补丁 |
| [`I2g-tasklist.md`](./chat/I2g-tasklist.md) | achievement3 | I2g — 任务清单（已归档） |
| [`I2h-attachment-budget-tasklist.md`](./chat/I2h-attachment-budget-tasklist.md) | achievement3 | I2h — 附件链路收敛（长文本预算 + PDF 降级） |
| [`I40-history-message-tree-spec.md`](./chat/I40-history-message-tree-spec.md) | achievement4 | I40 — 主窗口历史消息树渲染规格 |
| [`I35-screenshot-qa-spec.md`](./chat/I35-screenshot-qa-spec.md) | achievement10 | I35 — 截屏提问规格（rev11 已实现；§11.3 手动验收待执行） |

### ui/ (36)

| 文档 | 原批次 | 标题 |
|---|---|---|
| [`CLOCK-NOTES-REFERENCE-ANALYSIS.md`](./ui/CLOCK-NOTES-REFERENCE-ANALYSIS.md) | achievement | 时钟与便签：工具化可行性分析 |
| [`I1-orb-no-clickthrough-spec.md`](./ui/I1-orb-no-clickthrough-spec.md) | achievement | I1-orb-no-clickthrough — 移除 Orb 自动穿透轮询 规格说明 |
| [`I1-orb-no-clickthrough-tasklist.md`](./ui/I1-orb-no-clickthrough-tasklist.md) | achievement | I1-orb-no-clickthrough — 移除 Orb 点击穿透 任务清单 |
| [`I1-spec.md`](./ui/I1-spec.md) | achievement | I1 — Orb 悬浮球 规格说明 |
| [`I1-tasklist.md`](./ui/I1-tasklist.md) | achievement | I1 — Orb 悬浮球 任务清单 |
| [`I10-command-registry-spec.md`](./ui/I10-command-registry-spec.md) | achievement | I10 — 命令面板注册表化 规格说明 |
| [`I10-command-registry-tasklist.md`](./ui/I10-command-registry-tasklist.md) | achievement | I10 — 命令面板注册表化 任务清单 |
| [`I11-A-hardcoded-strings.md`](./ui/I11-A-hardcoded-strings.md) | achievement | I11-A — 硬编码文案清单 |
| [`I11-i18n-spec.md`](./ui/I11-i18n-spec.md) | achievement | I11 — 国际化（i18n）规格说明 |
| [`I11-i18n-tasklist.md`](./ui/I11-i18n-tasklist.md) | achievement | I11 — 国际化（i18n）任务清单 |
| [`I3-spec.md`](./ui/I3-spec.md) | achievement | I3 — 主窗口工具箱 规格说明 |
| [`I3-tasklist.md`](./ui/I3-tasklist.md) | achievement | I3 — 主窗口工具箱 任务清单 |
| [`I4-spec.md`](./ui/I4-spec.md) | achievement | I4 — 设置系统 + 全局快捷键 + 主窗口 UI 改造 规格说明 |
| [`I4-tasklist.md`](./ui/I4-tasklist.md) | achievement | I4 — 设置系统 + 全局快捷键 + 主窗口 UI 改造 任务清单 |
| [`I5-extensible-renderer-spec.md`](./ui/I5-extensible-renderer-spec.md) | achievement | I5-Ext — Mascot 可扩展渲染架构 规格说明 |
| [`I5-extensible-renderer-tasklist.md`](./ui/I5-extensible-renderer-tasklist.md) | achievement | I5-Ext — Mascot 可扩展渲染架构 任务清单 |
| [`I5-spec.md`](./ui/I5-spec.md) | achievement | I5 — Mascot 形象 + 表情联动 规格说明 |
| [`I5-tasklist.md`](./ui/I5-tasklist.md) | achievement | I5 — Mascot 形象 + 表情联动 任务清单 |
| [`I6-settings-optimize-spec.md`](./ui/I6-settings-optimize-spec.md) | achievement | I6 — 设置系统优化 规格说明 |
| [`I6-settings-optimize-tasklist.md`](./ui/I6-settings-optimize-tasklist.md) | achievement | I6 — 设置系统优化 任务清单 |
| [`I8-declarative-settings-spec.md`](./ui/I8-declarative-settings-spec.md) | achievement | I8 — 声明式设置系统 规格说明 |
| [`I8-declarative-settings-tasklist.md`](./ui/I8-declarative-settings-tasklist.md) | achievement | I8 — 声明式设置系统 任务清单 |
| [`design-settings-page-shell-unify.md`](./ui/design-settings-page-shell-unify.md) | achievement1 | SettingsPageShell 统一壳改造设计（组合方案） |
| [`frontend-timing-analysis.md`](./ui/frontend-timing-analysis.md) | achievement1 | 前端时序问题审查 |
| [`I14-mascot-expression-extensibility-spec.md`](./ui/I14-mascot-expression-extensibility-spec.md) | achievement1 | I14 — Mascot 表情映射可扩展化 规格说明 |
| [`I14-mascot-expression-extensibility-tasklist.md`](./ui/I14-mascot-expression-extensibility-tasklist.md) | achievement1 | I14 — Mascot 表情映射可扩展化 任务清单 |
| [`I20-cross-window-event-sync-spec.md`](./ui/I20-cross-window-event-sync-spec.md) | achievement1 | I20 — 跨窗口事件同步改进方案 |
| [`I26-mini-mode-clock-spec.md`](./ui/I26-mini-mode-clock-spec.md) | achievement3 | I26 — mini 模式与时钟工具规格 |
| [`I26-mini-mode-clock-tasklist.md`](./ui/I26-mini-mode-clock-tasklist.md) | achievement3 | I26 — mini 模式与时钟工具任务清单 |
| [`I37-clock-modes-layout-spec.md`](./ui/I37-clock-modes-layout-spec.md) | achievement3 | I37 — clock 双模式与双布局优化规格 |
| [`I37-clock-modes-layout-tasklist.md`](./ui/I37-clock-modes-layout-tasklist.md) | achievement3 | I37 — clock 双模式与双布局优化任务清单 |
| [`MAIN-WINDOW-MINI-MODE.md`](./ui/MAIN-WINDOW-MINI-MODE.md) | achievement3 | 主窗口小窗（Mini）模式分析 |
| [`I1-orb-touch-compat-spec.md`](./ui/I1-orb-touch-compat-spec.md) | achievement5 | I1-orb-touch — Orb 悬浮球触摸兼容规格 |
| [`I1-orb-touch-compat-tasklist.md`](./ui/I1-orb-touch-compat-tasklist.md) | achievement5 | I1-orb-touch — Orb 触摸兼容任务清单 |
| [`I47-orb-runtime-state-spec.md`](./ui/I47-orb-runtime-state-spec.md) | achievement5 | I47 — Orb 运行态与可见性 实施规格 |
| [`I47-orb-runtime-state-tasklist.md`](./ui/I47-orb-runtime-state-tasklist.md) | achievement5 | I47 — Orb 运行态与可见性 任务清单 |

### infra/ (35)

| 文档 | 原批次 | 标题 |
|---|---|---|
| [`DESIGN.md`](./infra/DESIGN.md) | achievement | Assistant - 智能助手应用设计文档（完整非当前版本） |
| [`I7-cmd-split-spec.md`](./infra/I7-cmd-split-spec.md) | achievement | I7 — cmd.rs 模块拆分 规格说明 |
| [`I7-cmd-split-tasklist.md`](./infra/I7-cmd-split-tasklist.md) | achievement | I7 — cmd.rs 模块拆分 任务清单 |
| [`I9-handover.md`](./infra/I9-handover.md) | achievement | I9 交接 — Phase 1-6 + 核心测试 + H1 验收 已完成 |
| [`I9-provider-multi-api-spec.md`](./infra/I9-provider-multi-api-spec.md) | achievement | I9 — Provider 多 API 格式适配 规格说明 |
| [`I9-provider-multi-api-tasklist.md`](./infra/I9-provider-multi-api-tasklist.md) | achievement | I9 — Provider 多 API 格式适配 任务清单 |
| [`I16-naming-migration-spec.md`](./infra/I16-naming-migration-spec.md) | achievement1 | I16 命名迁移规格 — 后端 DTO + 前端 camelCase 统一 |
| [`I16-naming-migration-tasklist.md`](./infra/I16-naming-migration-tasklist.md) | achievement1 | I16 — 命名迁移（后端 DTO + 前端 camelCase 统一）任务清单 |
| [`I19-code-responsibility.md`](./infra/I19-code-responsibility.md) | achievement1 | 代码文件职能检查与功能提取分析 |
| [`I19-error-system-spec.md`](./infra/I19-error-system-spec.md) | achievement1 | I19 — 统一错误处理体系规格说明 |
| [`compile-time-analysis.md`](./infra/compile-time-analysis.md) | achievement2 | 后端编译时间与体积瓶颈分析 |
| [`crate-architecture-analysis.md`](./infra/crate-architecture-analysis.md) | achievement2 | 后端 Crate 架构依赖分析 |
| [`DEPENDENCY-OPTIMIZATION.md`](./infra/DEPENDENCY-OPTIMIZATION.md) | achievement2 | 后端依赖优化补充分析（cargo tree --duplicates） |
| [`dependency-tree-analysis.md`](./infra/dependency-tree-analysis.md) | achievement2 | 后端依赖树审查与优化分析 |
| [`dto-boundary-analysis.md`](./infra/dto-boundary-analysis.md) | achievement2 | Vault / Crypto DTO 边界分析与直出类型清单 |
| [`I16-protocol-type-misplacement-audit.md`](./infra/I16-protocol-type-misplacement-audit.md) | achievement2 | I16 协议类型错放审查 |
| [`I23-deadcode-and-timestamp-units.md`](./infra/I23-deadcode-and-timestamp-units.md) | achievement2 | 死代码补充审计 + 时间戳单位审计 |
| [`dto-frontend-backend-sync-audit.md`](./infra/dto-frontend-backend-sync-audit.md) | achievement3 | DTO 前后端同步审计（收口前基线） |
| [`I20-command-simplification-spec.md`](./infra/I20-command-simplification-spec.md) | achievement3 | I20: 后端命令简化规约 |
| [`I22-chat-request-utils-downgrade-analysis.md`](./infra/I22-chat-request-utils-downgrade-analysis.md) | achievement3 | I22 — `chat/request.rs` 与 `chat/utils.rs` 下放至 `assistant-agent` 分析 |
| [`I24-execution-code-review.md`](./infra/I24-execution-code-review.md) | achievement3 | I24 阶段执行后代码审查记录 |
| [`I24-feature-extension-spec.md`](./infra/I24-feature-extension-spec.md) | achievement3 | I24 — 特性扩展通道通用化（Feature Extension v2） |
| [`I24-tasklist.md`](./infra/I24-tasklist.md) | achievement3 | I24 — 任务清单 |
| [`I42-logging-strategy-spec.md`](./infra/I42-logging-strategy-spec.md) | achievement4 | I42 — 日志策略规格（`tauri-plugin-log` 配置 + 高频日志治理） |
| [`I44-app-dir-management-spec.md`](./infra/I44-app-dir-management-spec.md) | achievement5 | I44 — 应用目录管理规格（受管目录的解析、归属与边界） |
| [`path-handling-inconsistency-analysis.md`](./infra/path-handling-inconsistency-analysis.md) | achievement5 | 路径处理不统一分析（前端 / Rust） |
| [`feature-config-ownership-spec.md`](./infra/feature-config-ownership-spec.md) | achievement7 | 特性配置归属规格（配置 schema 与 DTO 迁入特性边界） |
| [`feature-config-ownership-tasklist.md`](./infra/feature-config-ownership-tasklist.md) | achievement7 | 特性配置归属任务清单 |
| [`REFACTOR-SPLIT-PLAN.md`](./infra/REFACTOR-SPLIT-PLAN.md) | achievement8 | 超长文件拆分与职责归位规划 |
| [`FEATURE-ZERO-COST-MOUNT-ANALYSIS.md`](./infra/FEATURE-ZERO-COST-MOUNT-ANALYSIS.md) | achievement8 | 可选特性零成本挂载：设计思路合理性分析 |
| [`code-structure-optimization-spec.md`](./infra/code-structure-optimization-spec.md) | achievement9 | 代码组织与深度扁平度优化规格（未完成项移交 code-structure-followups） |
| [`compat-tech-debt-audit-1.md`](./infra/compat-tech-debt-audit-1.md) | achievement10 | 前后端兼容技术债审计（C6 已立项并首批落地 chat / agent，仍开放） |
| [`feature-mount-traversal-spec.md`](./infra/feature-mount-traversal-spec.md) | achievement11 | 可选特性装配点遍历化规格（S1–S7 已落地，关闭态证据由 feature-optional-binary-spec 接续） |
| [`code-structure-followups.md`](./infra/code-structure-followups.md) | achievement12 | 代码结构优化后续账目（F3 判定收口、F1-b 暂缓；随本批归档冻结） |
| [`command-error-contract-spec.md`](./infra/command-error-contract-spec.md) | achievement12 | 命令错误契约规格（B0–B2 首批落地，C6 仍开放） |

