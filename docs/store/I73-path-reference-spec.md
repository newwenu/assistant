# I73 — 路径引用体系规格

> 性质：规格（实施约束，不含实施记录）
> 进度：`I73-path-reference-tasklist.md`（唯一任务级进度表；本规格只定义「改什么、为什么」，不记录「改没改」）
> 事实来源：本规格 §2（以代码现场为准，不引用二手描述）
> 判据来源：`docs/principle/CODE-REVIEW-PRINCIPLE.md`（路径解析失败必须可判定、禁止回退相对路径或默认值）、`docs/principle/CODE-SPLIT-PRINCIPLE.md`
> 冲突处置：`I56-shell-tool-spec.md`（已归档）§8.4 写明「`outputPath` 是绝对路径，是模型回读的唯一入口」，与本规格冲突：`data.outputPath` 仍为绝对（§3.6 R6），但模型可见的回读入口改为引用（§3.6 R2 / §4）。归档文档冻结不改；本规格为活跃唯一来源。

---

## 0 判定

**可行，但必须限定「谁遵守」**。结论分三层：

| 问题 | 判定 | 依据 |
|---|---|---|
| 能否定义一套统一的路径引用语法，让**全部工具**都按它解析？ | 可行 | 入参字符串只有一个来源（`parse_path_arg`），可读/写方向各只有一个解析入口（`resolve_readable` / `resolve`），见 §2 第 4–6 行 |
| 能否让**任何一层**都不会把引用串误当普通相对路径？ | 可行且必须做 | 所有路径最终都过 `join_input`；在此处显式拒绝未路由的 scheme，即「静默错」不可能发生 |
| 能否把 shell 临时区也纳入同一套？ | 可行，作为第三个域 | 临时区需要一个沙箱可写根内、又在工作区外的位置；多可写根改造后与输出域同构，见 §3.8 |
| 能否让**整个应用**（含 vault 仓库相对路径、MCP 工具入参、前端工作区绑定值）统一到这套语法？ | 不可行，且无收益 | 这些路径**不接收模型产出的串**，语义是「仓库内相对位置」「第三方 schema」「用户配置值」。强行统一要引入翻译层，违反最小改动 |

因此本规格的范围是：**模型与工具之间往返的路径串**。这是唯一存在「模型抄错、抄长、抄漏」问题的边界。

---

## 1 定位

本规格只做一件事：**把「路径」从自由字符串收敛为一个封闭的引用体系**，并让这条规则在编译期与运行期都可强制执行。

它不新增工具、不改权限模型、不改落盘格式、不引第三方 URL 库。

要解决的现状缺陷（按命中频率排序）：

| # | 缺陷 | 后果 |
|---|---|---|
| F1 | 落盘标签里的路径是**本机绝对路径 + 会话哈希目录**，长度 80–120 字符 | 模型每次回读都要原样抄写；抄错一个字符就变成一次「文件不存在」 |
| F2 | 输出域文件**无法用相对路径表达**（域根在工作区之外，相对路径恒落在工作区） | 模型只有「抄绝对路径」一条路，没有更短的写法可选 |
| F3 | 三处渲染各自拼绝对串（`run.rs` / `pipeline.rs` / `envelope.rs`） | 任一处置换口径，另外两处就与它不一致 |
| F4 | 审批动作映射在 `read` 与 `super_read` 各写一份 | 两处一旦分叉，「同一文件两种写法」可能命中不同审批键 |
| F5 | shell 的临时区按调用创建、调用结束即删，模型没有任何引用通道 | 命令把结果写进临时文件后，模型只能让它重新输出一遍或重跑命令 |
| F6 | 引用类路径的**错误文本**回显解析后的域根物理路径（含用户名与会话哈希） | 域根进入模型可见文本：模型据此拼出系统 temp 并连续被沙箱拒绝（§2 第 16 行 / §3.10 / N18） |

---

## 2 现状取证

| # | 事实 | 位置 |
|---|---|---|
| 1 | 模型只看到 `assistant_text`；`data` 与 `compact` 只进 wire 与 UI | `src-tauri/src/adapters/tool_exec.rs::exec_result`（`content = assistant_text`）、`src-tauri/crates/assistant-agent/src/agent/loop_executor/persist.rs::append_tool_results` |
| 2 | 落盘引用的路径串是绝对路径 | `src-tauri/crates/assistant-tools/src/runtime/storage.rs::persist`、`FileSpillSink::finish`（`path.display().to_string()`） |
| 3 | 同一个绝对串被三处渲染 | `os/tools/agent_shell/run.rs::assemble_text` 与 `data.outputPath`、`runtime/pipeline.rs::persist`、`runtime/compact/envelope.rs::build_compact` |
| 4 | 入参字符串唯一来源 | `os/shared/path/args.rs::parse_path_arg` / `parse_optional_path_arg`（12 个工具模块共 16 处调用） |
| 5 | 可读路径唯一解析入口 | `os/shared/path/resolve.rs::resolve_readable`（`agent_read` 4 处、`super_read` 4 处） |
| 6 | 写方向唯一解析入口 | `resolve.rs::resolve`（`write` / `edit` / `super_edit` / `mkdir` / `list` / `shell` 的 `cwd` / `grep` / `glob`）、`resolve_leaf_preserving`（`move` / `delete` / `fs_link`） |
| 7 | 相对路径基准唯一 | `resolve.rs::resolution_base`（`resolve_base` 优先，回退 `workspace_root`） |
| 8 | 输出域根唯一来源 | `contract/context.rs::tool_output_root` ← `adapters/tool_exec.rs`（`pipeline.storage().namespace_root(session)`） |
| 9 | 域判定顺序固定「输出域优先于工作区」 | `resolve.rs::resolve_readable` 文档注释 |
| 10 | 落盘文件必为命名空间目录下的**单个文件名** | `runtime/storage.rs::result_path`（`dir.join(file)`，`file` 由 `safe_component` + `result_file_name` 生成） |
| 11 | 前端只按 `compact.persisted.path` 的**原值**裁剪标签 | `src/chat/tool/toolCallDisplay.ts::trimPersistedTag`；`ToolCallBubble.tsx` 原样展示 |
| 12 | 输出域对写方向天然关闭 | `tests/agent_write.rs`、`agent_edit.rs`、`agent_delete.rs` 的「输出命名空间只读」用例 |
| 13 | shell 临时区位置由 `SpawnPlan.scratch` 给出，子进程侧只通过 `forced_env` 的 `TEMP` / `TMP` / `TMPDIR` 访问 | `os/tools/agent_shell/spawn.rs` |
| 14 | shell 临时区当前是**调用级**：`RunGuard::drop` 结束即删，无回填、无引用 | `os/tools/agent_shell/run.rs::RunGuard` |
| 15 | 错误文本一律渲染**解析后**路径：`read` / `super_read` 的 stat / 目录 / 非文件三处、`file_revision`、`canonicalize_lenient`、`reject_system_dirs` 全部取 `resolved.display()` | `agent_read/mod.rs`、`super_read/mod.rs`、`path/resolve.rs`、`path/protected.rs` |
| 16 | 实测（引用改名 + 文件不存在）：模型把回填的 `tool-output://` 名改成 `scratch://` 后 `read` 报「文件不存在」，错误串回显域根物理路径；模型据此拼出系统 temp，随后连续三轮被沙箱拒绝（**只记事件，不记本机绝对路径**） | 会话级 scratch 根（§3.8）+ `agent_read/mod.rs` 的 stat 错误文案 |

关键结论：**改动不需要新增第二套解析路径**。F1–F4 与 F6 全部可以在既有入口内解决（F6 只需把错误文案收敛到一个渲染点，§3.10）；F5 需要新增第三个域（§3.8），但仍复用同一条解析流水线。

---

## 3 体系设计

### 3.1 语法

```
path-ref := scheme "://" body
scheme   := "tool-output" | "scratch"        // 封闭集合
body     := tool-output → 单个路径组件（输出文件名）
            scratch    → 域内相对路径（允许嵌套，可为空表示域根）
```

scheme 除了名字与域根，还带一个**正交类别**（`SchemeNature`），它才是两者真正不同的地方：

| scheme | 类别 | 含义 |
|---|---|---|
| `tool-output` | `Namespace`（平铺内容域） | body 是**平铺**的成员名；域内容由产出方生成；域本身不可写 |
| `scratch` | `Location`（可写目录重定向） | body 是域内相对路径；域**只是一个可写目录**；写方向拒绝只源于传输层 |

| 写法 | 含义 | 现状 |
|---|---|---|
| 无 scheme（绝对路径） | 本机绝对路径 | 保留（兼容历史与用户粘贴） |
| 无 scheme（相对路径） | 相对 `resolution_base` | 保留（不变） |
| `tool-output://<name>` | 本会话输出域下的单个文件 | **新增** |
| `scratch://<rel>` | 本会话 shell 临时区内的路径 | **新增**（§3.8） |

约束：

| 编号 | 约束 |
|---|---|
| G1 | scheme 为**封闭集合**。新增一项必须同时改解析、拒绝文案、接入矩阵与验收，不得只改一侧 |
| G2 | scheme 名与 `://` 之间无空格、无引号；`tool-output` 的 body 不得为空（`tool-output://` 报错），`scratch` 可为空（表域根，§3.8） |
| G3 | 无 scheme 时语义**逐字节不变**：这是兼容性的全部来源 |
| G4 | 不使用 URL 解析库。判定只用 `split_once("://")` + scheme 精确匹配：`C:\…`、`\\server\…`、`/abs/…` 都不含 `://`，不会被误判 |
| G5 | `tool-output` / `scratch` 是保留名。Unix 上若存在名为 `tool-output:` 或 `scratch:` 的目录，该目录不可作为普通路径使用（Windows 文件名不允许 `:`，无此问题） |
| G6 | 类别是**声明式**的（`Scheme::nature()` 的穷尽匹配 + `Scheme::ALL` 闭集清单），不是散落在各方向的 `match scheme`。新增 scheme 只声明它的类别，写方向文案自动就位；新增**类别**才会在 `SchemeNature` 的穷尽匹配处产生编译错误，那正是需要重新判断的地方 |
| G7 | 类别只承载两类**确有差异**的事实（写方向能否提示替代通道，§4 V7）。**不**把「哪个方向接受引用」做成按 scheme 声明的矩阵：那张矩阵按方向统一、与 scheme 无关（§4 接入矩阵），按 scheme 声明会变成 O(scheme×方向) 的冗余数据 |

### 3.2 域注册表

| scheme | 类别 | 域 | 根（唯一来源） | 可读工具 | 可写工具 | 审批动作 |
|---|---|---|---|---|---|---|
| — | — | 工作区 | `resolve_base` / `workspace_root` | `read`、`super_read` | `write` / `edit` / `super_edit` / `mkdir` / `delete` / `move` / `fs_link` | 各工具既有动作 |
| `tool-output` | `Namespace` | 本会话输出域 | `ToolUseContext.tool_output_root` | `read`、`super_read` | **拒绝** | `readToolOutput` |
| `index`（保留未启用） | — | 行索引根 | `index_root` | — | — | — |
| `scratch` | `Location` | shell 会话临时区 | `AppDirs::tool_scratch()/<session_hash>`（应用层派生为 `tool_scratch_root`） | `read`、`super_read` | **拒绝** | `readScratch` |

