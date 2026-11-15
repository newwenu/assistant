# I50 — 运行时审计 任务清单

> 规格：[I50-runtime-audit-spec.md](./I50-runtime-audit-spec.md)
> 事实基线：[audit-coverage-audit-1.md](./audit-coverage-audit-1.md)（后端缺口 G1–G10 / 观察项 O1–O7；`G11`–`G14` 为本规格编写时新增）、[audit-coverage-audit-2.md](./audit-coverage-audit-2.md)（前端缺口 F1–F8 / 观察项 OF1–OF5）
> 前置：无。主体为「补生产调用点 + 收敛口径 + 加约束测试」，契约变更有三处：`AuditLogEntry` 追加 `entryId`、新增 `PermissionAuditEntryDto`、`vault_lock` 新增 `LockReasonDto`（前两处落主契约、第三处落 vault 契约，均需 `npm run contract:gen`）。

十四个批次对应规格章节：V1 vault 事件补点（§2.3）→ V2 补录正确性（§2.5）→ V3 查询面与 detail 收敛（§2.4 / §2.6）→ V4 实现整齐性收敛（§2.7）→ T1 outcome 词表（§3.1）→ T2 决策点覆盖（§3.2 / §3.3）→ T3 intent 声明（§3.4）→ T4 ToolBox 直调（§3.5）→ T5 保留与索引（§5）→ P1 锁定归因（§2.3 F5）→ P2 授权管理审计（§3.6 F3）→ P3 vault 审计面板（§2.6 F1）→ P4 权限审计查询命令（§3.7 F2）→ P5 词表映射与前端面板（§6 F7 / F8）。

依赖：V2 依赖 V1（新增 J 事件必须先落地才能进白名单断言）；**V2.3 依赖 V4.1**（自回响防护是改去重键的前置，顺序颠倒会先引入假补录行）；T2 / T4 依赖 T1.1（`bypassed` / `validateRejected` / `intentError` 必须先存在）；T3 与 T1 独立，可并行；P2 独立（仅写不读）；P3 依赖 V3.1 / V3.2 与 P5.1；P4 依赖 T1.1 与 T5.2；P5.3 依赖 P4；V4.5 依赖 V1.5 / V4.2 / P5.1。

> 本清单不管理提交。提交粒度按代码依赖与可编译性现场决定；这里只回答「代码改没改、验收跑没跑」。
> 全部任务初始未实施；勾选表示代码已改且对应验收已跑。

---

## 阶段 V1 — vault 事件补点（§2.3）

### V1.1 `vault_unlock_failed` 落地（G1）

对应规格 §2.3；验收走「手工验收」第 1 条（连续输错口令），白名单侧由 V1.5 承接 §8 测试第 1 条。

- [x] `src-tauri/src/cmd/vault/lifecycle.rs` `vault_unlock` 命令的 `Err(VaultError::InvalidPassword)` 分支：保留 `log::warn!`，追加 `let _ = state.audit_jsonl(AuditEvent::VaultUnlockFailed, Some("wrong_password"));`
- [x] 同文件原生解锁对话框回调（`UnlockResult::Confirmed` 内 `Err(InvalidPassword)`）同样补点
- [x] 同文件 `vault_rebuild_local_db` 的 `Err(InvalidPassword)` 分支同样补点（该点库正在重建，必须走 JSONL）
- [x] 三处一律 J 通道：禁止改调 `log()`（`log()` 是 `log_dual(Some(conn), None, ..)`，此处无可用连接，会把事件丢在 DB 侧并掩盖失败）
- [x] 写法照抄同函数内已有的 `VaultUnsupportedVersion` 分支（`let _ = state.audit_jsonl(..)`）：三处 `state` 均已在作用域内，不需要额外取 state
- [x] detail 只写固定串 `wrong_password`；禁止拼接错误文本（错误文本可能夹带路径或口令片段）
- [x] 审计失败不得改变返回值：`let _ =` 吞掉，业务仍返回 `VaultError::InvalidPassword`
- [x] `vault_unsupported_version` 的现有调用点保持不动（G4 缺的是白名单条目，不是调用点）

**依赖**：无

### V1.2 删除死事件 `local_db_repaired`（G2）

对应规格 §2.3 / §9「新增事件提交清单」反向约束（无生产调用点不得留在词表）。

- [x] `assistant-vault/src/audit/mod.rs`：删除 `AuditEvent::LocalDbRepaired` 变体与 `Display` 中对应分支
- [x] 全仓确认无调用点后再删（`local_db_rebuilt` 是另一个事件，必须保留）
- [x] 编译通过即可证明无残留引用，不需要额外测试

**依赖**：无

### V1.3 同步覆盖路径补 `vault_entry_updated`（G3）

对应规格 §2.3 / §2.4 条目文法。

- [x] `assistant-vault/src/security_db/entries/import.rs::upsert_imported_in` 的 `Some((None, ..))` 更新分支：在 `UPDATE` 成功、返回 `ImportOutcome::Updated` 之前追加 `let _ = audit_log(conn, AuditEvent::VaultEntryUpdated, Some(&entry.id));`
- [x] 位置必须与 `Inserted` 分支的 `VaultEntryCreated` 对称：都在写库成功之后、`Ok(...)` 之前
- [x] `SkippedStale` / `SkippedLocallyDeleted` 分支**不**写审计（未发生写入）
- [x] `create_remote_copy_tx` 已有的 `VaultEntryCreated` 保持不动

**依赖**：无

### V1.4 锁库批量移除 SSH 密钥补点（G5）

对应规格 §2.3 `ssh_key_removed` 行。

- [x] `src-tauri/src/cmd/vault/mod.rs::ssh_remove_all_on_lock`：每个 `bridge.remove_key(fp)` 成功后追加 `let _ = state.audit_jsonl(AuditEvent::SshKeyRemoved, Some(fp));`
- [x] 必须 J 通道：该函数在 `lock_session` 之后调用，DB 连接已关闭，走 DB 会静默失败
- [x] `remove_key` 失败仍只 `log::warn!`，**不**写审计（没有发生移除）
- [x] `fingerprints.is_empty()` 的提前返回保持不动（无移除即无事件）
- [x] 函数签名需要能拿到 `SecurityDbState`：从 `app.state::<SecurityDbState>()` 取，不要为此改调用方

**依赖**：无（补点本身独立；`ssh_key_removed` 进白名单由 V1.5 负责，两者必须同版本上线，否则锁库写入的 JSONL 行不会补录）。**另需与 V2.3 同批**：本项一次锁库会在同一毫秒写 N 行同事件，按现状 `(event_type, timestamp)` 去重会合并成 1 行（见 §2.5 第 3 条）。

### V1.5 白名单与词表自洽（G4）

对应规格 §2.5 第 2 条 / 验收 §8「测试（vault）」第 1 条。

- [x] `assistant-vault/src/security_db/mod.rs` `BACKFILL_EVENTS` 追加 `vault_unlock_failed`、`vault_unsupported_version`、`biometric_failed`、`ssh_key_removed`
- [x] 保留现有 4 项（`recovery_export_used`、`dek_rotated`、`local_db_rebuilt`、`vault_repository_reset`）——双写事件留在白名单里无害；但其中 `dek_rotated` / `local_db_rebuilt` 是「既双写又在白名单内」，会构成自回响，**必须由 V4.1 显式挡重复**，不得只依赖现状的 `(event_type, timestamp)` 去重（该键在 V2.3 后会失效）
- [x] 新增一致性测试：维护 `J_CHANNEL_EVENTS: &[&str]` 常量，断言其每个元素都在 `BACKFILL_EVENTS` 中；新增 J 事件漏改白名单必须让测试失败
- [x] 测试放在 `assistant-vault/tests/audit.rs`，与现有 JSONL 用例同文件

**依赖**：V1.1（`vault_unlock_failed` 进 J 集合）、V1.4

---

## 阶段 V2 — 补录正确性（§2.5）

### V2.1 游标推进到已处理最大 `at`（G11 / R2）

对应规格 §2.5 第 1 条 / 验收 §8「测试（vault）」第 3 条。

- [x] `try_backfill_jsonl_audit` 收集 `records` 后先取 `records.iter().map(|r| r.at).max()`，补录循环结束后把游标写成该值（无记录时保持原游标不动）
- [x] 删除现状的 `meta::set_meta(conn, JSONL_SYNC_KEY, &now.to_string())`
- [x] 游标只在**补录全部成功**后推进；任一 `INSERT` 失败则整体返回 `Err`，游标不动，下次重试
- [x] **同毫秒不漏补（R2）**：`read_since` 的过滤由严格 `>` 改为 `>=`，靠 V2.3 的新去重键挡重复；**禁止**只改游标不改过滤（单独改会让与游标同毫秒的其余记录永久漏补）
- [x] 新增测试：把游标设为未来值 → 写入一条更早 `at` 的 JSONL 记录 → 解锁补录 → 断言该记录被补入（现状会漏）
- [x] 新增测试：连续两次解锁不产生重复行（依赖 V2.3 的新去重键，不得再以 `(event_type, timestamp)` 为断言前提）

