# 代码注释规范

| 项 | 值 |
|---|---|
| 状态 | 现行（§7 的 G2 / G3 / G4 尚未接入门禁，见 §9 T1–T2） |
| 日期 | 2026-11-22 |
| 适用 | `src-tauri/**/*.rs`、`src/**/*.{ts,tsx}`、`scripts/**/*.mjs`；生成物与 JSON 配置只作为被约束对象 |
| 依据 | [CODE-REVIEW-PRINCIPLE](../principle/CODE-REVIEW-PRINCIPLE.md) D5（注释判据唯一来源）、D4（`SAFETY`）、D6（注释与代码一致）；[CODE-SPLIT-PRINCIPLE](../principle/CODE-SPLIT-PRINCIPLE.md)（文件职责）；[feature-optional-binary-spec](./feature-optional-binary-spec.md) §8.2（下线不留注释残骸）；[PROJECT-DESIGN-GUIDELINES](../analysis/PROJECT-DESIGN-GUIDELINES.md) §2（跨 slice 只读注释） |
| 关联 | [I65 吞没整改](./I65-error-swallowing-remediation-spec.md)（`best-effort:` 门禁来源）；[I67](./I67-storage-inventory-totals-spec.md)（编号引用约定） |

> 一句话：注释只回答代码回答不了的问题——**为什么这样写、不变量是什么、和哪条规格对齐**。它不是代码的复述、不是变更日志、不是待办仓库、不是设计文档的替身。

本文中的「必须 / 禁止」为可验收约束；「应当」为默认做法，偏离需在被注释处的同一段落写明理由。

**编号约定**：`N1–N15` 现状事实、`C1–C12` 条款、`G1–G6` 门禁、`V1–V10` 验收判据、`T1–T4` 实施切片、`X1–X8` 拒绝方案、`OQ1–OQ4` 开放问题。跨文档引用一律带文档前缀（`CODE-REVIEW-PRINCIPLE D5`、`I61 §3.11.3`），裸写 `§` 一律指本文。`D*`（审查维度）沿用 CODE-REVIEW-PRINCIPLE 定义，本文不重复定义。

---

## 1 定位与边界

### 1.1 注释回答的三类问题

1. **为什么**：为什么用这个顺序、为什么不用更直观的写法、为什么这里必须绕过某层。
2. **不变量**：什么东西必须始终成立（单位、键恒在、`None` 与 `0` 的区别、锁的持有范围、顺序即语义）。
3. **锚点**：这段代码对应哪条规格的哪一节、哪个外部事实（平台行为、上游版本）在支撑它。

代码本身已经能回答「做了什么」，因此复述实现不属于注释职责。

### 1.2 与其它载体的分工

| 载体 | 回答什么 | 不回答什么 |
|---|---|---|
| 代码 | 做了什么、怎么做的 | 为什么这么做 |
| 注释 | 为什么、不变量、边界、规格锚点 | 变更历史、待办、长篇设计、事故复盘全文 |
| 规格 / 文档 | 设计、决策、验收判据、实施切片 | 逐行实现细节 |
| git 历史 | 谁在何时改了什么、改动的原始动机 | 当前行为（注释也不承担） |
| 测试 | 行为是否成立 | 设计动机 |

同一事实只在一个载体上陈述：设计进文档，动机留在注释，历史交给 git。三处各写一遍即为 D6 缺陷。

### 1.3 纳入与排除

**纳入**：Rust / TS / 脚本注释的分层、落点、内容规则、标记词表、格式与门禁。

**排除**：

1. 注释的**判据**（什么是缺陷）——唯一来源是 CODE-REVIEW-PRINCIPLE D5，本文只给操作细则；
2. 文档写作与排版规范——见 [readme-spec](./readme-spec.md)；
3. 格式化——由 prettier / rustfmt 决定，注释内容不被重排（见 C9）；
4. 生成物内容——由 ts-rs 生成，本文只规定「不得手改」（C11）。

---

## 2 现状事实（以代码为准）