两个域的根**同形**：都是「已含会话段的命名空间目录」，映射都是 `域根.join(body)`。`tool-output` 的域根由 `ToolResultStorage::namespace_root` 派生，`scratch` 的域根由应用层注入点用同一套 `session_namespace` 派生（§5.4 P4.0）；body 恒为域内相对路径。会话段不进 ref（N15），故两个域的引用语法与解析路径完全一致。

注册表是**唯一**的「谁能引用谁」事实源。工具实现不得自行判断 scheme，只能声明自己需要哪种解析（见 §3.3 第 3 步）。

### 3.3 单一解析流水线

```
parse_path_arg → PathRef → resolve_* → ReadablePath / PathBuf → IO
```

| 步 | 动作 | 位置 |
|---|---|---|
| 1 | 取串并做形态校验（缺失 / 非串 / 空串 / 首尾空白拒绝） | `path/args.rs`（不变） |
| 2 | 拆 scheme：`None` 原样保留；命中已知 scheme 则校验 body（§3.4）；未命中但含 `://` 则报「未知 scheme」 | 新增 `path/ref.rs` |
| 3 | 按工具声明的能力路由：`PathRef::resolve_readable`（读）/ `PathRef::resolve_write`（写）/ `PathRef::resolve_working_directory`（`shell` 的 `cwd`）/ `PathRef::resolve_browse`（浏览）/ `PathRef::resolve_leaf_preserving`（操作链接本身）。**写方向与叶子保留方向遇到已知 scheme 时按类别返回 `write_rejected(scheme)` 的结果（`WRITE_LOCATION_SHELL_REJECTED` / `WRITE_LOCATION_NO_CHANNEL_REJECTED` / `WRITE_NAMESPACE_REJECTED`），工作目录返回 `CWD_REJECTED`，浏览方向返回 `BROWSE_REJECTED`**（§4 四类文案）；无 scheme 时五者各自转调既有 `resolve_readable` / `resolve` / `resolve_leaf_preserving`（`resolve_browse` / `resolve_working_directory` 与 `resolve_write` 共用 `resolve`） | `path/ref.rs` → `resolve.rs` |
| 4 | 拼根：`tool-output` → `tool_output_root.join(body)`；`scratch` → `tool_scratch_root.join(body)`（两者域根都已含会话段，见 §3.2）；无 scheme → `join_input` 原逻辑 | `path/ref.rs` |
| 5 | 归一与边界：`canonicalize_lenient` → `reject_system_dirs` → 域判定（输出域 / scratch 域优先）→ 工作区边界 | `resolve.rs`（复用，不改判定顺序） |
| 6 | 返回 `ReadablePath { path, scope }`；写方向返回 `PathBuf` | 现状类型 |

最低层强制（这是「整个应用都能认」的机制）：

| 编号 | 机制 |
|---|---|
| E1 | `join_input` 增加断言式拒绝：入参若含已知 scheme，一律返回内部错误（`internal: path ref '<raw>' reached join_input`）。**文案分工固定**：业务拒绝由 `PathRef` 的三个入口在调用 `join_input` 之前给出（§3.3 第 3 步 / §4 统一文案）；`join_input` 只兜底，命中即说明有代码绕过了 `PathRef`（E2），属于缺陷而非用户输入错误。它意味着**没有任何代码路径能把 `tool-output://x` 静默拼成工作区相对路径**——最坏情况是明确报错，不是错读 |
| E2 | `PathRef` 为私有字段的 newtype，只能由 `parse_path_arg` / `parse_optional_path_arg` 构造（`cfg(test)` 额外提供字面量构造）。工具若绕过它直接读 `args["path"]`，会撞上 E1 |
| E3 | `parse_path_arg` 返回类型由 `String` 改为 `PathRef`：**17 处**构造点在编译期被强制迁移（`super_read` / `super_edit` / `fs_link`×5 / `write` / `read` / `move`×2 / `mkdir` / `list` / `grep` / `glob` / `edit` / `delete`），无一处可遗漏 |

### 3.4 边界与逃逸

`tool-output://<body>` 的 body 必须满足（scratch 的 body 规则见 §3.8「语法」，允许嵌套但不放宽下列任何一条）。**校验发生在 `parse_ref`（解析期），与域根是否存在无关**：body 非法时报的是「引用格式错误」，不报「文件不存在」；域根缺失（`tool_output_root` 为 `None`）才报 §3.4 失败语义里的「no tool output root」：

| 判据 | 拒绝理由 |
|---|---|
| 只有 1 个 `Component::Normal` | 多组件或 `RootDir` / `Prefix` / `ParentDir` 会让 `Path::join` **替换**根（`root.join("C:\\Windows")` = `C:\Windows`），等于绕过域边界 |
| 不含 `..`、`.`、`/`、`\`、`:` | 同上；`..` 亦被 `canonicalize_lenient` 拒绝，此处提前给出更准确的文案 |
| 非空、无首尾空白、无 NUL | 与 `parse_path_arg` 的形态校验同口径 |
| 非 `\\?\` / `//?/` 形态（verbatim） | **拒绝而非剥离**：剥离后 `\\?\C:\x` 会退化成合法绝对路径，等于绕过域边界。剥离只发生在 `canonicalize` 内部（§3.9 / N17） |
| 拼根后仍过 `ensure_within_root(output_root)` | 命名空间内的符号链接指向域外时由 `canonicalize` 展开后拦下 |

失败语义（不回退）：

| 场景 | 结果 |
|---|---|
| `tool_output_root` 为 `None`（ToolBox 无会话归属） | 业务错误「no tool output root」，**不**回退工作区 |
| 域内无该文件 | 业务错误「file not found」，**不**去工作区找同名文件 |
| 串来自其它会话的输出（用户粘贴） | 在当前会话命名空间内查找 → 不存在 → 同上。不提供跨会话回读 |
| `scratch://` 的 body 越出会话临时区 | 逐段校验 + `ensure_within_root` 拦下；`canonicalize` 后若落到域外则报「scratch escape」，不跨域查找 |
| 本会话从未调用过 `shell`（scratch 域根不存在） | 业务错误「scratch not found」，**不**顺手创建空目录（创建只由 `shell` 触发） |

### 3.5 审批口径

| 编号 | 规则 |
|---|---|
| P1 | `ReadableScope → (action, resource)` 收敛为**一个**函数（`ReadablePath::intent(context)`），`read` 与 `super_read` 共用，消除 F4 |
| P2 | `ToolOutput` 恒 `readToolOutput`，`resource` 取命名空间**目录**（现状不变）：审批键覆盖本会话全部输出 |
| P3 | 同一文件以 `tool-output://<name>` 或绝对路径读，action 与 resource **逐字节相同** → 命中同一授权键。两种写法不得出现两次弹框 |
| P4 | 审批卡片继续只展示文件名（不展示 ref、不展示绝对路径） |

`scratch` 域的审批规则（`readScratch` / 会话级 resource / 写方向拒绝）见 §3.8「审批」。

### 3.6 渲染口径（产出侧）

| 编号 | 规则 |
|---|---|
| R1 | 新增 `PersistedRef::model_ref()`：由 `path` 的文件名派生 `tool-output://<name>`。**不新增字段、不改落盘格式**（§2 第 10 行保证文件名即唯一标识） |
| R2 | 模型可见的路径串一律用 `model_ref`：`<persisted-output path="…">` 标签（`run.rs::assemble_text`、`pipeline::persist`） |
| R3 | UI 可见的 `compact.persisted.path` 用 `model_ref`：卡片不再展示本机绝对路径，与 I52 §5.11.4(5)「卡片不暴露本机路径」的既有意图一致 |
| R4 | `PersistedRef.path` 保持绝对：内部 IO、`result_stats`、测试直接 `fs::read_to_string` 都依赖它 |
| R5 | shell 结果恒回填一行 `scratch://`（会话临时区根 ref），非空时附一层子项名清单（≤20，超出 `…(+N more)`）（§3.8「双通道」） |
| R6 | `data.outputPath` 保持绝对：它不进模型上下文（§2 第 1 行），且 15 处 shell 用例以它为 IO 入口 |
| R7 | 前端**零逻辑改动**：`trimPersistedTag` 按原值裁剪，ref 与绝对串都能对上；`parsePersistedTag` 只做字符串回放 |
| R8 | 历史行（绝对串标签）继续可读、可渲染：`resolve_readable` 的绝对路径分支不变 |

### 3.7 引用 ↔ 存储路径的映射

映射**不是查表**，是结构性恒等式：ref 的 body 就是落盘文件名本身，根由上下文提供。

```
ref  = "tool-output://" + <file>                       // 产出侧（model_ref）
path = ctx.tool_output_root.join(<file>)               // 消费侧（resolve）
ctx.tool_output_root = storage.namespace_root(Some(session))
                     = storage.root.join(session_namespace(session))
<file> = "{unique_prefix}-{safe_component(tool_name)}-{result_file_name(call_id)}.txt"
```

往返证明成立的全部依据：

| 条件 | 内容 | 现状保证 | 位置 |
|---|---|---|---|
| M1 | 落盘文件**直接**位于命名空间目录下（平铺，无嵌套） | `result_path` 只做一次 `dir.join(file)` | `runtime/storage.rs::result_path` |
| M2 | 文件名在目录内唯一 | `unique_prefix()` = 12 位纳秒 + 4 位进程内自增序号 | `runtime/storage.rs::unique_prefix` |
| M3 | 命名空间可由会话 id 纯函数重算 | `session_namespace` = 会话 id 的 FNV-1a；`None` → `shared` | `runtime/storage.rs::session_namespace` |
| M4 | 生产与消费取同一个根 | `tool_output_root` 只从 `storage.namespace_root` 取，不自行拼接 | §2 第 8 行 |
| M5 | 只有一个文件名生成器 | `persist` 与 `open_sink` 都走 `result_path` | `runtime/storage.rs` |

于是：

```
resolve(model_ref(path)) = tool_output_root.join(file_name(path))
                         = path.parent().join(file_name(path))
                         = path                                      // 由 M1
```

这是**可证明的逆运算**，不是前缀搜索、不是模糊匹配。由此得到的性质：

| 性质 | 结论 |
|---|---|
| 无需注册表 / 索引文件 / 数据库 | 名字即键，映射零状态 |
| 无需给 `PersistedRef` 加字段 | `model_ref` 由 `path` 的 `file_name()` 派生（R1） |
| 跨进程重启仍可解析 | 会话 id 持久、命名空间哈希确定、文件仍在（未被保留策略清理） |
| 串里不含会话 id | ref 语义是「本会话输出域内」，会话由上下文提供 → 跨会话引用**按构造不可读**，不靠额外校验 |

失效语义（不回退，见 N6）：

| 场景 | 结果 | 是否可接受 |
|---|---|---|
| 保留策略已清理该文件 | 「file not found」 | 是：产物本就可重建 |
| 换会话后读旧 ref | 「file not found」（命名空间不同） | 是：禁止跨会话回读 |
| 存储实现不提供 `namespace_root`（`tool_output_root = None`） | 「no tool output root」 | 是（A7） |
| 未来改为嵌套目录 | 必须同时改 `model_ref` 与本文（N7） | 由 N7 约束 |

本节只论证 `tool-output` 域。`scratch` 域是**目录引用**，映射为 `tool_scratch_root.join(<rel>)`（域根已含会话段，§3.2）：body 是纯域内相对路径（可空、可嵌套），逐段拒绝 `..` 与绝对组件（§3.8「语法」）。它与本节的差别只在「body 不是单个文件名」与「不要求平铺」，往返仍成立：域根由上下文纯函数重算（会话段同 M3，`AppDirs` 基根由应用层注入），body 由模型照抄回填值，无查表。