**依赖**：V1.5、V2.3（去重键）

### V2.2 读取容错：坏行与不可读文件

对应规格 §2.5 第 5 / 6 条。

- [x] 坏行：`read_since` 遇解析失败只 `log::warn!` 跳过，不整体返回 `Err`（现状已满足，仅补测试）
- [x] **不可读文件（R4）**：`read_since` 现状 `File::open(&path)?` 会让整批补录失败并丢弃已读记录；改为逐文件 `log::warn!` 后 `continue`，其余文件照常读取
- [x] 新增测试：混合「好行 + 坏行 + 好行」的 JSONL，断言返回 2 条且无 `Err`
- [x] 新增测试（可用临时文件权限/占位模拟）：目录中混入一个不可读的 `audit-*.jsonl`，断言其余文件记录仍被返回且结果为 `Ok`

**依赖**：无

### V2.3 去重键区分同毫秒多条记录（R3）

对应规格 §2.5 第 3 条。

- [x] 选定实现路径并写进代码注释说明取舍：**(a)** JSONL 行加单调 `seq`；**(b)** 用 `(文件名, 行号)` 作记录身份
- [x] 去重条件与 PK 同步改造：`(event_type, timestamp)` → 新键；`id` 由 `jsonl-{at}-{event}` 改为能区分同毫秒的值
- [x] 兼容历史行：已落盘的无 `seq` 记录必须仍可补录（`seq` 缺省时的兜底键不得与旧键冲突）
- [x] 若选 (b)，同一变更内明确「禁止重写/压缩 audit 文件」或补上重写后的键失效处理
- [x] 新增测试：同一毫秒写 2 条 `ssh_key_removed`（模拟锁库批量移除）→ 补录后 `security_audit_log` 必须有 2 行（现状会合并成 1 行）
- [x] 新增测试：同一毫秒写 2 条 `biometric_failed` → 补录后 2 行

**依赖**：V2.1（游标改用 `>=` 后必须靠本项挡重复）、V4.1（自回响防护必须先落地，否则本项会让 `dek_rotated` / `local_db_rebuilt` 每次多一条）

### V2.4 补录包事务、重建后写游标、按日裁剪（R6）

对应规格 §2.5 第 7 条。

- [x] `try_backfill_jsonl_audit` 的补录循环包一层事务（现状逐条 `conn.execute` 隐式提交，N 条 = N 次 fsync），提交后再写游标；事务内失败则整体 `Err`、游标不动
- [x] `assistant-vault/src/security_db/rekey.rs::rebuild_local_db` 建库成功后写入 `JSONL_SYNC_KEY = now`：现状只写 `vault_id`，导致**重建后首次解锁必然全量补录**
- [x] `read_since` 按文件名日期裁剪候选文件（只读游标所在日及之后的文件）；文件名用本地日期、记录用 UTC 毫秒，**必须留 1 天余量**，禁止无条件遍历全部日文件
- [x] 新增测试：游标为 D 日，目录内有 D-3 / D-1 / D / D+1 四个文件，断言只读 D-1 起（留余量）且结果正确
- [x] 新增测试：`rebuild_local_db` 后立即解锁，断言补录不重扫全部历史文件（可用「裁剪后未读取的文件数」断言）

**依赖**：V2.1

### V2.5 补录行来源标注（R1）

对应规格 §2.5 第 8 条 / §6「补录行不得与原生行同质化」。

- [x] 保留 `id` 前缀 `jsonl-`（新去重键落地后仍必须可辨识来源），**禁止**改成与原生行同形
- [x] 前端面板按 `AuditLogEntry.id` 前缀标注来源（P3.1 落地项），措辞不得暗示与加密库行同等防篡改
  - 落地说明：`SecurityLogTab` 对 `jsonl-` 前缀行挂「补录」标签，`title` 说明「从库外明文日志补录、可信度低于库内记录」，不做防篡改承诺
- [x] 新增测试：补录行 `id` 以 `jsonl-` 开头；原生行不以 `jsonl-` 开头

**依赖**：V2.3

### V2.6 恢复备份后的游标对齐（R11）

对应规格 §2.5 第 9 条（已知取舍）。

- [ ] 恢复/回滚备份成功后把 `JSONL_SYNC_KEY` 对齐到备份点时间，避免「恢复点之后」的 JSONL 事件被补入当前库（**暂不实现**：现状恢复流程只克隆仓库副本并只读打开会话，不替换本地库，故不产生时间线回退；真正会触发 R11 的是用户手工替换仓库目录）
- [x] 若暂不实现，必须在恢复流程文档中声明「补录可能注入恢复点之后的事件」这一已知行为，不得静默（已在规格 §2.5 第 9 条「已知取舍：库回滚与跨库污染（R11）」声明）

**依赖**：V2.1

---

## 阶段 V3 — 查询面与 detail 收敛（§2.4 / §2.6）

### V3.1 查询接口参数化（O6）

对应规格 §2.6。

- [x] `assistant-vault/src/audit/mod.rs::query_log` 签名改为 `query_log(conn, limit, since: Option<i64>, event_types: Option<&[String]>)`
- [x] SQL 改为条件拼接：`timestamp >= ?since` 可选（闭区间，与 V2.1 的 `read_since` 口径一致，避免同毫秒行被游标跳过）、`event_type IN (...)` 可选，仍固定 `ORDER BY timestamp DESC`
- [x] **排序补稳定次级键（R20）**：`ORDER BY timestamp DESC, id DESC`；同毫秒多条时顺序必须确定，否则面板翻页会重复 / 漏项
- [x] `limit` 夹取：默认 100、上限 500；非正值回落到默认值（禁止 `LIMIT 0` 返回空）
- [x] `event_type` 集合为空数组时按「不过滤」处理，不拼出 `IN ()` 语法错误
- [x] `src-tauri/src/cmd/vault/audit.rs::vault_query_audit_log` 透传两个新参数；命令名与前置条件不变
- [x] `src/vault/core/slices/vaultAuditSlice.ts::queryAuditLog` 与 `vaultService.queryAuditLog` 透传可选参数
- [x] 未接入 UI 前，不得在任何文案 / 文档中宣称「用户可查看审计日志」（规格 §2.6 末条）

**依赖**：无

### V3.2 `AuditLogEntry` 追加 `entryId`（O6）

对应规格 §2.6「规格要求」中「返回项必须包含 … 条目级事件额外返回解析出的 `entryId`」一条。

- [x] `assistant-vault/src/dto/audit.rs::AuditLogEntry` 追加 `pub entry_id: Option<String>`
- [x] 输出 DTO 的 `Option` **不加** `skip_serializing_if`：键恒在、值 `null`，前端声明 `string | null`
- [x] `query_log` 内部按 §2.4 消歧规则解析 `detail` 填充 `entry_id`（规则本身**不复述**，以规格 §2.4 为唯一实现入口）；解析不出填 `None`
- [x] `npm run contract:gen`，提交 `src/vault/core/contract/vault.contract.ts` 的生成结果（禁止手改）
- [x] `assistant-vault/tests/dto_contract.rs` 补 `entryId` 键恒在断言

**依赖**：V3.1（同一函数一起改）

### V3.3 detail 收敛与入参校验前置（O5 / R22 / G14）

对应规格 §2.4。