| 编号 | 事实 |
|---|---|
| N1 | 注释叙述语言为中文，标识符与技术词保留英文（`None` / `partial` / `best-effort:`）。`src-tauri/src/cmd/storage/section.rs` 与 `src/services/stream/turnOrchestrator.ts` 通篇如此 |
| N2 | Rust 文件级 `//!` 承载文件级不变量：「本文件无条件编译」「三条形状纪律」「顺序是语义」。例：`src-tauri/src/cmd/storage/section.rs` 头部、`src-tauri/crates/assistant-agent/src/agent/middleware.rs` 头部 |
| N3 | 条目级 `///` 写契约而非实现：`None` 的语义（「不是 `0`」）、固定映射、是否进 wire、由谁自述。例：`section.rs` 的 `SectionStats` / `EntryDecl` / `SectionSample` / `StorageSection` |
| N4 | 内联 `//` 解释「为什么 / 顺序约束」并引用规格编号。例：`middleware.rs` 的「③ 必须在 ② 之后」、`turnOrchestrator.ts` 的「乐观消息必须带 `modeId`（I43 §4.1）」 |
| N5 | 分节横幅 `// ----…` 用于单文件内多职责段。例：`src-tauri/src/cmd/config.rs`、`src-tauri/crates/assistant-protocol/src/agent.rs`、多个 `tests/*.rs`；横幅标题语言不统一（`Side-effect registry` 为英文，其余多为中文） |
| N6 | `// SAFETY:` 与 `unsafe` 块配对，写不变量与成立前提。例：`assistant-tools/src/sandbox/macos.rs`、`sandbox/linux.rs`、`assistant-tools/src/runtime/storage.rs` |
| N7 | `// best-effort:` 是**机器可读**豁免标记：`eslint.config.ts` 的 `SWALLOW_SELECTORS` 与 `scripts/check-error-swallow.mjs` 都按「同行或上一行」识别；代码侧存量 4 处 |
| N8 | 前端跨 slice 只读约定要求头部注释 `// 跨 slice 读取`（PROJECT-DESIGN-GUIDELINES §2）；当前 `src/` 无命中——是「约定尚未落地」，不是既成事实 |
| N9 | 代码中无 `TODO` / `FIXME` / `HACK` / `XXX` 标记；仅两处非标记命中：`agent_shell/classify.rs` 的测试字符串 `"rg TODO"`、`middleware.rs` 的「不留 TODO 桩」。现有做法是「不留桩」 |
| N10 | CODE-REVIEW-PRINCIPLE D5 已给出注释判据（解释为什么；复述做了什么、与代码不一致、无主 TODO 为缺陷；被注释掉的代码块属调试残留）。本文不复述判据，只补落点、词表、格式与门禁 |
| N11 | 生成物头部自带 `Do not edit manually`；Rust DTO 字段的 `///` 会传播为 TS 字段注释（`src/types/protocol.contract.ts` 的 `toolConfig` / `avatar`）；`scripts/contract-check.mjs` 按整文件哈希比对，注释漂移同样会让「契约」门禁失败 |
| N12 | 门禁现状：`scripts/check.mjs` 的 PHASE1 为格式 / 类型 / 规则 / 吞没 / 文案，PHASE2 为契约；没有注释卫生（TODO、裸 `§`、注释掉的代码）的任何检查 |
| N13 | 四个配置文件 `config.json` / `vault-config.json` / `orb-config.json` / `mascot-config.json` 是 JSON（I67 N14），JSON 无注释语法；说明只能落在 DTO 字段注释或文档 |
| N14 | 代码注释存在裸 `§` 引用：`src/services/stream/turnOrchestrator.ts:478` 的 `（§3.7）`；「跨文档引用带前缀」的约定只写在文档里（I67 编号约定），代码侧未成文 |
| N15 | 注释按需出现而非全覆盖：`src/settings/pages/storage/` 的纯函数模块没有注释；注释密集处集中在契约面（DTO、分区接口）与顺序 / 时序敏感处。这是「不设密度指标」的事实基础 |

---

## 3 分层与落点

| 编号 | 层级 | 语法 | 只写什么 |
|---|---|---|---|
| C1 | 文件级 | Rust `//!`；TS 文件头 `/** */` | 文件职责、文件级不变量、依赖方向、规格锚点 |
| C2 | 条目级 | Rust `///`；TS `/** */` | 契约面：参数约束、返回值语义、副作用、错误类别、是否进 wire |
| C3 | 内联 | `//` | 为什么、顺序约束、为何不用另一方案 |
| C4 | 分节横幅 | `// ----…` | 同一文件内互不相关职责段的分隔 |

**C1 文件级**：必须能在三句话内说清「这个文件负责什么、什么不能进这个文件」。超过一屏按 C8 升级。没有文件级不变量、也没有对外契约的文件可以省略（N15）。

**C2 条目级**：必须写清调用方无法从签名推断的语义——`None` / 空 / 下界分别表示什么，`0` 是否合法，返回值是否进 wire，失败落在哪个封闭类别。禁止复述函数名与实现步骤。DTO 字段注释按跨端契约写（N11）：改它等于改契约。

**C3 内联**：紧邻被解释的语句；标记词允许同行或上一行（N7）。单条应当不超过两行；超出即按 C8 升级，不得堆成段落。