唯一**不可检测**的失效模式是 M2 被破坏（同名覆盖）：ref 会解析到另一个文件，读到错内容而不报错。因此 M2 不是「顺带成立」而是硬前提：`unique_prefix` 的时间戳 + 进程内自增序号不得移除或削弱。若将来引入「同一 `tool_call_id` 重试复用同名文件」的优化，必须同时改 `model_ref` 的派生方式。

被否决的替代方案：

| 方案 | 否决理由 |
|---|---|
| 注册表（ref id → 绝对路径） | 引入第二事实源（违反 N1）；需并发写、GC、陈旧条目处理；目录改名即全表失效 |
| 把绝对路径编码进 ref（`tool-output://<url-encoded abs>`） | 长度回到 100+（F1 未解）；把本机路径带进模型上下文 |
| 内容哈希作键 | 相同输出合并成一个文件 → 破坏「一次调用一个文件」与 `bytes` 口径；且需遍历目录反查 |
| 仅用 `result_file_name(call_id)` 作键 | 丢掉 `unique_prefix` 的防撞能力（同一 call_id 重试会覆盖），即破坏 M2 |

#### 3.7.1 权威源链

映射的每一段都只有一个**写入方**，其余使用点均为只读派生：

| 段 | 唯一权威 | 站点 | 其它使用点（只读） |
|---|---|---|---|
| 存储根 | `AppDirs::tool_results()` ← `dir_name::TOOL_RESULTS` | `app_dirs/roots.rs::tool_results` | `app_dirs/registry.rs`（存储管理条目）、`tool_setup.rs`（沙箱拒写名单） |
| 存储实例 | 生产仅一处构造 | `tool_setup.rs`（`FileToolResultStorage::new(dirs.tool_results())`） | 测试各自 `new(tempdir)` |
| 命名空间 | `session_namespace(session_id)` | `runtime/storage.rs::session_namespace` | 生产代码只有两处调用：`namespace_root`（输出域）与 `scratch_root_for`（scratch 域，§5.4 P4.0）；`scratch_root_for` 本身只有两个应用层调用点（注入 `adapters/tool_exec.rs` / 清理 `cmd/conversation/mutate.rs`）。trait 文档明文禁止其它地方自行 `root.join(session_namespace(..))`（N1） |
| 文件名 | `result_path(&PersistToolResult)`（**私有**，非 `pub`） | `runtime/storage.rs::result_path` | `persist` 与 `open_sink` 都经它；`FileSpillSink` 的文件由 `open_sink` 传入 |
| 域根注入 | 输出域 `pipeline.storage().namespace_root(Some(session))`；scratch 域 `scratch_root_for(dirs.tool_scratch(), session)` | `adapters/tool_exec.rs`（注入）、`cmd/conversation/mutate.rs`（清理） | 全工程仅此一处给 `tool_output_root` / `tool_scratch_root` 赋值；会话段拼法唯一实现在 `runtime/storage.rs::scratch_root_for` |

`session` 字符串同样只有一个来源：`adapters/tool_exec.rs` 里 `let session_id = ctx.conversation_id.to_string()`，它同时用于

| 用途 | 站点 |
|---|---|
| 派生 `tool_output_root` | `tool_exec.rs`（`namespace_root(Some(&session_id))`） |
| 填 `context.session_id` | `tool_exec.rs`（`session_id: Some(session_id)`） |
| 流水线落盘取会话 | `runtime/pipeline.rs`（`self.persist(outcome, context.session_id.as_deref())`） |
| shell 流式落盘取会话 | `os/tools/agent_shell/run.rs`（`session_id: context.session_id.clone()`） |

四处使用、一个来源，故生产路径上 `tool_output_root` 与落盘命名空间必然一致。

**唯一薄弱环节（W1）**：`ToolUseContext.tool_output_root` 是**脱离的值**，类型上不约束它与同一上下文里的 `session_id` 同源。现状只靠「派生只有一处」这条约定；一旦新增适配器或测试手工构造不一致的组合，ref 会解析进别的命名空间，症状是「file not found」——可判定但误导（看起来像产物过期，实际是上下文接错）。测试代码已在做这种构造（`tests/gateway.rs` 直接给 `tool_output_root: Some(tempdir)` 且 `session_id: Some("conv-1")`），说明类型允许不一致。

**加固（建议纳入 P1）**：把派生点从适配器移到流水线。`ToolPipeline` 同时持有 `storage` 并接收 `context`，因此可以在 `execute` 入口构造一份派生上下文：

```
let derived = ToolUseContext { tool_output_root: self.storage.namespace_root(context.session_id.as_deref()), ..context.clone() };
```

下游（网关、工具、`persist`）一律使用 `derived`。这样 `tool_output_root` 与落盘用的 `session_id` 出自**同一次调用、同一个 `storage` 实例**，M4 从「约定」升级为「构造即一致」，适配器不再有机会算出不同的根。代价是 `execute` 内多一次上下文克隆（`ToolUseContext` 持有 `Arc` 字段，克隆廉价）。

### 3.8 scratch 域：shell 临时区

#### 定位

shell 调用会产生大量中间文件（`cargo` 的 target、`npm` 的缓存、`tempfile` 的临时目录）。这类文件有两个刚性要求：① 必须落在沙箱**可写根**内，否则写入即被拒；② 不能留在用户工作区，否则 `git status` 长期可见、`git add -A` 会被误提交。

本体系把它作为**第三个域**正式纳入，与工作区、输出域并列：根、命名、引用语法、审批、清理全部走同一套结构，不设「某个工具的点目录特例」。

#### 域根与命名

```
AppDirs::tool_scratch()/<session_hash>/
```

`session_hash` 复用 `session_namespace`（FNV-1a），与输出域命名空间同一套哈希。基根由 `AppDirs` 唯一给出，会话段由应用层注入点一次派生（§5.4 P4.0），故 `ToolUseContext.tool_scratch_root` 与输出域的 `tool_output_root` **同形**（都已是命名空间目录）。**不再有 `<call_id>` 层**：临时区是会话级共享目录，理由见「为什么是会话级单目录」。

命名判据：

| # | 判据 |
|---|---|
| C1 | 与系统 temp 语义**不混淆**（名字含 `temp`/`tmp` 会让用户去 `%TEMP%` 找，也让模型以为是 OS 管的） |
| C2 | 与**代码词汇**一致（现有代码已是 `SCRATCH_DIR_NAME` / `prepare_scratch` / `cleanup_scratch` / `scratch_dir_failed` / `SpawnPlan.scratch`；W6 删除 `SCRATCH_DIR_NAME` / `cleanup_scratch` / `SpawnPlan.scratch` / `scratch_component`，词汇族名（`prepare_scratch` / `scratch_dir_failed`）保留） |
| C3 | 与 scheme 同名（`scratch://`） |
| C4 | 前缀粒度正确（不叫 `shell-*`：临时区将来可能被别的工具复用） |
| C5 | 在存储管理 UI 里可读（作为条目 key 出现，须让用户看出「可删、删了只丢临时文件」） |

| 候选 | C1 | C2 | C3 | C4 | C5 | 判定 |
|---|---|---|---|---|---|---|
| **`tool-scratch`** | 过 | **过** | **过** | 过 | 过 | **采纳** |
| `tool-temp` | **不过**：与 `%TEMP%` / Windows `Temp` 同名族 | 不过 | 不过 | 过 | 中 | 否决 |
| `scratch` | 过 | 过 | 过 | 过 | 弱（太泛，看不出归属） | 次选 |
| `shell-temp` | 不过 | 不过 | 不过 | **不过** | 过 | 否决 |
| `tool-spill` | 过 | 不过 | 不过 | 过 | 弱（术语生僻） | 否决 |
| `sandbox-scratch` | 过 | 过 | 不过 | 过 | 中 | 否决：该目录在可写根**之外**，叫 sandbox 反误导 |

排序收益：`tool-index` → `tool-results` → `tool-scratch` 在存储管理列表里天然相邻，用户能看出是同一类可重建产物。

#### 语法

`scratch://<域内相对路径>`

| 判据 | 规则 |
|---|---|
| body | 域内相对路径，**允许嵌套**；空 body 或单个 `/` 表示会话临时区根本身 |
| 禁止 | `..`、`.`、绝对路径、盘符、`/` 前导、反斜杠、NUL、首尾空白；尾部单个 `/` 仅用于标记「目录根」，不产生空段 |
| 拼根 | `tool_scratch_root.join(其余)`（域根已含 `<session_hash>`，§3.2 / P4.0），逐段 join，不做字符串拼接 |
| 边界 | 拼根后仍须过 `ensure_within_root(tool_scratch_root)`；域内符号链接指向域外由 `canonicalize` 展开后拦下 |

与 `tool-output://<file>` 的关键差异：后者的 body 恒为单组件（§3.4 / N7「平铺」），scratch 必须支持嵌套，因为临时区是真实目录树。**N7 的平铺约束只适用于输出域**，不得推广到 scratch。

body 中**不含**会话标识与调用标识：会话由上下文提供（与输出域 ref 不含会话 id 同理，§3.7），调用标识在共享临时区里没有语义。这使 ref 短到 `scratch://out.log` 这种量级——正是 F1 要消灭的照抄负担。

失败语义与输出域同口径（§3.4 失败表）：域根缺失、文件不存在，一律业务错误，**不**回退工作区、不换域重试。

#### 双通道

环境变量与 `scratch://` 服务**不同消费者**，不构成替代关系：

| 消费者 | 谁在读 | 通道 |
|---|---|---|
| A. 被 spawn 的子进程及其调用的第三方程序（`cargo` / `npm` / `python tempfile`） | 子进程自己 | **只能**是环境变量 `TEMP` / `TMP` / `TMPDIR`——第三方程序不认识我们的 scheme |
| B. Agent 的其它工具（`read` / `super_read`） | 应用进程 | `scratch://` |
| C. Agent 在命令串里显式指名位置 | 子进程 | 环境变量（`%TEMP%` / `$TMPDIR`） |

消费者 A 是重定向的**本体且不可替代**：`forced_env` 强制注入三条变量，让任意程序调 `tempfile` 时落在本会话的临时区；否则它们会写到系统 temp，而系统 temp 不在沙箱授权内，直接被拒。

**模型如何得知 ref**：`shell` 结果里恒回填一行 `scratch://`（会话临时区根引用），并附该目录的**直接子项名**清单（最多 20 项，超出以 `…(+N more)` 结尾）；目录为空时只回填根引用、不带清单。回填值与是否本次调用产生文件无关——因为目录是会话级共享的，清单反映的是「当前临时区里有什么」，这正是 handoff 所需的视图。

清单是必要的：`list` / `glob` 对任何 scheme 都拒绝（§4），模型没有第二个「列举」通道。清单只给**名字**（不含大小、时间），且只列一层，不递归——避免结果膨胀与「隐式目录遍历」。

跨调用 handoff 由此真正成立：第 1 条命令把结果写进 `$TMPDIR/out.txt`，第 2 条命令直接 `read` `scratch://out.txt`，或在命令串里用 `$TMPDIR/out.txt` 继续处理——**两个消费者看的是同一个目录**。