- [x] `src-tauri/src/cmd/vault/sync.rs`：冲突揭示的 `vault_reveal` / `vault_reveal_rate_limited` 的 `detail` 由路径原文改为 `conflict:{entryId}:{side}:{field}:{pathDigest}`（规格 §2.4）
- [x] entryId 必须来自 `sync_engine::preview::parse_entry_id`：该函数为 `pub(super)`，需在 `assistant-vault` 内新增公开辅助（如 `sync_engine::entry_id_from_path`）或在 `reveal_conflict_field` 内一并返回 `(entryId, digest)`；**禁止**在命令层复制 `entries/xxx.enc` 的解析逻辑（§2.7.2 单一入口）（落地为 `sync_engine::conflict_audit_detail`，命令层只调用）
- [x] 限流路径（`VaultRevealRateLimited`）在 `check_and_record()` 失败时拿不到 entryId 也不应退化为路径原文：无法解析时写 `conflict:unknown:{side}:{field}:{digest}`，`unknown` 属封闭集合成员（`t_a22` 覆盖畸形路径）
- [x] **`side` / `field` 就地校验**：限流分支写审计发生在 `reveal_conflict_field` 之前，二者同样是未校验的前端入参；不在封闭集合内时写 `unknown`，**禁止**把原串写入 detail（`t_a22` 的 `bad_side` / `unresolved`）
- [x] `src-tauri/src/cmd/vault/reveal.rs`：`vault_reveal_password` / `vault_reveal_notes` 的限流分支在 `check_and_record()` 失败时直接用前端传入的 `id` 写 `vault_reveal_rate_limited`（此时未做任何库内校验）→ 写入前校验 UUID 形态，不合法写 `unknown`（规格 §2.4「入参校验前置」）
- [x] 新增测试：限流分支传入非 UUID 的 `id`（如 `a:b`、`123`、含换行串）→ `vault_reveal_rate_limited` 的 detail 为 `unknown`，不含原串（`t_a23`）
- [x] 逐条核对各调用点是否落在 §2.4 的形态集合内（形态清单**不复述**）；不符合的调用点就地改到对应形态（逐事件样本断言见 `t_a29`）
- [x] 新增测试：`path_digest` 输出固定 64 字符小写十六进制（不含 `:` / `/` / `\` 与原文字节），确保摘要不会退化成路径原文（与导出事件的 digest 形态一致）（`t_a22` 摘要断言）
- [x] 新增测试：前端传入含 `:`、空格、换行的 `path`（限流分支在校验前就写 detail）→ detail 仍恰好 5 段且不含路径原文（验证「路径串未校验」不再影响段数）（`t_a22` 的 `bad_path`）

**现状核对结论（非待办，已核对完毕）**：未校验入参写入 `detail` 的调用点仅 3 处（`reveal.rs` 两处限流分支 + `sync.rs` 冲突揭示限流分支），其余调用点均在 `decrypt_*` 或库内查找成功之后写入，ID 已被库内校验。恢复事件的 `{entryId}@{commit}` 之所以收敛，是 `git show` 成功的副产物而非安全保证，详见规格 §2.4「该『隐式校验』不构成安全保证」；恢复路径入参白名单**范围外，单独立项**。

**注**：`hibp_leak_found` 的 `{entryId}:{count}`、`dek_rotated` 的自由文本、批量删除的 `remote` 三项归 V4.2 处理，本批次只做路径摘要化；本批次的「入参校验前置」对应 R22 / G14（限流分支写未校验前端入参）。

**依赖**：无

---

## 阶段 V4 — 实现整齐性收敛（§2.7）

判定标准见规格 §2.7.2：**同一语义只允许一个实现入口、一个失败级别、一个文法形态**。

### V4.1 自回响防护（R12，**必须先于 V2.3**）

自回响只发生在「**既双写、又在白名单内**」的事件上，当前是 `dek_rotated` 与 `local_db_rebuilt`（都用 `log_dual` 在同一 `at` 写 DB + JSONL）。`vault_unlock` / `vault_lock` 虽双写但不在白名单，被过滤挡住，不构成本条问题。

现状挡重复靠两重机制：`NOT EXISTS (event_type, timestamp)` + PK `jsonl-{at}-{event}` 上的 `INSERT OR IGNORE`。V2.3 为区分同毫秒多行会把 `seq` 并入 id，PK 这一重失效；若去重键同时改掉，原生行没有新键字段 → **每次解锁都会给 `dek_rotated` / `local_db_rebuilt` 多插一条 `jsonl-` 假补录行**（`>=` 游标会反复读到）。

- [x] 补录循环先按 `(event_type, timestamp)` 单独查一次原生行，命中则跳过（该键与原生行天然一致，与 V2.3 的新去重键互不干扰）
- [x] 或让去重条件同时对照原生行键（与上一条二者择一，写进注释说明取舍）
- [x] 无论 V2.3 选 `seq` 还是 `(文件名, 行号)`，**都不得**让原生行与同次写入的 JSONL 行互相漏去重
- [x] 新增测试：执行一次 DEK 轮换 → `dek_rotated` 在 `security_audit_log` 中**恰好 1 行**（现状 1 行；改 V2.3 后漏做本项会变成每次解锁多 1 行）
- [x] 新增测试：重建本地库 → `local_db_rebuilt` **恰好 1 行**
- [x] 新增测试：连续解锁 3 次 → `vault_unlock` **恰好 3 行**（现状成立，作为「若有人把 `vault_unlock` 加进白名单」的守卫）

**依赖**：无（但 V2.3 依赖本项）

### V4.2 `detail` 文法收敛（R13 / R14 / R15）

对应规格 §2.4（五形态 + 两组三元组 + `:` / `@` 消歧规则）。

- [x] `assistant-vault/src/lifecycle.rs::rotate_dek`：`dek_rotated` 的 detail 由 `"{}->{} entries {}"` 改为 `"{old}:{new}:{count}"`（R14）
- [x] `assistant-vault/src/security_db/entries/write.rs::delete_entries_tx`：批量删除 detail 由调用方标签（`remote`）改为 `{entryId}@{source}`，逐条用各自 `id`（R15）
- [x] 同步更新调用方 `sync_engine/pull_push.rs` 的入参语义：`Some("remote")` 表示 **source 标签**，不再表示整条 detail
- [x] `hibp_leak_found` 的 `{entryId}:{count}` 形态保留；与 `{tag}:{entryId}` 的消歧规则**不复述**，一律按规格 §2.4（R13）实现（`audit::parse_detail` 为唯一解析入口）
- [x] `@` 形态按**首个 `@`** 切分（规格 §2.4 第 4 条），禁止要求「恰好 2 段」：历史恢复的 suffix 是 git 引用，`HEAD@{1}` 这类 reflog 语法本身含 `@`
- [x] 新增测试：`hibp_leak_found` 的 `{uuid}:{count}` 与 `vault_copy_field` 的 `totp:{uuid}` 按同一解析函数分别得到 `{entryId, count}` 与 `{tag, entryId}`；`hibp_leak_found` **不得**被解析成 `tag:entryId`（`t_a27`）
- [x] 新增测试：`{format}:{count}:{digest}:failed`（4 段）与 `{format}:{count}:{digest}`（3 段）分别解析为失败/成功，不得互串（`t_a26`）
- [x] 新增测试：`dek_rotated` detail 用 `split(':')` 恰好 3 段且后两段为数字（`t_a24`）
- [x] 新增测试：远端同步删除 3 条 → 3 行 detail 均为 `{各自 entryId}@remote`（`t_a25`）
- [x] 新增测试：`{uuid}@HEAD@{1}` 解析为 `entryId = uuid`、`suffix = HEAD@{1}`（首 `@` 切分，不被 3 个 `@` 段误导）（`t_a27`）

**依赖**：V3.2（`entryId` 解析函数）

### V4.3 通道与语义纠偏（R16 / R17）

- [x] `vault_copy_field` 语义宽化写进注释与前端文案约束：`vault_totp_generate` 复用该事件表示「字段明文被读出」，**前端文案不得一律写「复制」**（R16）
- [x] `ssh_key_removed` 通道标注纠正：单条路径走 `audit::log` 实为 **D**；锁库批量 `ssh_remove_all_on_lock` **现状零审计写入**（G5），补齐后为 **J**；规格 §2.3 表已按此改正（R17）
- [x] 若产品要求「单条移除也需锁库后可追溯」，则把 `cmd/vault/ssh.rs` 的单条路径改为 `state.audit_dual(..)` 升为 B，并同批更新 §2.3 表——**二选一，不得既标 D 又按 B 断言**（已选定：保持 D + 锁库批量 J，不升 B）
- [x] 两条 `ssh_key_removed` 写入点共用同一入参校验 `audit::sanitized_fingerprint`，同事件只有一个 detail 形态（`t_a28`）

**依赖**：无

### V4.4 失败级别、返回类型、时间源（R18 / R19 / R21）

- [x] `assistant-vault/src/audit/mod.rs::write_db_at` 的 `error!` 改为 `warn!`（R19）
- [x] 审计原语返回类型统一为 `()`（`audit::log` / `log_dual` / `SecurityDb::audit_dual` / `SecurityDb::audit_jsonl`），内部自行 `warn`，**从类型上消除 `?` 传播的可能**（R18）
- [x] 若评估后保留 `Result`，则必须在规格 §4 明文禁止生产调用点用 `?` 传播，并把 `cmd/conversation.rs` 的现有写法一并改掉（已按上一条统一为 `()`，本项不适用）
- [x] 时间源统一：`log_dual` 的 `chrono::Utc::now().timestamp_millis()` 改为 `crate::types::now_millis()`（R21）
- [x] 新增测试：审计写入失败时业务仍返回成功（注入只读连接或坏路径）（`t_a30` 无 schema 连接 + `t_a8` 审计目录不可写）

**依赖**：无（与 P2.1 的工作区行修正同源，建议同批）

### V4.5 三张表的一致性测试（§2.7.2 末条）

- [x] `BACKFILL_EVENTS`：J 集合 ⊆ 白名单（V1.5 已列，此处仅确认归属）
- [x] 事件词表：`AuditEvent` 的 `Display` 串两两唯一，且与前端字典全集一致（导出快照比对，禁止手抄）（后端唯一性已由 `audit_event_names_unique` 覆盖；前端字典比对随 P5.1 落地后补断言）
  - 落地说明：`auditLexicon.test.ts` 直接解析 `audit/mod.rs` 的 `Display` 分支取出全部串，断言两两唯一且与 `VAULT_AUDIT_EVENT_TYPES` 集合相等（`outcome_str` 同样比对）
- [x] `detail` 文法：每个事件的样本 detail 落在 §2.4 形态集合内（`t_a29` + `sample_detail` 穷尽 match）
- [x] 新增事件 / 新增 tag / 改 detail 形态时，三张表任一漏改必须让测试失败（新增事件漏补样本 → 编译失败；新增 tag 未入 `DETAIL_CLOSED_TAGS` → 落 `Raw` 被 `t_a29` 拦下）

**依赖**：V1.5、V4.2、P5.1

---

## 阶段 T1 — outcome 词表扩展（§3.1）

### T1.1 三个新 outcome

对应规格 §3.1 词表 / §3.2 / §3.3。

- [x] `assistant-tools/src/gateway.rs::AuditOutcome` 追加 `ValidateRejected`、`IntentError`、`Bypassed`
- [x] 枚举本身 `#[serde(rename_all = "camelCase")]` 自动生成 wire 名，不手写字符串
- [x] `src-tauri/src/adapters/permission_store.rs::outcome_str` 补三个分支，落库串为 `validateRejected` / `intentError` / `bypassed`
- [x] `bypassed` 语义写进枚举文档注释：**未经权限策略求值即执行**，只允许空 intent 短路与 ToolBox 直调两个触发点；不得用它表示「策略允许」
- [x] 前端展示映射与统计口径由 P5.1 落地（规格 §3.1 / §6）；本批次只保证后端落库串定型，**不**在前端硬编码 outcome 串