**C4 分节横幅**：仅当同一文件有 ≥3 个互不相关职责段时使用；分隔线沿用仓库既有样式，不新造符号。新增横幅标题用中文；存量英文标题不强制改（避免无意义 churn）。

---

## 4 内容与引用

**C5 只写三类内容**：为什么、不变量、锚点（§1.1）。代码已能表达的「做了什么」禁止重复；能靠命名与类型表达的内容不写注释。

**C6 规格引用必须带文档前缀**：`I61 §3.10`、`CODE-REVIEW-PRINCIPLE D5`。禁止裸 `§`（N14）。引用**章节号**，不引用行号（行号会漂移）；引用已归档文档写「文档名（已归档）」，不得写路径。

**C7 注释与代码同批变更**：行为变了，注释必须同批改（D6）。注释声明的不变量被代码破坏时，二者必有一处是缺陷，禁止留下「注释是对的、代码是错的」的悬置状态。

**C8 升级阈值**：出现下列任一情形，长文进 `docs/`，代码只留一行锚点 + 文档名：

1. 内联超过两行；
2. 文件头超过一屏；
3. 需要表格、图、时间线或事故复盘；
4. 需要跨文件、跨迭代才能说清。

**C9 语言与格式**：中文叙述 + 英文标识符；`//` 后一个空格；`///` / `/**` 段落之间用空注释行；标点跟随中文（全角）。单行不超格式化宽度（TS 80 / Rust 100）——注释不被 prettier / rustfmt 重排，必须人工控制。

**C10 敏感信息**：注释中禁止出现密钥、令牌、完整本机路径、用户名、机器名；需要举例时用占位符，与文档规范一致。

**C11 生成物**：禁止手改 `src/types/protocol.contract.ts`、`src/types/shortcutDefaults.generated.ts`、`src/vault/core/contract/**` 的注释。DTO 注释改动后必须 `npm run contract:gen` 并提交，否则「契约」门禁失败（N11）。

**C12 配置**：JSON 无注释语法（N13），禁止新增 `_comment` / 注释键；配置项说明写进 DTO 字段注释或文档。

---

## 5 标记词表

标记词是**机器可读**的，只允许下表三项；新增标记必须先改本节，禁止自造（自造标记门禁无法识别）。

| 标记 | 落点 | 语义 | 保障 |
|---|---|---|---|
| `SAFETY:` | `unsafe` 块紧邻上一行 | 不变量、成立前提、为何安全（D4） | G4 |
| `best-effort:` | 丢弃点同行或上一行 | 允许吞没的理由 | 已有门禁（N7） |
| `跨 slice 读取` | 跨域只读的头部 / 字段 | 只读不写，写操作必须走 action | 审查（T4） |

`TODO` / `FIXME` / `HACK` / `XXX` **不属于**标记词表：它们没有主人、没有门禁、没有关闭条件，按 D5 判缺陷（N9）。

---

## 6 禁止事项

| 形态 | 判据 | 依据 |
|---|---|---|
| 复述代码（`// 把 x 设为 y`） | 删掉注释不损失信息 | D5 |
| 与行为不一致 / 已过期 | 注释描述的与代码实际行为不同 | D5 / D6 |
| 被注释掉的代码块 | 调试残留，应删除；恢复靠 git | D5 |
| 无主 `TODO` / `FIXME` / `HACK` / `XXX` | 无主人、无关闭条件 | D5 / G2 |
| 变更日志式注释（日期、作者、「本次修改」） | 历史归 git，注释只留当前事实 | §1.2 |
| 注释代替校验 / 测试（「这里不会为 `None`」） | 应写断言或类型约束，不是注释 | D1 |
| 注释掩盖下线残留 | 下线是「真删除」，不用注释或 `#[allow]` 掩盖 | feature-optional-binary-spec §8.2 |
| 手改生成物注释 | 改 Rust 源后重新生成 | C11 |
| JSON 伪注释字段（`_comment`） | JSON 无注释语法 | C12 |
| 调试残留（`dbg!` / `println!` / `console.log`） | 不进主干 | D5 |
| 注释中的敏感信息与本机绝对路径 | 用占位符 | C10 |
| 自造标记词 | 门禁无法识别，形成第二套词表 | §5 |

---

## 7 门禁

**G1 只做形态扫描**：注释质量门禁只检查可机读形态（标记词、引用前缀、生成物漂移），语义判断交给 D5 审查。任何「理解注释含义」的检查都会产生高误报，按 X1 拒绝。