| 编号 | 决定 |
|---|---|
| T1 | 环境变量是子进程侧唯一的「指名」通道，保持三条注入；**不新增** `ASSISTANT_SCRATCH` 之类专用变量——它会诱导模型把临时区当持久区，且 `ASSISTANT_` 在 env 侧本就被 `is_sensitive_name` 当作不外传族 |
| T2 | 模型侧唯一的「指名」通道是 `scratch://`；ref 不含会话段与调用段，模型无从拼出其它会话的路径，运行时也不接受非本会话的域根 |
| T3 | `shell` 描述必须写明「临时目录为会话级共享、保留到会话结束、同名文件会覆盖、可用回填的 `scratch://` 引用回读」——否则模型不知道能回读，或误以为调用结束即失效 |

#### 生命周期与清理

启用 ref 的前提是目录在调用结束后**仍然存在**，故清理时机整体后移：

| 时机 | 动作 |
|---|---|
| 会话首次 shell 调用 | `create_dir_all(<session_hash>)`，**幂等**；已存在则直接复用，不删不重建 |
| 调用结束 | 目录**不动**；`RunGuard::drop` 只 kill 进程（`cleanup_scratch` 调用点删除） |
| 会话结束 | 删除 `<session_hash>` 整棵子树；落点为 `cmd/conversation/mutate.rs` 的三个入口 `delete_conversation` / `batch_delete_conversations` / `purge_conversations`，`NotFound` 静默 |
| 存储管理 | 条目只**观测**（`action: None` / `NotImplemented`），不提供独立回收动作；`SweepExpired` 不可用（没有执行器） |

代价：长会话可能积累数 GB（大构建产物），因此该条目必须 `risk: Low` + `inventory: false` + 在存储管理里显式可见（见登记表字段）。

清理实现口径（避免两处各写一份）：

| 事项 | 规则 |
|---|---|
| 取根 | 会话结束清理用 `scratch_root_for(dirs.tool_scratch(), Some(id))`，与注入侧同一实现点（N1） |
| 删除 | `std::fs::remove_dir_all`，`NotFound` 静默（会话从未调用过 shell 属正常） |
| `spawn::cleanup_scratch` | 调用点全部移除后即为死代码：**删除该函数**，不要留 `pub` 空壳（`pub` 也不豁免 `dead_code` 之外的语义混乱）。会话级删除走上面两条，不跨层复用 shell 模块的私有工具 |

#### 为什么是会话级单目录

早期设计在 `<session_hash>` 下再加一层 `<call_id>`，让「一次调用一个目录」。否决理由：

| # | 理由 |
|---|---|
| D1 | **破坏 handoff**：`TEMP` / `TMP` / `TMPDIR` 只能指一个目录。若指向 `<call_id>`，第 2 次调用的 `TMPDIR` 就是它自己的目录，模型无法把第 1 次产出的临时文件交给第 2 条命令使用——只剩「回读」一项能力，「临时区」语义退化为「一次性输入区」 |
| D2 | **自相矛盾**：`call_id` 不保证唯一，须再加唯一段（纳秒 + 自增），ref 变成 `scratch://20261124T…-<call_id>/out.log`，长度回到 F1 要消灭的量级 |
| D3 | **破坏缓存**：`pip` / `maturin` / 部分构建系统把中间产物放 `TMP`，每次调用换目录等于每次全量重来 |
| D4 | **整节风险消失**：不再需要「同名目录先 `remove_dir_all` 再重建」，也就不存在「同 `call_id` 重试互删」「并发删掉正在运行的目录」 |
| D5 | **清理粒度本就在会话级**：S2 已是「会话结束删整棵」，`call_id` 层不提供任何额外回收能力，只是多一层无用嵌套 |
| D6 | **产物标识属输出域**：一次调用一个文件 + `unique_prefix` 是 `tool-output` 的职责。把「产物标识」叠进 scratch，会让 N7（输出域平铺）与 S5（scratch 允许嵌套）两套约束在同一批目录上互相牵制 |

代价（显式接受，不隐含）：

| 代价 | 说明 | 处置 |
|---|---|---|
| 失去 per-call provenance | 无法「只列本次调用产生的文件」；要做就得运行前后 diff 或全树扫描，而条目已定 `inventory: false` | 回填不依赖目录非空判定：恒回填根 ref，清单反映「当前临时区有什么」 |
| 跨调用陈旧 / 同名覆盖 | 命令失败时旧文件仍在，`read` 读到上一次内容而不报错 | 共享 temp 的固有语义，也正是 handoff 能力本身；T3 要求在描述里明说「同名会覆盖」 |
| 并发同名写 | 同会话两个并发 shell 写同一名字互相覆盖 | 由命令作者用子目录区分（`$TMPDIR/<name>`），运行时不强制加段 |

#### 审批

| 编号 | 规则 |
|---|---|
| P5 | `Scratch` 恒 `readScratch`，`resource` 取 `<session_hash>` **目录**：一次授权覆盖本会话全部临时区，避免每次调用弹框 |
| P6 | 写方向一律拒绝：scratch 与输出域同属应用内部域，写入只由子进程产生，模型不得 `write` / `edit` / `delete` / `move` 进入。**这是当前阶段的取舍，错误文案只写「本工具当前不接受 scheme」，不写「只读」**（§4 V1） |
| P7 | 同一文件的两种写法（`scratch://` 与绝对路径）命中同一审批键，不得弹两次 |
| P8 | 审批卡片展示 `scratch://` 引用与域内相对路径，不展示本机绝对路径 |
| P9 | `preview_for_approval` 的「非输出域即返回 `None`」分支必须同步纳入 `Scratch`（`agent_read` / `super_read` 各一处）：否则临时区读取虽能审批，却没有预览卡片，与输出域体验不一致 |

#### 为什么不用系统 temp

| # | 障碍 | 后果 |
|---|---|---|
| 1 | Windows 的 `%TEMP%` 在**用户主目录**内（`%LOCALAPPDATA%\Temp`） | `acl.rs::prepare` 的硬约束写明「敏感目录、系统目录、**用户主目录**一律不动」，授权即违规 |
| 2 | Linux 的 `/tmp` 世界可写 + 粘滞位 | 同机其它用户可读中间文件（常含 token、构建产物、下载内容），真实隐私泄漏面；符号链接 / TOCTOU 攻击面 |
| 3 | macOS 的 temp 在 `/var/folders/...`，真实路径 `/private/var/...` 且按用户按进程随机化 | SBPL 须写真实路径，且跨运行不稳定，引用无法复用 |
| 4 | 外部清理器不受我们控制 | Storage Sense / periodic / systemd-tmpfiles 可能在命令**运行中**删除目录，兜底清理的假设失效 |
| 5 | 容量与生命周期不可控 | Linux 的 `/tmp` 常为 tmpfs 且有上限，大产物触发 ENOSPC 且报错离因很远 |

一句话：**系统 temp 归操作系统所有，而我们需要「应用拥有、应用清理、保留策略可管」的目录。**

#### 登记表字段

| 字段 | 取值 | 依据 |
|---|---|---|
| `dir_name::TOOL_SCRATCH` | `"tool-scratch"`，并加入 `dir_name::ALL` | 字面量只在一处（`dir_name.rs` 首行注释即此规则） |
| `AppDirs::tool_scratch()` | 走 `path_for(TOOL_SCRATCH)` | 与 `tool_results()` / `indexes()` 同形态 |
| `CoreEntry::ToolScratch` | 新条目 `E19` | 封闭枚举，编译器会点名**四处** `match`：`entries/decl.rs::static_for`、`entries/eligibility.rs::eligibility_for`、`reclaim/execute.rs::execute_storage_reclaim`（主路由）、`reclaim/execute.rs::executor_action`（白名单）。**`sample/collect.rs` 不是 `match`**（靠人工追加任务），编译器不点名，故采样必须与登记同批落地（见下两行）；`registry.rs` 的「12 条 / E1–E6、E12–E15、E17–E18」文档注释也要一并改 |
| 静态声明 | `action: None`、`eligibility: NotImplemented`、`reason: NoReclaimApi`，`note` 写明「会话结束时随会话一并删除，这里只展示占用情况」 | 与 `E13` 崩溃条目同档：没有独立回收 API。**不得给 `SweepExpired`**——`executor_action` 会据此放行一个 `execute_storage_reclaim` 主路由里没有实现的动作，两处必须同档。复用既有 DTO 变体，**无 `contract:gen`** |
| 采样 | `sample/collect.rs` 增一条 `timed_blocking(CoreEntry::ToolScratch.id(), sample_scratch)`；`sample/rows.rs::sample_scratch` 直接复用 `app_dirs::fs_stats::tree_stats`（与 `sample_logs` 同口径） | **漏采样 ⇒ `declared = 0` ⇒ 整棵树被误报成余量**（`E12` 的同款 G11 缺陷）；该处无编译期保护，只能靠清单约束 |
| `scope` | **`Local`** | scratch 是「大、易失、机器本地」，`Data` 在 Windows 上会漫游（主动造成坏处）。现有 `Local` 家族正是 `logs` / `crashes` / `EBWebView` |
| `coverage` | `RecursiveTree` | 整棵子树归本条目 |
| `risk` | `Low` | 删了只丢临时文件 |
| `inventory` | **`false`** | 与 `E12` 日志条目同构：长命令运行期文件持续增删，登记值与扫描值来自两个时刻，减法必产生假余量 / 假负数 |
| 字面量 | 唯一来源 `dir_name::TOOL_SCRATCH`（`"tool-scratch"`），shell 侧不得再持有独立常量 | 字面量只在一处（`dir_name.rs` 首行注释即此规则） |

#### 前置改造：多可写根

| # | 改动 | 位置 |
|---|---|---|
| W1 | `SandboxPlan.writable_root: PathBuf` → `write_roots: Vec<PathBuf>`；`cwd` 归属校验改为「在任一根内」 | `sandbox/mod.rs`、三后端 |
| W2 | 逐根做 `paths_overlap` 敏感检查；Windows 逐根 `converge_ace` | `macos.rs`、`linux.rs`、`windows/acl.rs` |
| W3 | `sensitive_read_deny` 从「整树拒绝 `dirs.local()`」收窄为逐项枚举 | `tool_setup.rs::sandbox_host_paths` |
| W4 | `forced_env` 的 `TEMP` / `TMP` / `TMPDIR` 指向 `tool_scratch/<session_hash>`（会话级单目录） | `agent_shell/spawn.rs` |
| W5 | 新增 `dir_name::TOOL_SCRATCH` + `AppDirs::tool_scratch()` + 登记表条目 | `app_dirs/` |
| W6 | `prepare_scratch` 改为 `create_dir_all` 幂等复用（不 `remove_dir_all`）；`scratch_component` 删除（不再参与目录命名，也无其它用途） | `agent_shell/spawn.rs` |
| W7 | `acl.rs::to_wide` 的入参必须先经 `canonicalize_lenient` 归一；长度 ≥248 时由调用方补 `\\?\` 前缀（M3），**不**依赖 exe 清单的 `longPathAware`（M2 未声明，且依赖用户机器注册表开关） | `sandbox/windows/acl.rs` |

W3 是真正的评审点：现在 `sensitive_read_deny` 直接放整棵 `dirs.local()`，macOS / Linux 后端对「可写根与敏感目录重叠」是**直接判负**（`writable_root_overlaps_sensitive`），不是警告；收窄等于放弃对将来新增文件的默认保护，属防御纵深削弱。另注：`data_local_dir` 在 macOS / Linux 上等于 `data_dir`，故整树拒绝在非 Windows 上覆盖了整个应用数据区。

W1 与 W4 的耦合：`cwd` 仍须落在工作区，因此 scratch 是「第二写根」而非「替换根」——这种形态最省，也最贴合「工作区仍可写、临时区外置」。

W6 的意义：会话级目录的唯一危险操作就是「先删再建」。改成幂等 `create_dir_all` 后，同会话重试与并发调用都不会互删；`scratch_component(call_id)` 的唯一用途（调用级目录名）随会话级目录消失，函数一并删除，不留死代码。

#### 结论

| 编号 | 结论 |
|---|---|
| S1 | scratch 是本体系的**第三个域**，与工作区、输出域并列；根由 `AppDirs::tool_scratch()` 唯一给出，不再落在用户工作区内 |
| S2 | 生命周期从「调用级」改为「会话级」：会话首次使用时幂等创建，调用结束保留，会话结束（删除 / 批量删除 / 清空）时删除。这是启用 ref 的唯一前提 |
| S3 | 双通道固定分工（T1 / T2）：子进程只认 `TEMP` / `TMP` / `TMPDIR`，应用内工具只认 `scratch://`，ref 只由 shell 回填产生 |
| S4 | scratch 域对模型**只读**（P6），写入只由子进程产生；与输出域同一规则，不设例外 |
| S5 | 语法允许嵌套（与输出域的平铺约束不同），因为临时区是真实目录树；N7 不推广到本域 |
| S6 | 落地前置是多可写根改造（W1–W7），其中 W3（收窄 `sensitive_read_deny`）是唯一的防御纵深削弱点，必须随本域一并评审 |
| S7 | 不落系统 temp（理由见上表），不新增用户设置项：位置由授权集合决定，不是偏好 |
| S8 | `shell` 描述必须写明会话级共享、同名覆盖、回填 ref（T3），否则模型不知道能回读，或误以为调用结束即失效 |
| S9 | **域内不再分层**：域根即 `<session_hash>` 目录，ref 的 body 是纯域内相对路径。这是 D1–D6 的直接结论，也是本域与输出域唯一的形态差异 |