**依赖**：无

---

## 阶段 T2 — 决策点覆盖（§3.2 / §3.3）

### T2.1 intent 求值失败可审计（G7）

对应规格 §3.2。

- [x] `ToolGateway::evaluate` 中 `let intents = tool.permission_intents(input, context)?;` 改为 `match`：`Err(e)` 时先 `self.audit(tool_name, "none", "intent:error", AuditOutcome::IntentError).await`，再 `return Err(e)`
- [x] detail 只记固定串 `intent:error`，**不**把错误文本写进 `resource`（可能夹带入参片段）
- [x] 新增测试：构造 `permission_intents` 返回 `Err` 的桩工具，断言返回 `Err` 且 `InMemoryAuditStore` 收到 1 条 `IntentError`

**依赖**：T1.1

### T2.2 Validate 失败可审计（G7）

对应规格 §3.2。

- [x] `ToolGateway` 追加 `pub async fn audit_external(&self, tool, action, resource, outcome)`，内部复用私有 `audit()`，作为 pipeline 的唯一外部写入口
- [x] `assistant-tools/src/pipeline.rs` `validation.ok == false` 分支：在构造 `ToolOutcome` 之前调 `self.gateway.audit_external(tool_name, "none", "validate:rejected", AuditOutcome::ValidateRejected).await`
- [x] 禁止在 `ToolPipeline` 里另注入一个 `PermissionAuditStore`：审计写入必须只有网关一个入口
- [x] `blocks_input_rewrite` 与返回的 `ToolOutcome` 保持不变（审计不改变业务结果）
- [x] 新增测试：`blocks_input_rewrite=true` 与 `false` 两种校验失败都产生 1 条 `ValidateRejected`

**依赖**：T1.1

### T2.3 空 intent 短路记 `bypassed`（G6）

对应规格 §3.3。

- [x] `ToolGateway::evaluate` 的 `if intents.is_empty()` 分支：先 `self.audit(tool_name, "none", "intent:none", AuditOutcome::Bypassed).await`，再 `return Ok(GatewayDecision::Allowed)`
- [x] `resource` 固定为 `intent:none`，`action` 固定为 `none`：此处没有具体动作可记
- [x] 审计失败不影响返回 `Allowed`（§4：审计不阻断业务）
- [x] 新增测试：`ask_user`（`AGENT_ONLY`，唯一真实经过网关的空 intent 工具）产生 1 条 `bypassed`

**依赖**：T1.1

### T2.4 决策分支全覆盖回归

对应规格 §3.2 / 验收 §8「测试（工具）」第 1 条。

- [x] `assistant-tools/tests/gateway.rs` 新增表驱动用例：`tier 越界` / `Denied` / `GrantHit` / `Allowed` / `Approved` / `SessionApproved` / `AlwaysApproved` / `Rejected` / `TimedOut` / `Bypassed` / `IntentError` 各断言恰好 1 行且 `outcome` 与分支匹配
  - 落地说明：`gateway_decision_branches_each_audit_exactly_once` 覆盖上列 11 个分支；`ValidateRejected` 发生在网关之前，用例落在 `assistant-tools/tests/pipeline.rs`（`blocks_input_rewrite` 两种取值各断言 1 条）
- [x] 断言 `permission_audit` 行的 `action` / `resource` 非空（`none` 也是合法固定值，不允许空串）
- [x] `InMemoryAuditStore::entries()` 作为唯一事实源，禁止断言日志文本

**依赖**：T2.1–T2.3

---

## 阶段 T3 — intent 声明（§3.4）

### T3.1 只读工具声明 `read`（G6）

对应规格 §3.4。

- [x] `assistant-tools/src/os/agent_read.rs`：`permission_intents` 返回 `vec![PermissionIntent::new("read", 解析后的绝对路径)]`（`PermissionIntent { action, resources: Vec<String> }`，单资源用 `new()`）
- [x] `agent_list.rs`（`ls`）同上，`resource` 为目录绝对路径
- [x] `agent_glob.rs` 同上，`resource` 为 `path` 参数解析后的绝对路径
- [x] `agent_grep.rs` 同上
- [x] 四个工具的资源解析必须走现有 workspace 解析函数，保持与写类工具同一套归一化；解析失败返回 `Err`（由 T2.1 记 `intentError`）
- [x] 声明 `read` 不新增规则：`dangerous_only` 档位下基础规则 `("read", "*") → Allow` 直接放行；`all_os_tools` 档位把 `read` 转 `Ask` 是该档位既有语义，不算回归。禁止新造 `list` / `glob` / `grep` action 名（会落到 `("*","*") → Ask`，凭空增加审批）
- [x] 每个工具补一条测试：声明恰好 1 条 intent、`action == "read"`、`resource` 为绝对路径
  - 另补一条「无工作区基准时返回 `Err` 而非空 intent」的测试，锁住 §3.4 的失败路径（不得退回 `bypassed` 口径）

**依赖**：无（与 T1 并行）

### T3.2 `fs_link action=check` 声明 `read`（G6）

对应规格 §3.4 第 3 条。**注意**：`fs_link` 是 `TOOLBOX_ONLY`，当前不经过网关，本项**不改变**现有审计口径（ToolBox 可见性由 T4.1 的 `bypassed` 保证），只是消除「返回空 intent」的不一致。

- [x] `assistant-tools/src/os/fs_link/mod.rs` 的 `action="check"` 分支：`resource` 改为目标路径解析后的绝对值，`action` 改为 `"read"`，不再返回 `vec![]`
- [x] `action="create"` / `"remove"` 分支保持现有 intent 不变
- [x] 新增测试：`action=check` 返回 1 条 `("read", 绝对路径)`；并更新现有断言「`check` 返回空」的用例（这是行为修正，不是放宽）

**依赖**：无

### T3.3 `ask_user` 明确不声明 intent

对应规格 §3.4 第 2 条。

- [x] `agent_ask_user.rs` **不**加 `permission_intents`；其可见性由 T2.3 的 `bypassed` 保证
- [x] 新增测试断言 `ask_user` 的 intent 列表为空（防止后人「顺手」补一个 intent 把审计口径改掉）

**依赖**：T2.3

---

## 阶段 T4 — ToolBox 直调审计（§3.5）

### T4.1 `os_tool_exec` 补审计（G8）

对应规格 §3.5。

- [x] `assistant-tools/src/gateway.rs`：新增 `impl AuditEntry { pub fn bypassed(tool: &str, resource: &str) -> Self }`，内部复用私有 `now_millis()`，`action` 固定 `"none"`、`outcome` 固定 `Bypassed`——时间源只留一处，禁止在命令层另造毫秒函数
- [x] `src-tauri/src/tool_setup.rs`：在把 `audit` 交给 `ToolGateway` 之前追加 `handle.manage(audit.clone())`（现状 `DbPermissionAuditStore` 只被塞进网关，命令层拿不到）
- [x] `src-tauri/src/cmd/os_tool_exec.rs` 增加入参 `audit: State<'_, Arc<dyn PermissionAuditStore>>`，在 `invoke` **之前** `if let Err(e) = audit.log_decision(AuditEntry::bypassed(registered.tool.id(), "toolbox")).await { log::warn!(..) }`
- [x] 审计失败只 `warn`，`invoke` 照常执行，返回 `OsToolResultDto` 不变
- [x] 未注册工具（`find_for_toolbox` 返回 `None`）**不**写审计：没有发生任何工具动作
- [x] 固定在 `invoke` 之前写：`bypassed` 表示「未过策略」，与执行成败无关；放到之后会漏掉 panic / 超时路径
- [x] `resource` 固定串 `toolbox`：禁止写用户路径原文
- [ ] 新增测试（`src-tauri` 侧能构造 state 则写集成测试，否则列入手工验收）：从 ToolBox 执行一次 `delete`，断言主库 `permission_audit` 多出一行 `toolbox` / `bypassed`
  - 现状：`src-tauri` 无 Tauri mock app 依赖（dev-dependencies 只有 `minidump` / `tempfile` / `tokio-util`），命令层无法在单测内构造 `State`；`AuditEntry::bypassed` 的形态已由 `assistant-tools` 侧测试锁定，本项转手工验收