**G2 无主标记扫描**：新增 `scripts/check-comment-hygiene.mjs`，扫描 `src-tauri` 与 `src`（排除 `tests/`、`examples/`、`target/`、`node_modules/`），命中 `TODO|FIXME|HACK|XXX` 即失败。字符串字面量内的合法命中（如 `"rg TODO"`）逐条白名单登记理由，仿 `check-error-swallow.mjs` 的 `WHITELIST`。

**G3 裸 `§` 扫描**：每个 `§` 必须紧跟 `I<数字>` 或文档名前缀（C6）；命中裸 `§` 即失败。白名单与 G2 共用。

**G4 `SAFETY` 配对**：启用 `clippy::undocumented_unsafe_blocks`（restriction 组，需显式启用）。先 `warn` 收集存量，存量确认齐全（N6）后转 `deny`。

**G5 生成物漂移**：已由 `scripts/contract-check.mjs` 覆盖（N11），不新增检查。

**G6 注释掉的代码块**：不做正则门禁（误报高，X1），由 D5 审查兜底。

---

## 8 验收判据

| 编号 | 判据 | 检查方式 |
|---|---|---|
| V1 | 新增 / 修改的模块有文件级注释，或能说明「无文件级不变量」（N15） | 审查 |
| V2 | 每个 `unsafe` 块紧邻 `SAFETY:` | G4 |
| V3 | 每个吞没点有 `best-effort:` 或日志 | 现有门禁（N7） |
| V4 | `src-tauri` / `src` 无 `TODO` / `FIXME` / `HACK` / `XXX`（白名单除外） | G2 |
| V5 | 代码注释中的规格引用均带文档前缀，无裸 `§` | G3 |
| V6 | 无注释掉的代码、无调试残留 | 审查（D5） |
| V7 | 生成物未手改；DTO 注释改动后已重新生成且「契约」门禁通过 | G5 |
| V8 | 注释与行为同批变更 | 审查（D6，不可机检） |
| V9 | JSON 配置无伪注释字段 | 审查（C12） |
| V10 | 注释无敏感信息与本机绝对路径 | 审查（C10） |

---

## 9 实施切片

| 编号 | 动作 | 落点 | 退出条件 |
|---|---|---|---|
| T1 | 新增 `scripts/check-comment-hygiene.mjs`（G2 + G3），接入 `scripts/check.mjs` PHASE1（与「吞没」同批） | `scripts/` | 脚本无白名单时能抓出存量裸 `§`；白名单条目逐条带理由 |
| T2 | 启用 `clippy::undocumented_unsafe_blocks`，先 `warn` 后 `deny` | workspace / crate 根部属性 | 存量 `unsafe` 全部有 `SAFETY:`，转 `deny` 后零告警 |
| T3 | 收敛存量裸 `§` 引用（N14），改为 `I<n> §x` | `src/services/stream/turnOrchestrator.ts` 等 | G3 无白名单通过 |
| T4 | 首次出现跨 slice 只读时补 `// 跨 slice 读取`（N8） | 前端 slice 文件 | 约定从「未落地」变为「有实例 + 审查项」 |

---

## 10 拒绝方案

| 编号 | 方案 | 拒绝理由 |
|---|---|---|
| X1 | 用正则门禁检查「注释掉的代码」或注释语义 | 误报高、不可判定；交 D5 审查（G6） |
| X2 | 给注释设密度 / 行数指标 | 与 CODE-SPLIT-PRINCIPLE 不设阈值同向；指标会催生复述型注释 |
| X3 | 要求所有函数都有文档注释 | 复述型噪声；只要求契约面（C2） |
| X4 | 用 `TODO` 留桩 | D5 判缺陷；未落地能力「不留空位、不留桩」（N9） |
| X5 | 在 JSON 配置加 `_comment` | JSON 无注释语法，破坏契约（C12） |
| X6 | 手改生成物注释 | 会被 `contract-check` 判漂移（C11 / G5） |
| X7 | 用注释记录变更历史、作者、日期 | 历史归 git；注释只留当前事实（§1.2） |
| X8 | 用注释代替校验 / 测试 | 注释不执行；声明「不会为 `None`」而不写断言即为 D1 缺陷 |

---

## 11 开放问题

| 编号 | 问题 | 现状 |
|---|---|---|
| OQ1 | 是否对 `pub` 面启用 `missing_docs` | workspace 未启用；若启用，只覆盖跨 crate 可见面 |
| OQ2 | 分节横幅标题语言是否统一 | 存量中英混用（N5）；本文只约束新增用中文 |
| OQ3 | 存量裸 `§` 是否一次性收敛 | T3 范围待定：只收 `src/`，还是含 `src-tauri/` |
| OQ4 | `跨 slice 读取` 是否需要门禁 | 当前无存量（N8），先按审查项落地，再评估 |