### 3.9 路径形态（`\\?\`）的处置

沿用 I44 §10.2 的三形态定义：**IO 形态**（`canonicalize` 原样，Windows 带 `\\?\`）/ **绑定形态**（原样字符串，用于比较、持久化、键）/ **展示形态**（`strip_verbatim_prefix` 后，仅 UI、日志、wire 展示）。

| 位置 | 是否剥离 | 依据 |
|---|---|---|
| scheme 的 body | **不剥离，直接拒绝** | `tool-output` 的 body 只允许 1 个 `Component::Normal`；`scratch` 逐段拒绝绝对组件、盘符、`/` 前导、反斜杠。`\\?\C:\x`、`//?/C:/x`、`\\?\UNC\srv\share` 全部落在这些判据上，报 `invalid`（§3.4 / §3.8「语法」） |
| 无 scheme 的绝对串 | **不变，已覆盖** | `canonicalize_lenient` → `canonicalize_existing` → `simplify_verbatim`（`dunce` + `\\?\UNC\` → `\\`）。既有测试 `verbatim_prefix_is_stripped` / `unc_verbatim_prefix_is_simplified` / `malformed_verbatim_prefix_is_not_resolved_against_cwd` 三条已在 `path/mod.rs` |
| 产出侧的 ref | **构造上不可能** | ref 的 body 取自 `file_name()` 或域内相对段，不含前缀；`data.outputPath` 走的是同一份已简化路径（R5） |
| **域根本身** | **必须归一一次**（新增面） | 见下 |

**为什么不能「先剥离再校验」**：剥离后 `\\?\C:\Windows` 会退化成合法绝对路径，等于把「body 是域内单段文件名」换成「body 是任意绝对路径」，域边界直接失效。形态校验与边界校验是同一个判断，剥离发生在边界之后（`canonicalize` 内部），不能提前。

**域根归一一次**是唯一真正新增的约束：

| 事项 | 规则 |
|---|---|
| 归一动作 | 域根在**构造点**做一次 `canonicalize_lenient`，之后只以该结果参与 `join` / `ensure_within_root` / 回填 |
| 为什么 | `ensure_within_root` 只对 **root** 做宽松归一（`membership.rs`），要求 **path 侧**已归一。两侧形态不一致时，`component_prefix_match` 逐组件比较失败，域内文件会被误判成「域外」 |
| 何时会踩到 | `std::fs::canonicalize` 在 Windows 上**返回** verbatim 形态——只要有一处漏掉 `simplify_verbatim`，立刻踩到。当前 `tool_output_root` / `tool_scratch_root` 都来自 `AppDirs`（`dirs` 给出非 verbatim），但 `FileToolResultStorage::root` 是构造参数，装配层可注入任意串，不得依赖「目前恰好不是」 |
| 幂等 | `canonicalize_lenient` 对已归一的路径是恒等操作，重复调用无副作用；不需要在每处 `join` 前再调 |

#### 长路径：三种机制，互不等价

`\\?\` 除「形态」外还有第二个含义（突破 260 字符），与剥离无关，但必须分清是哪一种机制在起作用：

| 机制 | 作用域 | 触发条件 | 是否需声明 |
|---|---|---|---|
| M1 Rust std 自动加前缀 | 本进程内经 `std::fs` 的所有调用 | 路径不足 248 个 UTF-16 单元（含结尾 NUL）时原样使用；达到后经 `GetFullPathNameW` 归一并补 `\\?\`（长 UNC 补 `\\?\UNC\`）。已带前缀者提前返回 | **不需要**，编译进二进制 |
| M2 进程清单 `longPathAware` | 本进程内经**非 std** 的 Win32 调用（裸 `windows-sys` 传宽字符） | 清单声明 + 系统 `LongPathsEnabled=1`，两者**缺一不可** | 需在 exe 清单声明 |
| M3 调用方显式传 `\\?\` 前缀 | 任意一次 Win32 调用 | 该串自带前缀即绕过 MAX_PATH 检查，**不依赖** M2 | 不需要，但要保证串是绝对且已归一 |

本项目的实际状态（已核实）：

| 事实 | 依据 |
|---|---|
| **M2 未声明** | `tauri-build` 默认清单只有 Common-Controls v6 一项，无 `longPathAware`；`src-tauri/build.rs` 未覆写 `windows_attributes.app_manifest` |
| **M1 已生效** | Rust std 自带；`std::fs` 路径 ≥248 自动前缀，无需任何配置 |
| **M3 未覆盖** | `sandbox/windows/acl.rs::to_wide` 直接 `encode_wide`，不加前缀，交给 `GetNamedSecurityInfoW` / `SetNamedSecurityInfoW` |

因此「longPathAware 是默认能力」只在 M1 意义上成立：**`std::fs` 通道自动，非 std 的 Win32 通道不自动**。

三个消费者分别落在哪种机制上：

| 消费者 | 机制 | 结论 |
|---|---|---|
| 应用内 `std::fs`（`read` / 落盘 / 回读 / `canonicalize`） | M1 | 长路径自动可用，无需声明 |
| 应用内裸 Win32（`acl.rs` 的 `converge_ace`） | M3（当前缺） | 只对**可写根本身**调用，根本身是 `%LOCALAPPDATA%\<APP_IDENTIFIER>\tool-scratch\<hash>` 量级，远不到 248，**当前不构成风险**；但 W1 改成多可写根后根数增加，规则须写死：该处入参必须先过 `canonicalize_lenient`，长度 ≥248 时由调用方补前缀 |
| 子进程（`cargo` / `npm` / `python`） | 各自进程自己的 M2/M1/M3 | **我们的清单对它零作用**。它们能否处理长 `TEMP` 取决于其自身实现；这是 K8 的真正边界 |

**前端不需要任何声明**，理由有三条：

| # | 理由 |
|---|---|
| F1 | `longPathAware` 是**进程级**清单项。前端不是进程，只是同一 exe 内 WebView 的内容；WebView2 渲染进程（`msedgewebview2.exe`）是独立进程，其清单不由我们控制，也无法控制 |
| F2 | 前端不调用任何文件 API。路径在前端只是**字符串**（展示、裁剪、审批卡片），实际 IO 全在后端；字符串没有 260 限制 |
| F3 | 唯一相关的两端是「后端 exe 的清单」与「子进程各自的清单」，两端都与前端无关 |

`TEMP` / `TMP` / `TMPDIR` 的值仍**绝不带 verbatim 前缀**：`cmd.exe` 与部分第三方程序不认该形态，会把整串当相对路径或直接失败；env 通道无法用 M3 补，只能靠子进程自身的 M2/M1——这正是 K8 的由来。

### 3.10 诊断口径（错误回显）

范围：**模型可见的文本**（工具错误串与 `assistant_text`）。`data` / `compact` 只进 wire 与 UI（§2 第 1 行），不受本节约束。

| 编号 | 规则 |
|---|---|
| D1 | 模型可见文本中**不得出现域根物理路径**（含用户名与会话哈希）。引用（`tool-output://` / `scratch://`）的路径类错误一律回显调用方原串（`PathRef::raw()`） |
| D2 | 覆盖全链路，不只是工具层的 `failed to stat`：解析层 `canonicalize_lenient` / `reject_system_dirs` 的失败文案同样按原串渲染；`ensure_within_root` 的失败已由 `ref.rs::escape_message` 用原串重写，属既成口径 |
| D3 | 无 scheme 路径**维持现状**：回显解析后的绝对落点。工作区根对模型已知，落点即诊断信息；「改回原串 / 改相对渲染」是 P3.2，不做 |
| D4 | 实现为**单一渲染点**：`PathRef::error_label(&resolved)`（有 scheme → `raw()`，无 scheme → `resolved.display()`）。工具层与解析层都取它，禁止各自 `format!` 时挑字段（与 F3「三处各自拼串」同类，不重开第二个口径） |

为什么只约束引用、不约束全部路径（窄口径）：

| 判据 | 说明 |
|---|---|
| 泄漏源只有内部域 | 域根含用户名与会话哈希，模型既不该知道、也无法据此操作；工作区绝对路径是模型已知信息，回显落点反而是诊断收益 |
| 宽口径成本高、收益近零 | 要动 `file_revision` 的十余个调用点、十个工具的错误文案与大量既有断言，而模型本就知道自己写了什么 |
| 窄口径不新增泄漏面 | 模型唯一能看到的「物理路径」永远是它自己写进去的那个串（`raw()` 原样回显） |
| 目的可测，手段可换 | 验收断言「错误串不含 `<session_hash>`、不含域根前缀」，不绑定具体实现写法 |

D4 之所以写进规格：F3 已经证明「同一个口径散在几处各自拼串」必然分叉，错误文案是同一类问题，必须在它第一次落地时就收敛。

---

## 4 接入矩阵

| 工具 | 路径参数 | 无 scheme | `tool-output://` | `scratch://` | 落地阶段 |
|---|---|---|---|---|---|
| `read` | `path` | 接受（工作区） | 接受（输出域，`readToolOutput`） | 接受（`readScratch`） | P1 / P4 |
| `super_read` | `path` | 接受 | 接受（同上，共用 `intent`） | 接受（同上） | P1 / P4 |
| `write` / `edit` / `super_edit` | `path` | 接受 | **拒绝**（暂不支持 scheme） | **拒绝**（暂不支持 scheme） | P1 |
| `mkdir` | `path` | 接受 | **拒绝**（暂不支持 scheme） | **拒绝**（暂不支持 scheme） | P1 |
| `delete` | `path` | 接受 | **拒绝**（暂不支持 scheme） | **拒绝**（暂不支持 scheme） | P1 |
| `move` | `source` / `target` | 接受 | **拒绝**（暂不支持 scheme） | **拒绝**（暂不支持 scheme） | P1 |
| `fs_link` | `source` / `target` | 接受 | **拒绝**（暂不支持 scheme） | **拒绝**（暂不支持 scheme） | P1 |
| `shell` | `cwd` | 接受 | **拒绝**（暂不支持 scheme） | **拒绝**（暂不支持 scheme） | P1 |
| `list` / `grep` / `glob` | `path` | 接受 | **拒绝**（暂不支持 scheme，域扩展见 §8） | **拒绝**（同） | P1 |
| `request_tool` | 转发 | 按被调工具口径 | 按被调工具口径 | 按被调工具口径 | P1 |