**依赖**：T1.1

---

## 阶段 T5 — 保留与索引（§5）

### T5.1 JSONL 总量上限（G12）

对应规格 §5「已知取舍」第 1 条。

- [x] `assistant-vault/src/audit/jsonl.rs` 新增 `prune_before(cursor_ms, retention_days)`：只删除「按日文件名推导的日期早于 `cursor` 所在日、且早于 `retention_days`」的文件
- [x] 调用点放在 `try_backfill_jsonl_audit` 成功之后，传入**刚推进的游标**与保留天数；禁止用 `now` 作判据
- [x] 保留天数从配置读（默认 90），`0` 表示不清理（与 `spawn_result_cleanup` 的 0=禁用口径一致）
- [x] 删除失败只 `log::warn!`，不影响补录结果
- [x] 新增测试：造 3 个日文件，游标落在中间，断言只删游标之前且超期的文件，游标之后的文件必须保留
  - 验收：`cargo test -p assistant-vault --test audit` 35 条通过（`t_a31` 三文件裁剪、`t_a32` 0=禁用与畸形名保留、`t_a33` 解锁后按推进游标清理）
  - 实现要点：保留天数由 app 层从 `AppConf.audit_retention_days` 注入 `SecurityDbState`（库层不读配置），未注入时默认 `0`（不清理）；判据同时要求「早于游标所在 UTC 日」与「早于今天减保留天数」，任一日期推导失败即放弃清理；`symlink_metadata` 保证符号链接与非普通文件不删

**依赖**：V2.1（游标语义修正后才谈得上「早于游标」）

### T5.2 `permission_audit` 时间索引（O3）

对应规格 §5「已知取舍」第 2 条。

- [x] `assistant-storage/src/db.rs` `init` 收尾前新增 `CREATE INDEX IF NOT EXISTS idx_permission_audit_at ON permission_audit(at_millis)`
- [x] 与其它索引同批执行；存量库靠 `IF NOT EXISTS` 幂等，不需要 `has_*` 守卫
- [x] 新增测试（`assistant-storage` 侧）：索引存在且按 `at_millis` 范围查询可用（`EXPLAIN QUERY PLAN` 不含 `SCAN permission_audit`）
  - 验收：`cargo test -p assistant-storage --test db` 14 条通过（含 `init_db_creates_permission_audit_time_index`）

**依赖**：无（P4 反过来依赖本项：查询面必须先有索引）

---

## 阶段 P1 — 锁定归因（§2.3 F5）

### P1.1 `vault_lock` 接受 `reason`，`AutoLock` 落地

对应规格 §2.3「`vault_lock` 的归因」/ 差异表 F5。

- [x] `src-tauri/crates/assistant-vault/src/dto/lifecycle.rs` 新增 `LockReasonDto`：`user` / `autoLock` / `screenLock` / `appExit` / `reset` / `panic`，`#[serde(rename_all = "camelCase")]`
  - 落地说明：放在 vault 契约域（`#[ts(export, export_to = "vault.contract.ts")]`），与 `security_db::LockReason` 同 crate；`protocol/dto/config.rs` 只留配置 DTO（`VaultConfDto` / `VaultCipherAlgorithmDto`），不承载运行时归因枚举
- [x] 输入 DTO 的 `Option` 加 `#[ts(optional = nullable)]` 并加 `deny_unknown_fields`（输入侧规则）
  - 落地说明：本项为命令参数 `Option<LockReasonDto>`，非结构体输入 DTO；命令参数由 Tauri 逐项反序列化，缺键即 `None`，无需 `ts(optional = nullable)` / `deny_unknown_fields`
- [x] `src-tauri/src/cmd/vault/lifecycle.rs::vault_lock` 签名改为 `async fn vault_lock(app, reason: Option<LockReasonDto>)`：`reason` 随 `move` 进 `spawn_blocking` 闭包，缺省映射 `LockReason::User`
  - 落地说明：`reason` 在闭包外先经 `reason.map(LockReason::from).unwrap_or(LockReason::User)` 转成 `LockReason`（`Copy`），再随 `move` 进闭包；缺省值是命令层策略，不是 DTO 语义
- [x] 映射实现落在 vault 契约层，`LockReason` 本体不加 serde：审计 detail 必须继续用 `as_str()` 的 snake 串，不得改用 serde 名
  - 落地说明：`impl From<LockReasonDto> for LockReason` 位于 `assistant-vault/src/dto/convert.rs`（同 crate，无孤儿规则限制）；`security_db::LockReason` 未加任何 serde derive，detail 仍取 `as_str()`
- [x] 前端 `src/vault/core/vault.service.ts::lock` 由 `(): Promise<void>` 改为 `(reason?: LockReason): Promise<void>`，`invoke("vault_lock", { reason })`
  - 落地说明：`LockReason` 由 `../vault.types` barrel 的 `LockReasonDto as LockReason` 别名提供，来源是生成的 `vault.contract.ts`，未手写同名类型
- [x] `src/vault/core/slices/vaultLifecycleSlice.ts` 的 `lock` 动作透传可选 `reason`；**自动锁定定时器**（`setTimeout` 内 `get().lock()`）改调 `get().lock("autoLock")`，手动锁定调用点（`VaultApp.tsx` 的 `await lock()`、锁定菜单）**不传**
  - 落地说明：手动锁定断言 `{ reason: undefined }`、自动锁定断言 `{ reason: "autoLock" }`，均由 `vaultStore.test.ts` 覆盖
- [x] 禁止在前端本地记录锁定原因，也禁止为归因新增本地状态（不得把 `reason` 存进 store）
  - 落地说明：`reason` 仅在调用链上传参，未进 store、未落任何前端状态
- [x] `npm run contract:gen`，提交生成结果（禁止手改生成类型）
  - 落地说明：生成 `src/vault/core/contract/vault.contract.ts` 的 `LockReasonDto = "user" | "autoLock" | "screenLock" | "appExit" | "reset" | "panic"`；`src/types/protocol.contract.ts` 不再含该类型
- [x] 新增测试：`LockReasonDto::AutoLock` → `LockReason::AutoLock` → `as_str() == "auto_lock"` 全链断言；`None` → `user`
  - 验收：映射与 wire 冻结断言在 `assistant-vault/tests/dto_contract.rs`（`lock_reason_dto_wire_is_frozen` / `lock_reason_dto_maps_one_to_one_to_domain`）；命令层缺省断言在 `cmd/vault/lifecycle.rs`（`missing_reason_defaults_to_user`）；`cargo test -p assistant-vault --test dto_contract` 与 `cargo test -p assistant-app --lib lock_reason` 全过

**依赖**：无

---

## 阶段 P2 — 授权管理动作审计（§3.6 F3）

### P2.1 三个 grants 写命令补审计

对应规格 §3.6。

- [x] `src-tauri/src/cmd/permission.rs`：三个写命令各追加 1 条 `permission_audit` 行，`tool = "grant"`、`outcome = "allowed"`
- [x] `action` 固定串：`revoke_permission_grant` → `grant:revoke`；`clear_permission_grants` → `grant:clear`；`set_permission_grant_scope` → `grant:scope`
- [x] `resource` 为规范化 `grantKey`：`revoke_permission_grant` 用入参 `grant_key`；`set_permission_grant_scope` 用**变更前**的源键 `args.grant_key`（不是新算出的 `target_key`）；`clear_permission_grants` 写固定串 `none`（无单条目标）
- [x] `outcome` 串**复用** `permission_store.rs::outcome_str` 的 `allowed` 分支：把该函数改为 `pub(crate)` 并在 `cmd/permission.rs` 调用，禁止在命令层硬编码 `"allowed"`、禁止新增 outcome 变体
  - 落地说明：`outcome_str` 原本已是 `pub(crate)`，只需在 `adapters/mod.rs` 增加 `pub(crate) use ... outcome_str` 导出；`cmd/permission.rs` 的 `write_grant_audit` 统一走 `outcome_str(entry.outcome)`，命令层无 `"allowed"` 字面量
- [x] **禁止**用 `denied` / `bypassed` 表示管理动作（规格 §3.6 约束）
- [x] 审计行与授权变更**同一事务**：`set_permission_grant_scope` 已有 `unchecked_transaction`，审计 INSERT 放进同一 tx；`revoke_permission_grant` 与 `clear_permission_grants` 现状是裸 `conn.execute`（无事务），需包一层 tx 并把审计写入放进 tx
- [x] 审计行直接 `INSERT INTO permission_audit (tool, action, resource, outcome, at_millis)`，**不**走 `PermissionAuditStore::log_decision`：该实现内部重新锁 `DbState` 互斥量并另取连接，无法与授权变更共用事务
- [x] `at_millis` 取值复用 `AuditEntry` 的时间源（见 T4.1 的 `now_millis()`），禁止在命令层另造毫秒函数
  - 落地说明：在 `assistant-tools::gateway::AuditEntry` 新增 `AuditEntry::allowed(tool, action, resource)`（与 `bypassed` 同用私有 `now_millis()`），命令层只调构造器、不碰时间源
- [x] 受影响行数为 0（目标已不存在）时**不写**审计，命令返回值保持现状（`revoke` / `set_scope` 仍返回 `bool`，`clear` 仍返回 `()`；`clear` 需从 `execute` 返回值取 affected 判断，不改变返回类型）
- [x] 审计 INSERT 失败时 `log::warn!` 后继续提交事务，**不得**回滚授权变更、**不得**把失败包装成 `Err` 返回前端（§3.6「与 §4 的边界」）
  - 落地说明：`write_grant_audit` 返回 `()` 而非 `Result`，调用点无法用 `?` 把失败升级成错误
- [x] 新增测试：三命令各产生 1 行且 `tool/action/outcome` 匹配；对不存在的 grant 执行 revoke 不产生行
  - 验收：为可测性把三个命令的事务体抽成纯库层函数 `revoke_grant_row` / `clear_grant_rows` / `move_grant_scope`，新增 5 条测试（revoke 有行 / revoke 无行 / clear 有行且空表二次清除不再写 / 改作用域记源键且行真迁移 / 改作用域无源行）；`cargo test -p assistant-app --lib` 83 条通过
- [x] （顺带修正，规格 §3.6「现状偏差（待修）」）`cmd/conversation.rs` 工作区切换行的审计 INSERT 由 `map_err(..)?` 改为 `warn` 不中断，与 §4「不阻断业务」对齐

**依赖**：无（仅写不读，不依赖 P4）

---

## 阶段 P3 — vault 审计面板（§2.6 F1）

### P3.1 设置页新增 `security_log` tab

对应规格 §2.6「前端必须提供面板」/ 差异表 F1。

- [x] `src/vault/settings/VaultSettings.tsx` 新增 tab id `security_log`（与既有 `sync` / `ssh_agent` / `recovery` / `change_password` / `import_export` / `danger_zone` 并列）
  - 落地说明：面板组件 `src/vault/settings/SecurityLogTab.tsx`，tab 位置排在 `import_export` 之后、`danger_zone` 之前。`VaultSettings` 新增可选 `onOpenEntry` 透传，`VaultApp` 传入「关闭设置抽屉 + `handleSelect`」，因此抽屉与 `/settings/vault` 两处入口行为一致
- [x] 新增面板组件：消费 `vaultAuditSlice.queryAuditLog`，列 `time` / `event` / `detail` / `entryId`
  - 落地说明：行左列为 `time · detail`（`detail` 为空显示「无详情」），首行事件名走 P5.1 的 `vaultAuditEventText`（未知事件原样兜底）。`entryId` 受抽屉宽度（`min(480px, 100vw)`）限制无法整列展示：可跳转行右列显示「打开条目」并以 `title` 挂完整 id；不可跳转但有 `entryId` 的行右列直接显示原串（含 `unknown` 哨兵，按 §6 原样渲染）
- [x] **锁定态禁用**：`isLocked` 时不发请求、显示解锁提示；`locked` 错误码不得作为异常弹出（OF3）
  - 落地说明：取数 `useEffect` 在 `isLocked` 时直接 `return`（不发请求、清空行与错误）；渲染层先判 `isLocked || error?.code === "locked"` 输出解锁提示并提前返回，故 `locked` 永不走 `<Alert>` 分支。解锁后 `isLocked` 变化自动重新取数
- [x] 筛选：`eventType`（多选，对应 V3.1 的 `eventTypes`）与时间范围（对应 `since`）；默认只拉 100 条
  - 落地说明：事件多选选项来自 `VAULT_AUDIT_EVENT_TYPES`（30 项，标签为 i18n 文案，`showSearch={{ optionFilterProp: "label" }}`）；时间范围 `all` / `24h` / `7d` / `30d` 换算为 `since = now - N 小时`（`all` 不传）；筛选变化即重新查询，另设「刷新」按钮改 `reloadToken` 触发同一取数路径
- [x] **禁止**「加载全部」入口，`limit` 上限 500（OF5）
  - 落地说明：`limit` 下拉仅 `100 / 300 / 500`（默认 100），无「全部」项；后端 `clamp_query_limit` 为最终兜底
- [x] **禁止**在 store 里保存审计副本、增量合并、乐观插入（EF1）：面板只读后端返回
  - 落地说明：结果只落在组件本地 state（`rows`），store 无新增字段；每次筛选/刷新整体替换，不做追加合并
- [x] 条目级行可点击跳转条目详情（仅当 `entryId` 非空）
  - 落地说明：`openableEntryId` 仅放行非空且非 `unknown` 哨兵的 `entryId`；行用 `role="button"` + `tabIndex=0` + Enter/Space 键盘可达
- [x] **补录行标注来源**：按 `AuditLogEntry.id` 前缀 `jsonl-` 标注「来自库外日志补录」，措辞不得暗示与加密库行同等防篡改（§2.5 第 8 条 / §6）；**禁止**按时间做增量追加（补录行会被窗口过滤掉）
  - 落地说明：`isBackfilled` 判 `id.startsWith("jsonl-")` → 行首「补录」Tag，`title` 为「该记录在重建本地库时从库外明文日志补录而来，可信度低于库内记录」（en-US 同义），只声明来源、不声明防篡改；面板无增量追加
- [x] i18n 键统一 `vault.tab.security_log` / `vault.security_log.*`，**不得**复用 `vault.hibp.audit_all`（F8）
  - 落地说明：新增 `vault.tab.security_log` + `vault.security_log.*`（描述/占位/时间范围/条数/刷新/失败/空/锁定提示/补录/无详情/打开条目）双语言齐全，未引用任何 `vault.hibp.*` 键

**依赖**：V3.1、V3.2（参数与 `entryId`）、P5.1（事件字典）、V2.5（来源标注语义）

---

## 阶段 P4 — 权限审计查询命令（§3.7 F2）

### P4.1 `list_permission_audit` 后端命令

对应规格 §3.7 / 差异表 F2。

- [x] `src-tauri/src/protocol/dto/agent.rs` 新增 `PermissionAuditEntryDto`（与 `PermissionGrantDto` 同文件，`mod.rs` 已 `pub use agent::*`）：`tool` / `action` / `resource` / `outcome` / `atMillis`，全部非 Option；`#[serde(rename_all = "camelCase")]`，不加 `deny_unknown_fields`（输出 DTO）
- [x] 输出 DTO 的 `Option` **不加** `skip_serializing_if`（本 DTO 无 Option，仅记录规则）
- [x] `src-tauri/src/cmd/permission.rs` 新增 `list_permission_audit(app, limit: Option<i64>, since: Option<i64>, until: Option<i64>, offset: Option<i64>)`：`limit` 默认 100、上限 500、非正值回落默认；`since` / `until` 为毫秒闭区间；`offset` 非正值回落 0；返回 `PermissionAuditPageDto { entries, hasMore }`，固定 `ORDER BY at_millis DESC, id DESC`
  - 落地偏差：参数取 `Option<i64>` 而非 `Option<u32>`。本条目同时要求「负数回落默认」与「负数测试」，而 `u32` 遇负数会在 IPC 反序列化阶段直接报错、走不到夹取分支；`i64` 与既有 `vault_query_audit_log` 的 `clamp_query_limit` 口径一致。夹取抽成 `clamp_audit_limit` / `clamp_audit_offset`，查询抽成纯函数 `query_permission_audit(conn, limit, since, until, offset)`，命令层只负责取连接
  - 落地偏差：原条目为「固定 `limit`、无翻页」，实施时扩为翻页（P5.3 面板需要上一页 / 下一页）。`hasMore` 由「多取一行再截断」判定，`until` 冻结窗口上界，避免新写入把后续页挤偏；分页语义见 P5.3
- [x] 命令注册到 `generate_handler!`；命令名 `list_permission_audit`
- [x] `src/services/permissionService.ts` 新增 `listAudit(limit?, since?, until?, offset?)`，内部以字面量 `invoke("list_permission_audit", ..)` 调用
- [x] `commandNames.test.ts` 的断言为「前端字面量 ⊆ `generate_handler!` 声明清单」单向（只有 vault 组是双向）：声明清单由 `generate_handler!` 自动派生，**禁止**手写清单，也**禁止**用变量拼接命令名绕过字面量检测
- [x] `npm run contract:gen`，提交生成结果（`src/types/protocol.contract.ts` 生成 `PermissionAuditEntryDto` 与 `PermissionAuditPageDto`；`tests/protocol_bindings.rs` 同步注册两个 DTO，漏注册时 `export_protocol_contract` 直接失败）
- [x] 新增测试（`src-tauri` 侧）：`limit` 夹取（0 / 负数 / 超上限）、`since` / `until` 过滤、倒序、`offset` 翻页与 `hasMore`
  - 验收：`cmd::permission::audit_tests` 覆盖夹取边界 / `since` 闭区间 + 倒序 / `until` 冻结窗口 / `offset` 分页无重叠 / `hasMore` 精确边界 / 上限档位 `hasMore` / 同毫秒稳定序 / `limit` 作用在 SQL 上；`cargo test -p assistant-app --lib cmd::permission` 通过；`vitest run commandNames.test.ts` 3 条通过