拒绝文案分**四类**，由**方向**与 scheme **类别**（§3.1 G6）决定，不得互相复用：

| 方向 | 工具 | 类别 | 常量 | 文案 | 是否提示替代 |
|---|---|---|---|---|---|
| 写 | `write` / `edit` / `super_edit` / `mkdir` / `delete` / `move` / `fs_link` | `Location` + `ShellTempArea`（`scratch://`） | `WRITE_LOCATION_SHELL_REJECTED` | 「this tool cannot write through a path reference. Files in the shell temp area are created by shell commands: write them there (the command's temp dir is the same directory) and read them back with the reference」 | 提示 `shell`（真实可写通道） |
| 写 | 同上 | `Location` 无通道（暂无实例） | `WRITE_LOCATION_NO_CHANNEL_REJECTED` | 「this tool cannot write through a path reference. This location has no write channel from here」 | **不提示** |
| 写 | 同上 | `Namespace`（`tool-output://`） | `WRITE_NAMESPACE_REJECTED` | 「this tool cannot write through a path reference. Files under this scheme are produced by the run that generated them and cannot be edited afterwards」 | **不提示** |
| 工作目录 | `shell`(`cwd`) | 全部 | `CWD_REJECTED` | 「a working directory cannot be a path reference: pass a plain path inside the workspace」 | 提示「工作区内普通路径」 |
| 浏览 | `list` / `grep` / `glob` | 全部 | `BROWSE_REJECTED` | 「this path scheme is not supported by this tool yet: it takes a plain path only; to open a specific file under that scheme, use read」 | 提示 `read`（真实可读通道） |

写方向的三句由 `write_rejected(scheme)` **按类别派生**（`Scheme::nature()`），不是按 scheme 名硬编码：新增 scheme 只声明类别，文案自动就位。`Namespace` 在类型上不可能携带 `write_alternate`（`SchemeNature` 把它嵌在 `Location` 变体里），所以「给不可写域提示写通道」这种假提示不可表达。

四类文案不能合并，每一条合并都会产出一条**错误信息**：

| 合并方式 | 后果 |
|---|---|
| 写方向对两域共用一句 | `scratch` 的可用替代（子进程写入）与 `tool-output` 的不可写事实冲突；给输出域提示 shell 是**假提示**（该域在沙箱拒写名单内），必然换来一次失败调用 |
| 写方向与浏览方向共用一句 | 模型调 `list scratch://` 会收到「write targets / working directories」——它既没打算写也没打算设工作目录，只能得出「调错工具类别」的结论，下一步往往换 `write` 再撞一次（文案与实际意图错位等于诱导试探） |
| 写方向与工作目录共用一句 | 工作目录本来就要求「工作区内的普通路径」，与「域内写目标」不是同一诉求；混在一起会让模型在 `cwd` 场景下也以为要绕开引用 |
| 文案写「must be plain paths」（不带替代提示） | **最危险的一条**：域引用的物理路径正是模型不知道也不该知道的，把它写成输入要求等于让模型去猜绝对路径——这正是 §2 第 16 行缺陷的起点（用户名为哈希前缀被拼出、随后连撞沙箱）。文案只能陈述「本工具不认引用」并指向真实通道，不能对输入提要求 |

文案口径（V 类约束，改文案前先读；与 §8 的 K 类「已知代价」不同前缀，避免撞号）：

| 编号 | 规则 |
|---|---|
| V1 | 文案说的是**本工具当前不接受 scheme**，不是「该域只读」。只读是当前阶段的工程取舍（P6 / N5），会随能力扩展变化；把取舍写成「read-only」会让模型把它当成永久边界，扩展时又要回头改文案 |
| V2 | 「yet」是有意的：能力扩展后旧串失效，但模型侧的迁移成本只是一次措辞更新；不写「yet」则扩展时文案与事实直接冲突 |
| V3 | 文案**不得**把「域引用的物理路径」写成对输入的要求：`plain path` / `absolute` 这类措辞只允许出现在「工作区路径」语境（即 `CWD_REJECTED`，工作区根对模型已知，说明落点不构成泄漏）；域引用的文案只能陈述「本工具不认引用」+ 指向真实通道，且不得回显任何域根（§2 第 16 行 / V7） |
| V4 | 文案常量按 **(方向, 类别)** 划分（`WRITE_LOCATION_SHELL_REJECTED` / `WRITE_LOCATION_NO_CHANNEL_REJECTED` / `WRITE_NAMESPACE_REJECTED` / `CWD_REJECTED` / `BROWSE_REJECTED`），覆盖该组合全部入口；新增 scheme 按类别继承，不得各写一份。写方向的三句只由 `write_rejected` 派生，禁止在工具里 `match scheme` 选文案 |
| V5 | **方向决定文案，不是工具决定文案**：写方向与叶子保留方向共用写方向文案，浏览方向一律 `BROWSE_REJECTED`。提示替代的前提是「该通道真实可用」，否则就是教试探 |
| V6 | 浏览方向的拒绝是 `invalid`（输入不接受），不是 `blocked`（待批准）：不进入审批流程，也不标记 `blocks_input_rewrite` |
| V7 | 写方向按 scheme **类别**分叉：`Location` 在声明了真实通道（`ShellTempArea`：临时区本就是子进程的写入目标，§3.8 S3，命令的 `TMPDIR` 就是同一目录）时提示它，未声明时只陈述事实；`Namespace` **禁止**提示任何通道（输出域在沙箱拒写名单内，提示即假提示）。通道是否存在的**唯一**声明点是 `Scheme::nature()` 的 `write_alternate`，文案不得自行判断 |

---

## 5 改动面

### 5.1 P1 — 解析层（无行为变化，可独立回滚）

| # | 改动 | 位置 |
|---|---|---|
| P1.1 | 新增 `Scheme`（封闭枚举 `ToolOutput` / `Scratch`）、`PathRef { raw: String, scheme: Option<Scheme>, body: Option<String> }`（私有字段，不变量 `scheme.is_some() == body.is_some()`）、`parse_ref`；以及产出侧**格式串工具**（`pub const TOOL_OUTPUT_SCHEME: &str` + `pub fn tool_output_ref(file_name: &str) -> String`）。P1 只提供格式能力，不改任何输出（P2.1 才调用它） | 新增 `os/shared/path/ref.rs` |
| P1.2 | `parse_path_arg` / `parse_optional_path_arg` 返回 `PathRef` | `path/args.rs` |
| P1.3 | `join_input` 增 scheme 拒绝（E1） | `path/resolve.rs` |
| P1.4 | `PathRef` 上挂入口：`resolve_readable(&self, ctx) -> Result<ReadablePath, String>`（读）、`resolve_write(&self, ctx) -> Result<PathBuf, String>`（写）、`resolve_working_directory(&self, ctx) -> Result<PathBuf, String>`（`shell` 的 `cwd`）、`resolve_leaf_preserving(&self, ctx) -> Result<PathBuf, String>`（叶子保留）。`scheme.is_some()` 时：`resolve_readable` 走域根拼根（P1 实现 `tool-output`；`scratch` 因 `tool_scratch_root` 字段尚未存在，恒返回 §3.4 的「scratch not found」，P4b 改为读该字段拼根，文案不变）；`resolve_write` / `resolve_leaf_preserving` 按域返回 `write_rejected(scheme)`，`resolve_working_directory` 返回 `CWD_REJECTED`（§4 V7）。`scheme.is_none()` 时转调既有 `resolve_readable` / `resolve` / `resolve_leaf_preserving`（这三个函数降为 `pub(crate)`，导出面只留 `PathRef` 方法） | `path/ref.rs` + `resolve.rs` |
| P1.5 | 17 处 `parse_path_arg` / `parse_optional_path_arg` 构造点改为持有 `PathRef`，各工具的 `Request.path` / `source` / `target` 字段随之换型，后续所有 `path::resolve*` 调用改为对应 `PathRef` 方法（编译期强制）。**三个非构造点同批迁移**：`os/shared/scope.rs::base_directory(Option<&PathRef>, ctx)`（`list` / `grep` / `glob` 的基准目录，走 `resolve_browse` 语义 → 天然拒绝 ref）；`agent_shell/schema.rs` 的 `cwd` 先用 `parse_ref` 判 scheme，命中即 `ShellInputError::blocked(CWD_REJECTED)`，未命中再走 `resolve_working_directory`；`fs_link/validate.rs:41` 的 `resolve_leaf_preserving` 改为接收 `&PathRef` | 12 个工具模块 + `os/shared/scope.rs` + `agent_shell/schema.rs` |
| P1.6 | `ReadablePath::intent` 收敛审批映射，`read` / `super_read` 各删一份 | `path/resolve.rs`、`agent_read/mod.rs`、`super_read/mod.rs` |
| P1.7 | 导出面更新 | `os/shared/path/mod.rs` |
| P1.8 | 域根归一：`tool_output_root` / `tool_scratch_root` 在构造点各过一次 `canonicalize_lenient`，之后的 `join` / `ensure_within_root` / 回填只用该结果（§3.9 / N17）。当前 `AppDirs` 来源非 verbatim，但不得依赖该事实 | `adapters/tool_exec.rs`、`runtime/pipeline.rs`、`agent_shell/spawn.rs` |

### 5.2 P2 — 产出层（模型与 UI 看到 ref）

| # | 改动 | 位置 |
|---|---|---|
| P2.1 | `PersistedRef::model_ref()` | `runtime/storage.rs` |
| P2.2 | 标签渲染改用 `model_ref` | `agent_shell/run.rs::assemble_text`、`runtime/pipeline.rs::persist` |
| P2.3 | `compact.persisted.path` 改用 `model_ref` | `runtime/compact/envelope.rs::build_compact` |
| P2.4 | `shell` 描述改为「回填的是输出引用，可直接作为 `read` 的 `path`」 | `agent_shell/mod.rs::description` |
| P2.5 | `read` 描述补一句引用写法 | `agent_read/mod.rs::description` |
| P2.6 | 无契约变更：`PersistedResultDto` 形状不变，仅值变化 → **不需要** `contract:gen` | — |

> 原 P2.7（`shell` 描述写临时区语义）已移至 P4.10：描述里出现 `scratch://` 之前必须先有该域，提前写会让模型照抄一个尚不能解析的引用。

### 5.3 P3 — 诊断回显（P3.1 必做；P3.2 不做）

原 P3.1 的判据是「需证明回显原串不会让诊断变差」。§2 第 16 行的实测给出反例：回显物理路径没有带来任何诊断收益（模型本不该知道域根），反而把域根（含用户名与会话哈希）送进模型可见文本，诱发模型拼出系统 temp 并在后续三轮持续硬编码、连续被沙箱拒绝。故改判为必做，口径见 §3.10。

| # | 改动 | 位置 |
|---|---|---|
| P3.1a | 工具层：`read` / `super_read` 的 `failed to stat` / `is a directory` / `is not a regular file` 三处改取 `PathRef::error_label(&resolved)`；`file_revision` 增带 label 的变体，仅 `read` / `super_read` 传同一 label（其余调用点用 1 参版本，行为不变） | `agent_read/mod.rs`、`super_read/mod.rs` |
| P3.1b | 解析层：`canonicalize_lenient` / `reject_system_dirs` 增带 label 的变体，`ref.rs` 的 scheme 分支改传 `self.raw()`；1 参版本保留给不涉及域根的调用点（`acl.rs`、`approval_key.rs`、`list`、`delete`、`agent_shell/schema.rs`），行为不变 | `path/resolve.rs`、`path/protected.rs`、`path/ref.rs` |
| P3.1c | `PathRef::error_label(&resolved)`：单一渲染点（§3.10 D4） | `path/ref.rs` |
| P3.1d | 验收：A24 / A25 | 见 §7 |
| P3.2 | **不做**：工作区路径改「相对工作区」渲染，或改回显调用方原串 | 需证明相对路径在跨轮、跨 `cwd` 场景仍可解析；且回显绝对落点对模型是诊断信息（§3.10 D3） |

P3.1 只依赖 P1 的 `PathRef::raw()`，与 P2 / P4 互不依赖，可独立回滚（回滚即恢复「回显解析后路径」）。它波及的既有断言只有 `read` / `super_read` 两处工具的错误串，不触及任何工具的结果数据（`data.path` 保持解析后绝对，§3.10 范围）。

### 5.4 P4 — scratch 域（前置：多可写根）

| # | 改动 | 位置 |
|---|---|---|
| P4.0 | 两段注入，形态与 `index_root` 完全一致：① 装配期 `PipelineToolExecution` 增 `tool_scratch: Option<PathBuf>`（`tool_setup.rs` 填 `dirs.tool_scratch()`，库 crate 不解析基根）；② 每次 `execute` 用 `scratch_root_for(&tool_scratch, Some(&session_id))` 派生会话级域根，填 `ToolUseContext.tool_scratch_root: Option<PathBuf>`（`#[serde(skip)]`，`for_toolbox` 填 `None`）。`scratch_root_for` 是 `runtime/storage.rs` 里的纯函数（`base.join(session_namespace(session_id))`），是会话段拼法的**唯一实现点**，仅允许注入与清理两个应用层调用点（N1）。该字段语义是**已含 `<session_hash>` 的域根**，与 `tool_output_root` 同形，故 crate 内不得再 join 会话段。`ShellInput` 增 `scratch_root: PathBuf`，**直接取该字段**，缺失即 `blocked` + `sandbox_root_missing`（与 `writable_root` 同口径，fail-closed；ToolBox 无会话本就已被该码拦截，无行为变化） | `runtime/storage.rs`、`contract/context.rs`、`adapters/tool_exec.rs`、`tool_setup.rs`、`agent_shell/schema.rs` |
| P4.1 | `dir_name::TOOL_SCRATCH` + `AppDirs::tool_scratch()` + 登记表 `E19`（W5）；穷尽分支**四处** + 采样一处，取值见 §3.8「登记表字段」 | `app_dirs/`、`cmd/storage/entries/decl.rs`、`cmd/storage/entries/eligibility.rs`、`cmd/storage/reclaim/execute.rs`、`cmd/storage/sample/` |
| P4.2 | `SandboxPlan.writable_root` → `write_roots`；`cwd` 归属改「任一根内」（W1） | `sandbox/mod.rs`、三后端 |
| P4.3 | 逐根敏感重叠检查 + 逐根 `converge_ace`（W2） | `macos.rs`、`linux.rs`、`windows/acl.rs` |
| P4.4 | `sensitive_read_deny` 整树 → 逐项枚举（W3，**须评审**） | `tool_setup.rs::sandbox_host_paths` |
| P4.5 | `forced_env` 三条指向 `tool_scratch/<session_hash>`（W4） | `agent_shell/spawn.rs` |
| P4.6 | `prepare_scratch` 改幂等 `create_dir_all`；`RunGuard::drop` 只 kill 不清理；`cleanup_scratch` 的三个调用点删除后**删掉该函数**；会话结束用 `scratch_root_for(dirs.tool_scratch(), Some(id))` 取根后 `remove_dir_all`（`NotFound` 静默）（W6） | `agent_shell/spawn.rs`、`run.rs::RunGuard`、`cmd/conversation/mutate.rs`（删除 / 批量删除 / 清空三入口） |
| P4.7 | 恒回填 `scratch://` 根 ref + 一层子项清单（R5） | `agent_shell/run.rs::assemble_text` |
| P4.8 | `parse_ref` 支持 `scratch` 的嵌套 / 可空 body + `Scratch` scope + `readScratch` intent（P5–P9）；`preview_for_approval` 的「非输出域即 `None`」分支须一并纳入 `Scratch`，否则临时区读取无预览卡片 | `path/ref.rs`、`path/resolve.rs`、`agent_read/mod.rs`、`super_read/mod.rs` |
| P4.9 | 前端 i18n 与审批动作映射补 `readScratch`；`toolConfig` 快捷规则项（`effects` 只给 `ask` / `deny`，与 `readToolOutput` 同底线） | `src/i18n/locales/*.json`、`src/chat/tool/toolCallDisplay.ts`、`src/chat/core/toolConfig.ts` |
| P4.10 | `shell` 描述补临时区语义（T3 / S8）；须与 P4.7 同批，否则描述指向尚不能解析的引用 | `agent_shell/mod.rs::description` |

---

## 6 不变式

| 编号 | 不变式 |
|---|---|
| N1 | 域根（**已含会话段**）只有一个来源：输出域为 `ToolUseContext.tool_output_root`（由 `namespace_root` 派生），scratch 域为 `ToolUseContext.tool_scratch_root`。两者的会话段拼法只有一个实现点（`runtime/storage.rs::scratch_root_for(base, session_id)`，纯函数、无 IO），应用层只允许**两个调用点**：注入（`adapters/tool_exec.rs`）与清理（`cmd/conversation/mutate.rs`）。除此之外任何代码不得自行 `root.join(session_namespace(..))`；库 crate 内不得解析 `AppDirs` 基根 |
| N2 | scheme 串永不被当作相对路径拼接：`join_input` 见到即拒绝（E1） |
| N3 | 同一文件的两种写法命中同一审批键（action + resource 同源） |
| N4 | 绝对路径写法永久保留；但产出侧不再为输出域生成绝对路径 |
| N5 | 写方向不认任何 scheme；输出域与 scratch 域对模型均只读。拒绝文案只写「本工具当前不接受 scheme」，不得写成「只读」（§4 V1） |
| N6 | 不回退：缺根、缺文件、跨会话一律明确报错，不换域重试 |
| N7 | 输出域的落盘文件必须直接位于命名空间目录下（`model_ref` 由文件名派生的前提）。**该平铺约束只适用于输出域**，scratch 域允许嵌套（§3.8「语法」） |
| N8 | 模型可见串与 UI 可见串可不同（`model_ref` vs 绝对），但**同一处**不得混用两种口径 |
| N9 | 新增 scheme 必须同时改：解析、拒绝文案、接入矩阵（§4）、验收（§7） |
| N10 | ref 的 body 恒等于落盘文件名；映射恒为 `tool_output_root.join(body)`（§3.7）。M1–M5 任一失效，必须先改 §3.7 再改代码 |
| N11 | `tool_output_root` 只能由持有 `storage` 的那一层派生，且必须与同次调用落盘所用的 `session_id` 同源（§3.7.1）。适配器不得自行拼接域根 |
| N12 | 域内对模型一律只读：写方向拒绝，且拒绝判定在 `join_input` 之前 |
| N13 | scratch 的**位置**只由沙箱授权集合与 `AppDirs` 给出、**子进程通道**只由 `spawn` 注入的环境变量给出；两者都不得由模型输入或用户设置改写（§3.8 T1） |
| N14 | scratch 目录在调用结束后**必须存在**（会话级保留），否则 `scratch://` 引用无意义；清理只发生在会话结束（含删除 / 批量删除 / 清空会话），存储管理侧只观测、不提供独立回收动作（§3.8「登记表字段」/ S2） |
| N15 | `scratch://` 的 ref 只由 shell 回填产生，且**不含会话段**：会话由上下文提供，模型无从构造其它会话的路径；运行时只解析本会话域根（§3.8 T2 / §3.4 失败表） |
| N16 | scratch 域内不设调用级子目录：`TEMP` / `TMP` / `TMPDIR` 与 `scratch://` 必须指向**同一个**会话目录，否则 handoff 失效（§3.8 D1） |
| N17 | 域根必须先归一为绑定形态再参与任何 `join` / `ensure_within_root` / 回填；`\\?\` 在 scheme body 里一律**拒绝**而非剥离（§3.9）。剥离只发生在 `canonicalize` 内部，不得提前 |
| N18 | 模型可见文本中不得出现域根物理路径：引用（`tool-output://` / `scratch://`）的路径类错误一律回显原串，解析层与工具层同一口径（§3.10） |

---

## 7 验收

| 编号 | 判据 |
|---|---|
| A1 | `read` 以 `tool-output://<name>` 读取成功，返回内容与绝对路径读逐字节一致 |
| A2 | `super_read` 同上，且与 `read` 的 `permission_intents` 完全一致（`action` 与 `resource` 相同） |
| A3 | 两种写法命中同一授权键：同一会话内先绝对路径授权，再用 ref 读，**不**二次弹框 |
| A4 | 越界拒绝：`tool-output://../x`、`tool-output://a/b`、`tool-output://C:\x`、`tool-output://`、`tool-output:// x` 全部 `invalid`，且文案指明是引用格式问题 |
| A5 | 写方向拒绝：`write` / `edit` / `mkdir` / `delete` / `move` / `fs_link` / `shell`(`cwd`) 收到 ref 时返回业务错误，且**未**创建/删除任何文件 |
| A6 | 最低层拒绝：任意工具绕过 `PathRef` 直接传 `tool-output://x` 时，`join_input` 返回错误而非拼成工作区路径（E1） |
| A7 | `tool_output_root` 为 `None` 时，ref 读取报「no tool output root」，不回退工作区 |
| A8 | 产出侧一致性：`shell` 的标签路径 == `compact.persisted.path` == `PersistedRef::model_ref()`，三处逐字节相同 |
| A9 | 兼容性：历史行（绝对串标签）仍可 `read`、仍可渲染；`data.outputPath` 仍为绝对路径且 15 处既有 shell 用例不回归 |
| A10 | 前端不回归：`toolCallDisplay` 的裁剪用例在 ref 值下通过（新增一条 `tool-output://…` 用例） |
| A11 | 往返恒等：对任一落盘引用，`resolve_readable(model_ref(reference.path))` 得到的绝对路径与 `reference.path` 逐字节相同 |
| A12 | 唯一性：同一命名空间内连续落盘 N 次（含同一 `tool_call_id` 重试）产生 N 个互不相同的文件名（M2） |
| A13 | 权威源同源：一次真实 `execute` 中，工具收到的 `context.tool_output_root` == `storage.namespace_root(context.session_id)`，且 `persist` 用的 `session_id` 与派生所用的是同一字段（§3.7.1 / N11） |
| A14 | 通道唯一：一次 `shell` 调用中，子进程的 `TEMP` / `TMP` / `TMPDIR` 三者同值且等于本会话 `tool_scratch/<session_hash>` 目录；父进程同名变量被覆盖而非透传（§3.8 T1 / N13 / N16） |
| A15 | scratch 会话级保留：`shell` 结束后会话临时目录仍存在，`read` 以回填的 `scratch://<rel>` 读取成功；连续两次 `shell` 的 `TMPDIR` 相同（N14 / N16 / R5） |
| A16 | scratch 只读：`write` / `edit` / `mkdir` / `delete` / `move` / `fs_link` / `shell`(`cwd`) 收到 `scratch://` 时返回业务错误且未改动任何文件（N5 / N12） |
| A17 | scratch 越界拒绝：`scratch://../x`、`scratch://a/../../x`、`scratch://C:\x` 全部 `invalid`；`scratch://` 与 `scratch:///` 解析为会话临时区根，读取目录本身报「is a directory」而非越界（N15） |
| A18 | scratch 回填恒在：命令未产生任何临时文件时，结果中仍有 `scratch://` 行且不带清单（R5） |
| A19 | scratch 回填清单：清单与目录一层实际子项一致，超过 20 项时以 `…(+N more)` 截断且 N 正确；清单不含大小 / 时间 / 递归项 |
| A20 | 幂等创建：同会话连续 N 次 `shell` 不删除、不重建临时目录；先写入的文件在第 2 次调用中仍存在（§3.8 W6 / D4） |
| A21 | 形态拒绝：`tool-output://\\?\C:\x`、`tool-output:////?/C:/x`、`scratch://\\?\C:\x`、`scratch:////?/C:/x` 全部 `invalid`（**不**被剥离成合法路径）；合法的无 scheme verbatim 串仍照旧解析成功（§3.9 / N17） |
| A22 | 域根形态一致：把 `tool_output_root` 与 `tool_scratch_root` 分别注入为 verbatim 形态后，`read` 仍能读回各自域内的文件（两侧均归一为绑定形态，`ensure_within_root` 不误判域外）（§3.9 / N17） |
| A23 | 长路径不回归：scratch 内构造 ≥260 字符的深层文件，`read` 读取成功（M1 自动前缀生效）；`converge_ace` 对可写根仍成功（根短，M3 无需前缀）（§3.9「长路径」/ W7） |
| A24 | 引用错误回显原串：`read` / `super_read` 以不存在的 `scratch://<rel>` 与 `tool-output://<name>` 读取时，错误串只含该引用原串，不含 `<session_hash>` 与域根前缀；`scratch://` 读目录本身的「is a directory」同理（§3.10 D1 / D2 / N18） |
| A25 | 解析层同口径：引用命中受保护 / 凭据目录（如 `scratch://.ssh/x`）或域根缺失导致归一失败时，错误串只含原串，不含域根前缀（§3.10 D2 / N18） |
| A26 | 浏览方向文案分叉：`list` / `grep` / `glob` 收到 `scratch://<rel>` 与 `tool-output://<name>` 时，`execute` 与 `validate_input` 的文案都是 `BROWSE_REJECTED`（含「use read」），且**不含**「write targets」字样；拒绝为 `invalid` 而非 `blocked`，域内文件原样存在（§4 V5 / V6） |
| A27 | 写方向文案由类别派生且不诱导拼路径：对 `Scheme::ALL` 逐项断言——文案都**不含**「plain path / absolute」；`Namespace` 一律**不含** `shell`（假提示）；`Location` 的 `shell` 出现与否**严格等于**其 `write_alternate` 是否为 `Some`；两类文案互不相同；`shell` 的 `cwd` 拒绝文案是 `CWD_REJECTED` 且仍 `blocks_input_rewrite`（§3.1 G6 / §4 V7） |

---

## 8 明确不做

| 项 | 理由 |
|---|---|
| vault 仓库相对路径纳入本体系 | 该路径只在特性内部使用，不接收模型产出的串；纳入要引入「仓库根 ↔ 绝对路径」翻译层 |
| MCP 工具入参统一 | 入参 schema 由第三方服务器定义，本应用无权改写；且其资源标识是 `mcp:<server>:<hash>`，不是文件路径 |
| 前端工作区绑定值纳入 | 用户配置值，语义是「绑定的目录」，与模型引用无关 |
| `grep` / `glob` / `list` 接受 `tool-output://` | 需要给这三个工具新增输出域审批（当前只有 `readToolOutput`），属独立议题。现状是明确拒绝（`BROWSE_REJECTED`，§4），不是静默错读 |
| `index://` 启用 | 索引目录由 `super_read` 内部使用，不经工具入参；保留 scheme 名仅为将来扩展不留冲突 |
| 为 shell 临时目录新增用户设置项 | 位置由沙箱可写根决定，不是可自由选择的偏好；暴露开关会诱导用户把临时目录指到无沙箱覆盖的路径（§3.8 T1） |
| 把 scratch 根放到系统 temp | 系统 temp 归 OS 所有：Windows 上在用户主目录内（`acl.rs::prepare` 禁改区）、Linux 上世界可写（隐私泄漏面）、外部清理器可能在运行中删目录（§3.8「为什么不用系统 temp」） |
| 新增 `ASSISTANT_SCRATCH` 等专用环境变量 | 会诱导模型把会话临时区当跨会话持久区，产生静默失败；`ASSISTANT_` 在 env 侧本就被 `is_sensitive_name` 当作不外传族（§3.8 T1） |
| 在 scratch 域内再分 `<call_id>` 层 | 破坏 `TEMP` / `TMP` / `TMPDIR` 与 `scratch://` 指向同一目录的前提，handoff 失效；且 ref 长度回到 F1 量级（§3.8 D1–D6） |
| 临时区按调用清空 | 同上：会删除同会话其它调用正在使用的文件，并破坏构建缓存（§3.8 D3 / D4） |
| `read` 回读结果的二次落盘（回读超过 `inline_output_limit` 时产生「输出文件之输出文件」） | 与本体系无关的独立缺陷，见 `runtime/pipeline.rs::persist` 的跳过条件；另案处理 |
| 引入 `url` / `iri-string` 等解析库 | 只需 `split_once("://")` + 精确匹配；引入库会带来对 Windows 路径的额外判定面 |
| 在错误文本中附加「解析后绝对落点」作为诊断信息（如 `ref (resolved: <abs>)`） | 落点就是域根物理路径，等于把 §3.10 D1 原样放回去；无 scheme 串已回显落点（D3），引用串的落点恒为「本会话域根 + body」，对模型无可操作信息（N15） |
| 在 body 校验前剥离 `\\?\` 前缀 | 剥离会让 verbatim 形态的绝对路径通过单段校验，域边界失效；剥离属 `canonicalize` 内部职责，不在解析入口做（§3.9 / N17） |
| 为 exe 清单声明 `longPathAware`，或让前端「声明」长路径能力 | 两者都不成立：`longPathAware` 是**进程级**清单项，前端不是进程、不调用文件 API，路径在前端只是字符串；exe 侧的 M2 只影响非 std 的 Win32 调用，本应用该通道（`acl.rs`）只碰可写根本身，远不到 248。且 M2 还需用户机器 `LongPathsEnabled=1`，不可控；真正需要时改用 M3（调用点补前缀，见 W7）（§3.9「长路径」F1–F3） |

---

## 9 风险

| 编号 | 风险 | 处置 |
|---|---|---|
| K1 | Unix 下 `tool-output:` 目录名被保留 | 接受；文案明确「该前缀被保留」。Windows 无此问题 |
| K2 | 模型混用两种写法（ref 与绝对） | 允许。N3 保证两种写法同键、同域、同结果；产出侧只给 ref，模型没有理由再抄绝对串 |
| K3 | `parse_path_arg` 返回类型变更导致大面积编译错误 | 预期且有益：这正是 E3 的强制力来源。P1 一次性完成，期间不发布中间态 |
| K4 | UI 展示从绝对路径变为 ref，用户难以定位文件 | `data.outputPath` 仍为绝对（R5），需要定位时从数据面板取；卡片展示 ref 反而消除本机路径泄露 |
| K5 | 历史行与新行的 `compact.persisted.path` 值不同（绝对 vs ref） | 前端按原值裁剪，两种都能工作（R7 + A10）；不做回填改写 |
| K6 | 会话临时区无上限增长 | 条目在存储管理里显式可见（`risk: Low`），但**不提供独立回收动作**（`NotImplemented`，与 `E13` 同档）：清理只发生在会话结束与删除会话时。不做运行时自动淘汰，避免删掉正在使用的大产物；超长会话需用户主动结束会话以释放空间——接受 |
| K7 | 同名覆盖导致静默陈旧 | 共享 temp 的固有语义，也是 handoff 的前提；T3 要求在 `shell` 描述中明说，使模型能预判 |
| K8 | scratch 下深目录（`cargo target` / `node_modules`）触发 Windows 260 字符上限 | 分三种机制看（§3.9「长路径」）：应用内 `std::fs` 走 M1 自动前缀，**已覆盖**；本进程裸 Win32（`acl.rs`）只碰可写根本身，根本身远不到 248，**当前不构成风险**；唯一不可控的是**子进程**——`TEMP` 不能带前缀，其长路径能力由各子进程自身清单决定，我们的清单对它零作用。故该项为**已知限制**，不是可修缺陷；若实测有子进程失败，处置是缩短 scratch 根（降 `session_hash` 长度）而非加清单 |
| K9 | 回显原串后，错误文本只告诉模型「它写了什么」 | 引用串的落点恒为「本会话域根 + body」，对模型无可操作信息（N15）；排障需要落点时取 `data.path`（R4 / R6），不受 §3.10 约束 |

---

## 10 实施顺序

```
P1 解析层（E1/E2/E3 + 审批收敛）   └── 可独立回滚（行为不变）
   ├── P2 产出层（model_ref 三处 + 描述）          └── 可独立回滚（只改渲染串）
   ├── P3 诊断回显（error_label + 解析层同口径）    └── 可独立回滚（回滚即恢复解析后路径）
   └── P4a 前置（W1–W7 + 登记/采样 E19）→ P4b 会话级单目录 → 回填 ref + readScratch
                                          └── 可独立回滚（关回填即可退回双写法现状）
```

P1 完成后，ref 已能被全应用正确解析与正确拒绝，但产出仍是绝对路径——此时系统是「双写法可用」。P2 完成后，模型只看到 ref。P4 依赖 P1 的 `parse_ref` 扩展点，但**不依赖** P2：scratch 的 ref 由 shell 独立回填，与输出域 `model_ref` 是两条产出线。

P3 只依赖 P1 的 `PathRef::raw()`，与 P2 / P4 无顺序约束：它改的是错误文案的取值来源，不改产出串、不改解析结果。但它**必须**在 P4b 之前或与之同批完成——scratch 域一上线，引用类错误就会开始出现，此时若仍在回显域根，就是 §2 第 16 行的缺陷重新生效。

P4 内部的强制顺序：W1–W7 与 `E19` 登记（含采样，P4.0–P4.4）必须先于「会话级保留」，否则子进程写入落到旧位置、直接被沙箱拒，或每次调用删掉上次的产物；「会话级保留」（P4.5–P4.6）必须先于「回填 ref」（P4.7 / P4.10），否则 ref 指向已删目录、描述指向不能解析的引用；`readScratch` 审批（P4.8–P4.9）必须与 `parse_ref` 的 scratch 分支同批上线，否则会出现「能解析但不能授权」的中间态。

P4a 与 P4b 的分界即「能否独立回滚」：P4a 只改沙箱授权集合、临时区位置与登记表，行为对模型不可见；P4b 才让模型看见 `scratch://`。逐项任务与验收勾选见 `I73-path-reference-tasklist.md`。