**依赖**：T1.1（outcome 串定型）、T5.2（索引先行）

---

## 阶段 P5 — 词表映射与前端面板（§6 F7 / F8）

### P5.1 事件与 outcome 字典 + i18n

对应规格 §6「词表必须封闭映射」/「未知值必须原样兜底」。

- [x] 新增前端字典：全部 `event_type`（现状 31 个，V1.2 删除 `local_db_repaired` 后为 30 个）→ i18n key；11 个 `outcome` → i18n key
  - 落地说明：字典落在 `src/services/auditLexicon.ts`（跨域共用，避开 `src/settings` 的「core 不含特性名」边界扫描）：`VAULT_AUDIT_EVENT_TYPES`（30 个）/ `EVENT_TEXT_KEYS` 与 `PERMISSION_AUDIT_OUTCOMES`（11 个）/ `OUTCOME_TEXT_KEYS`，键名分别为 `vault.security_log.event.*` 与 `settings.agent_mode.audit.outcome.*`
- [x] zh-CN 与 en-US 同步补齐，缺任一侧视为未完成
  - 验收：`auditLexicon.test.ts` 逐项断言两侧 JSON 路径存在且为非空串
- [x] **未知值兜底**：字典未命中时返回原始蛇形串（`outcome` 返回原串），**禁止**过滤该行、禁止返回空串
  - 落地说明：`vaultAuditEventText` / `permissionAuditOutcomeText` 用 `hasOwnProperty` 判定命中，未命中直接返回入参原串（含空串也不改写）
- [x] 字典键集合必须与后端枚举全量对齐，并补一条集合一致性测试（前端常量 vs 后端枚举快照）
  - 落地说明：一致性测试直接解析后端真源（`audit/mod.rs` 的 `Display` 串、`permission_store.rs::outcome_str` 的 `=> "..."`），双向集合相等；后端新增变体而前端漏补时测试失败

**依赖**：无

### P5.2 `detail` 文法解析

对应规格 §6「`detail` 按事件分支渲染」/ §2.4。

- [x] 实现 §2.4 全部形态的解析（形态清单**不复述**，以规格 §2.4 为唯一来源）
  - 落地说明：解析器落在 `src/services/auditDetail.ts`（`parseAuditDetail`），逐条对齐后端唯一入口 `assistant-vault::audit::parse_detail`：`Empty` / `Count` / `EntryId` / `EntryWithSuffix` / `TaggedEntry` / `EntryCount` / `Export`（含 failed）/ `DekRotated` / `Conflict` / `Fingerprint` / `Label` / `Raw`
- [x] **解析顺序以 §2.4 为唯一来源**（R13）：含 `@` 形态按首个 `@` 切分；禁止在本文件或组件内另立规则、禁止只按段数猜测
  - 落地说明：`DETAIL_CLOSED_TAGS`（`totp` / `conflict` / `json` / `csv`）与 `DETAIL_EXPORT_FORMATS` 导出为常量，按「封闭标签 + 段数」有序匹配；`@` 用 `indexOf` 取首个，`HEAD@{1}` 类 reflog 后缀原样保留；集合一致性由测试直接比对后端 `DETAIL_CLOSED_TAGS` / `DETAIL_EXPORT_FORMATS` 源码
- [x] **条目身份只取 DTO 的 `entryId`**：禁止前端再从 `detail` 推断条目 ID（后端 `query_log` 已解析，§2.6）
  - 落地说明：`SecurityLogTab` 跳转与展示仍只读 `row.entryId`；解析结果的 `entryId` 字段仅用于展示分类（`unknown` 与非法形态一律 `null`），不参与跳转判定
- [x] `vault_copy_field` 文案用「读取 / 查看」而非「复制」（R16：该事件也覆盖 `vault_totp_generate`）
  - 验收：`vault.security_log.event.vault_copy_field` 中英文案为「读取字段」/「Read a field」
- [x] 解析失败一律原样展示整串，**禁止**抛错、禁止显示空白
  - 落地说明：`Raw` / `Label` 分支直接返回入参原串；空串与 `null` 走 `Empty` 显示「无详情」
- [x] **禁止**把解析结果回写后端或在本地重新拼装 detail
  - 落地说明：解析为纯展示函数，无副作用、不产生新 detail 串，也不进入 store
- [x] 新增测试：每形态 1 例 + 1 例畸形串原样兜底 + `hibp_leak_found` 不得被解析成 `tag:entryId`
  - 落地说明：`src/services/__tests__/auditDetail.test.ts` 16 例，覆盖全部形态、`unknown` 哨兵、conflict 段数畸形、导出段数/尾段畸形、`HEAD@{1}` 后缀与集合一致性

**依赖**：无

### P5.3 权限审计追溯面板

对应规格 §3.7「前端必须提供追溯面板」/ §6 统计口径。

- [x] 新增面板：消费 `permissionService.listAudit`，展示 `time` / `tool` / `action` / `resource` / `outcome`
  - 落地说明：`src/settings/shared/sections/PermissionAuditSection.tsx`，挂在 `AgentModeSettings` 的 `PermissionGrantsSection` 之后；时间列、`tool` 标签、`action`、`resource`（仅摘要末段，禁路径原文）与 `outcome` 标签（未知 outcome 走字典原串兜底）
- [x] 统计（拒绝率 / 审批率）只取网关决策行：排除 `tool ∈ {workspace, grant}` 与 `resource = toolbox`
  - 落地说明：`isDecisionRow` 先行过滤；拒绝率分母只取策略求值行（`allowed` / `grantHit` / `denied` / 审批类），审批率只取「被询问」行（审批类 + `rejected` + `timedOut`）；分母为 0 显示 `—`
- [x] **禁止**用消息侧 `toolCallView.status = error` 反推拒绝（OF2）
  - 落地说明：面板只读 `permission_audit` 行，未引用任何消息侧状态；`validateRejected` / `intentError` 不计入拒绝率分母
- [x] 面板与 `PermissionGrantsSection` 分离：后者是「当前授权」，前者是「历史动作」，不得混在一个列表里
  - 落地说明：两个独立组件、各自标题与说明，审计面板只读且无撤销/清空入口
- [x] 无「加载全部」入口，`limit` 上限 500
  - 落地说明：`LIMIT_OPTIONS = [50, 100, 300, 500]` 固定四档、默认 50，语义为**每页条数**；每次翻页/筛选/刷新整段替换当页结果，无本地缓存、无增量合并（EF1）。后端 `clamp_audit_limit` 只做「非正值回落默认、超上限截断」，50 属合法取值，无需改 Rust 侧
- [x] 翻页：`hasMore` 由后端判定，`until` 冻结窗口，杜绝「条数恰好等于上限」被误读成「只有这些」
  - 落地说明：`list_permission_audit` 扩为 `(limit, since, until, offset)`，返回 `PermissionAuditPageDto { entries, hasMore }`；SQL 走 `at_millis >= ? AND at_millis <= ?` 双闭区间 + `LIMIT ? OFFSET ?`，按 `at_millis DESC, id DESC` 稳定排序（同毫秒不抖）。`hasMore` 靠「多取一行再截断」判定——本页条数 == `limit` 时前端无法区分「刚好取完」与「还有更早记录」，故判定权必须留在后端。首页加载时把最新一行的 `atMillis` 记为窗口上界并作为后续页的 `until`，新写入的决策不会插到头部把后续页挤偏；`offset` 取 `page * limit`，`clamp_audit_offset` 对非正值回落 0。面板在列表上方给「上一页 / 第 N 页 / 下一页」控件，`page === 0` 时禁用上一页、`hasMore === false` 时禁用下一页并撤掉「还有更早记录」提示；切换范围、切换每页条数、点刷新一律 `resetPaging()` 回到第 1 页并清空窗口上界。文案 `limit_value` 由「最多 N 条」改为「每页 N 条」
- [x] 结果列表定高滚动、表头吸顶，避免上百行记录把页面撑长后难以翻看
  - 落地说明：`LIST_MAX_HEIGHT = 320` 的滚动区，列头（时间 / 会话轮次 / 工具 / 动作 / 资源 / 结果）`position: sticky` 吸顶；行与列头共用同一 `grid` 列宽保证对齐；列表上方显示「当前显示 N 条」（`rows_count`）；筛选、统计与说明仍在滚动区之外，不改变整段替换语义
- [x] 展示归属坐标（`conversationId` / `turnId` / `iteration`，规格 §6「必须展示归属坐标」）
  - 落地说明：新增「会话 / 轮次」列，会话标题经 `chatService.listConversations` 解析、查不到回落 ID，`iteration` 显示为「迭代 N」，悬停提示完整三元组；全空行（管理行 / ToolBox / 迁移前存量行）显示 `—` 并给出「无会话归属」说明。列宽改为 `minmax(0, fr)` 比例列 + 行级 `ROW_MIN_WIDTH = 720`，窄窗口横向滚动，避免固定列宽之和超出容器后资源列被挤成 0 而与结果列视觉重叠
- [x] 列间分界线 + 可拖拽调整列宽
  - 落地说明：列宽改为组件内 `columnWidths` 状态（`Record<ColumnKey, number>`，默认 132 / 168 / 104 / 104 / 232 / 124），`gridTemplateColumns` 与行 `minWidth`（`tableWidth` 求和）由它派生：前五列固定 `px`，末列（结果）为 `minmax(默认值, 1fr)` 吸收剩余宽度，容器宽于表格时不留空、窄于表格时横向滚动而非压缩列。每个非末列单元格带 `borderInlineEnd` 分界线，表头右侧叠一个 8px 透明热区（`role="separator"`、`cursor: col-resize`）：按下后用 `pointermove` 计算 `startWidth + Δx` 并夹取到 `[64, 420]`，拖动期间 `body` 置 `col-resize` + 禁选中，`pointerup` / `pointercancel` 收尾；双击热区恢复该列默认宽度。列宽不落库（仅面板生命周期内生效），未引入第三方 resizable 依赖

**依赖**：P4（命令与 DTO）、P5.1（outcome 字典）

---

## 验收矩阵

| 规格条款 | 任务 | 验收方式 |
| --- | --- | --- |
| §2.3 G1 `vault_unlock_failed` | V1.1 | JSONL 落行断言 / 手工错误口令 |
| §2.3 G2 死事件删除 | V1.2 | 编译通过 |
| §2.3 G3 同步覆盖审计 | V1.3 | 导入覆盖用例断言 `vault_entry_updated` |
| §2.3 G5 锁库批量移除 | V1.4 | 锁库移除 N 个密钥 → JSONL N 行、补录后 DB N 行（依赖 V2.3） |
| §2.5 G4 白名单自洽 | V1.5 | J 集合 ⊆ 白名单 枚举测试 |
| §2.5 G11 游标推进 | V2.1 | 游标回拨测试 + 重复解锁不重复 |
| §2.5 坏行容错 | V2.2 | 混合行用例 |
| §2.5 R3 同毫秒不丢行 | V2.3 | 同毫秒 2 条同事件 → 2 行 |
| §2.5 R6 事务与裁剪 | V2.4 | 重建后不重扫 + 事务断言 |
| §2.5 R1 来源标注 | V2.5 / P3.1 | `jsonl-` 前缀可辨识 |
| §2.5 R11 恢复对齐 | V2.6 | 游标对齐或文档声明 |
| §2.7 R12 自回响 | V4.1 | DEK 轮换后 `dek_rotated` 恰好 1 行、重建后 `local_db_rebuilt` 恰好 1 行 |
| §2.4 R13–R15 文法收敛 | V4.2 | `dek_rotated` 三段 + 批量 `@remote` + 消歧解析 |
| §2.3 R16 / R17 语义与通道 | V4.3 | 文案约束 + 通道标注二选一 |
| §4 R18 / R19 / R21 级别与类型 | V4.4 | 审计失败仍成功 + `warn` 级别 + 审计目录不可写时 DB 通道仍 `Ok` + `log_decision` 注入失败时工具仍返回正常结果 |
| §2.7.2 三张表一致性 | V4.5 | `Display` 唯一 + 文法枚举测试 |
| §2.6 O6 查询参数化 | V3.1 / V3.2 | 命令层参数透传 + 契约生成 |
| §2.6 R20 排序稳定 | V3.1 | 同毫秒多条 `query_log` 顺序稳定 |
| §2.4 O5 detail 收敛 | V3.3 | 摘要不含原文 |
| §2.4 R22 / G14 入参校验前置 | V3.3 | 限流分支畸形入参 → `detail` 为 `unknown`，不含原串 |
| §3.1 outcome 词表 | T1.1 | 落库串断言 |
| §3.2 G7 决策点覆盖 | T2.1 / T2.2 | 桩工具用例 |
| §3.3 G6 短路可审计 | T2.3 | `ask_user` / `fs_link check` 用例 |
| §3.2 分支全覆盖 | T2.4 | 表驱动用例 |
| §3.4 G6 只读 intent | T3.1 / T3.2 | 每工具 intent 断言 |
| §3.5 G8 ToolBox | T4.1 | 主库多一行 `bypassed` |
| §5 G12 JSONL 上限 | T5.1 | 日文件保留用例 |
| §5 O3 索引 | T5.2 | `EXPLAIN QUERY PLAN` |
| §2.3 F5 锁定归因 | P1.1 | `autoLock` → `auto_lock` 全链断言 |
| §3.6 F3 授权管理审计 | P2.1 | 三命令各 1 行 `tool = grant` |
| §2.6 F1 vault 面板 | P3.1 | 锁定态禁用 + 筛选 + 上限 |
| §3.7 F2 权限查询命令 | P4.1 | 命令注册 + `limit` 夹取 + 前端字面量断言 |
| §3.7 翻页语义 | P4.1 / P5.3 | `cargo test -p assistant-app --lib cmd::permission`（`has_more_distinguishes_exact_fit_from_truncation` / `has_more_survives_max_limit` / `offset_pages_without_overlap` / `until_freezes_the_window` / `offset_is_clamped_to_zero`）+ 手工翻页 |
| §6 F7 词表映射 | P5.1 / P5.2 | 字典全集一致 + 未知值兜底 |
| §6 前端类型检查 | V3.1 / V3.2 / P1.1 / P4.1 / P5.1 | `npm run typecheck` 通过（每次契约生成后各跑一次） |
| §6 F8 命名隔离 | P3.1 / P5.3 | i18n 键为 `security_log`，不复用 `hibp.audit_all` |
| §6 统计口径 | P5.3 | 排除管理行与 ToolBox 行 |

**命令**（cwd = `src-tauri`）：

```
cargo test -p assistant-vault audit
cargo test -p assistant-tools gateway
cargo test -p assistant-storage
cargo test --test assembly_contract
```

前端（V3.1 / V3.2 / P1.1 / P4.1 均会触发契约生成，按落地顺序各自跑一次）：

```
npm run contract:gen
npm run typecheck
npx vitest run src/services/__tests__/commandNames.test.ts
```

---

## 手工验收（无法自动化）

> 进度：11 条中已通过 2 条（2026-11-15 记录，均与审计日志可见性相关）；其余 9 条待人工执行，未勾选不代表代码缺失。

- [ ] 连续输错口令 3 次 → `<data>/security/audit/` 当日 JSONL 出现 3 行 `vault_unlock_failed`，解锁后加密库补入 3 行
- [ ] 锁库后检查 SSH agent 已清空 → JSONL 出现对应条数的 `ssh_key_removed`，下次解锁补入
- [ ] ToolBox 中执行一次 `delete` → 主库 `permission_audit` 出现 1 行 `toolbox` / `bypassed`
- [ ] 审批超时与用户拒绝各一次 → `permission_audit` 分别为 `timedOut` / `rejected`，不混用
- [ ] 静置到自动锁定触发 → `security_audit_log` 的 `vault_lock` detail 为 `auto_lock`；手动点锁定为 `user`
- [ ] 撤销一条持久授权 → 主库 `permission_audit` 出现 1 行 `grant` / `grant:revoke` / `allowed`；清空全部为 `grant:clear`
- [x] 解锁态打开 vault 设置页「安全日志」tab → 能看到最近事件；锁库后该 tab 显示解锁提示且不报错
  - 人工验收通过（2026-11-15）：抽屉与 `/settings/vault` 两处入口表现一致；锁库态只显示解锁提示，`locked` 未以异常弹窗形式出现（OF3）
- [x] 同一 tab 的事件名与时间均为可读文案（非原始 snake 串），筛选事件类型生效
  - 人工验收通过（2026-11-15）：事件名走 P5.1 字典映射、时间为本地可读格式；事件类型多选筛选生效
- [ ] 解锁 → 锁库 → 再解锁（3 个循环）→ `security_audit_log` 的 `vault_unlock` 行数等于循环次数，无 `jsonl-` 前缀的重复行（R12 自回响）
- [ ] 同步删除 2 条远端条目 → 两条 `vault_entry_deleted` 的 detail 分别含各自条目 ID 且以 `@remote` 结尾（R15）
- [ ] 执行一次主密码变更并轮换 DEK → `dek_rotated` 的 detail 为 `旧版本:新版本:条数` 三段，无空格与 `->`（R14）