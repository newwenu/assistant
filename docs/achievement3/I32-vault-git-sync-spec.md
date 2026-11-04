# I32 保险库 Git 同步：语义修正与远程感知规格

| 项 | 内容 |
| --- | --- |
| 文档状态 | 已实施（规格与代码同步落地，测试通过） |
| 适用范围 | 保险库 Git 同步的状态语义、远程比对、单独 pull/push 行为、错误分类、UI 文案 |
| 依据 | 对当前主干代码的逐文件审计，非历史存档 |
| 取代 | `docs/achievement2/I9-vault-spec-4-git-sync.md` 中与本文件冲突的部分（该目录为存档，不再修改） |
| 后端入口 | `src-tauri/crates/assistant-sync`、`src-tauri/crates/assistant-vault`、`src-tauri/src/cmd/vault` |
| 前端入口 | `src/vault/core`、`src/vault/settings/VaultSyncSettings.tsx`、`src/vault/VaultStatusBar.tsx` |

---

## 1. 问题陈述

当前实现能完成"拉取—导入—导出—提交—推送"，但**状态表达**与**远程认知**是缺失的，导致多处用户可见的误导：

| # | 现象 | 代码事实 | 用户误解 |
| --- | --- | --- | --- |
| P1 | 状态栏显示"本地改动已提交"、设置页显示"本地改动已全部提交（未与远程比对）" | `Readyness::Ready` 仅由 `is_init()` + `git_state()==Clean` + `!git_has_changes()` 推导，全程不触碰网络 | 以为"已经和远程一致了" |
| P2 | 没有任何 ahead/behind 展示 | 代码中不存在 `rev-list --left-right --count` 之类调用 | 不知道自己少推了几个提交、远程多了几个提交 |
| P3 | 未配置远程时，状态仍显示"同步就绪" | `Readyness` 没有"已初始化但无远程"这一态 | 以为同步可用，点同步后只得到一条 info 提示 |
| P4 | 首次绑定远程后直接点「拉取」/「推送」会失败 | 只有 `prepare()`（被"同步"和"确认删除"调用）会做上游引导；`pull()`/`push()` 不会 | 认为是网络或凭据问题 |
| P5 | 「推送」不包含未提交的本地改动 | `SyncEngine::push()` 仅 `git.push(None, None)` | 以为点了推送就把当前数据传上去了 |
| P6 | 网络错误、认证失败都被报成"同步冲突" | `pull()` 把所有 `SyncError::GitStatus(_)` 一律转成 `SyncResult::Conflict` | 被引导去做无意义的冲突解决 |
| P7 | 失败时看不到 Git 的真实原因 | `SyncError::GitStatus(i32)` 只带退出码，`git()` 使用 `.status()` 不捕获 stderr | 只能看到"退出码 128" |

本规格的目标是**用最小改动消除 P1–P7**，并且不改变既有加密格式、不引入三方合并 UI、不改动 `SyncActionDto` 的既有四个动作。

---

## 2. 术语与数据流

```
         ┌──────────────────────────── 本地（离线可判） ───────────────────────────┐
条目变更 → │ DB(SQLite) → export_all_entries() → 工作区文件 → commit → 本地分支 HEAD │
         └──────────────────────────────────────────────────────────────────────┘
                                      │
                         git fetch / pull / push（联网，可能失败）
                                      │
         ┌──────────────────────────── 远程 ──────────────────────────────────────┐
         │ origin/<branch> 引用（本地已有副本） → 与 HEAD 比对可得 ahead/behind     │
         └──────────────────────────────────────────────────────────────────────┘
```

- **本地就绪（Readyness）**：只看本地，永远不联网。回答"现在能不能开始同步操作"。
- **远程状态（RemoteStatus，本规格新增）**：回答"我比远程多几个、少几个提交"。需要联网刷新引用，也可基于上次拉取结果离线计算。
- **同步时间（sync_last_at）**：上一次 `pull`/`push`/`sync`/`forcePush` 成功的时间，存于 `VaultConf`。

关键结论：**Readyness 永远不能表达"与远程一致"**。要表达"一致"，必须依赖 RemoteStatus 且 `ahead==0 && behind==0`。

---

## 3. 设计目标 / 非目标

### 3.1 目标

1. Readyness 增加 `noRemote` 态，使"未绑定远程"成为一等状态（P3）。
2. 新增远程状态查询能力与 IPC 命令，输出 `ahead`/`behind`（P2）。
3. 状态栏与设置页文案由"本地干净"改为精确表述；"已与远程一致"仅在 RemoteStatus 确认后出现（P1）。
4. 单独「推送」补齐"导出 → 提交 → 上游引导 → 推送"（P4、P5）。
5. 单独「拉取」补齐上游引导（P4）。
6. 按"是否真的存在冲突"分类错误，并携带 Git stderr（P6、P7）。
7. 「同步」在无远程时也完成本地提交，返回 `NoRemote`（消除"同步后仓库仍然脏"）。

### 3.2 非目标

- 不实现三方合并、不实现行级冲突编辑器（保持现有"整文件选本地/选远程"）。
- 不引入 Git 库（`git2`），继续走 Git CLI。
- 不改动 `.enc` 文件格式、`vault_meta`、主密钥派生。
- 不做后台定时 fetch、不做推送通知。
- 不新增"仅 add+commit"的独立按钮（见 D4 的否决说明）。
- 不改变 `SyncActionDto` 的四个既有变体名称。

---

## 4. 状态模型

### 4.1 本地就绪 Readyness（扩展一态）

定义位置：`src-tauri/crates/assistant-sync/src/types/mod.rs`

```rust
pub enum Readyness {
    NoSync,                 // 仓库未初始化
    RepoState(RepositoryState), // 存在 merge/rebase/cherry-pick 等中间态
    NoRemote,               // 【新增】已初始化，但一个 remote 都没有
    Dirty,                  // 有未提交改动
    Ready,                  // 本地干净且已绑定远程
}
```

判定顺序（`GitSync::readyness()`）：

```
1. !is_init()                      → NoSync
2. git_state() != Clean            → RepoState(state)
3. !has_remote()                   → NoRemote
4. git_has_changes()               → Dirty
5. 否则                             → Ready
```

设计取舍：

- **`NoRemote` 排在 `Dirty` 之前**。无远程时"待推送"这个概念本身不成立，先把配置缺口暴露出来比显示"有未提交改动"更有行动指向；且条目变更本来就会自动导出并提交，"脏"在无远程场景下是瞬态。
- `has_remote()` 只执行本地 `git remote`，无网络开销，可在每次条目变更后安全调用。
- **不新增"有远程但无上游"的态**：该缺口由 `prepare()` / 新增的 `ensure_upstream()` 自动修复，不需要用户决策。

序列化：`ReadynessDto` 新增 `NoRemote` → 线格式 `"noRemote"`。

### 4.2 远程状态 RemoteStatus（新增）

定义位置：`src-tauri/crates/assistant-sync/src/types/mod.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatus {
    pub upstream: Option<String>, // 例 "origin/main"；无上游时为 None
    pub ahead: Option<u32>,       // 本地领先远程的提交数；未知为 None
    pub behind: Option<u32>,      // 远程领先本地的提交数；未知为 None
    pub fetched: bool,            // 本次是否真的成功刷新了引用
    pub checked_at: i64,          // 计算时刻（Unix 毫秒）
    pub error_kind: Option<RemoteStatusErrorKind>,
    pub error_detail: Option<String>, // 脱敏、截断后的 Git stderr 或内部错误摘要
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteStatusErrorKind {
    NoRemote,
    NoUpstream,
    FetchFailed,
    CompareFailed,
}
```

设计取舍：

- **`ahead`/`behind` 用 `Option` 而不是 `0`**：未知与"确认为 0"必须可区分，否则 UI 会再次撒谎。
- **失败不抛错**：命令返回 `Ok(RemoteStatus)`，把原因放进 `error_kind` / `error_detail`。这样 UI 可以保留上一次结果并降级展示，而不是整块报错。
- **错误分两层**：`error_kind` 是稳定枚举，供前端本地化；`error_detail` 只作为 Tooltip 的诊断细节，**永不与标题拼接后直接展示**。
- **`error_detail` 必须脱敏**：与 `GitFailed` 共用 `sanitize_git_stderr()`，移除 URL userinfo、把仓库绝对路径替换为 `<vault>`，并截断到 2000 字符。保险库场景不得把带凭据的远程 URL 或本机路径透到 UI。
- `fetched=false` 表示"本次没有成功刷新引用"：可能是 `fetch=false` 的离线比对，也可能是 fetch 失败。UI 必须能区分"离线结果"与"检查失败"。
- `checked_at` 是**计算时刻**，不是引用更新时间。文案必须写"离线比对于 {{time}}，未联网核对"，不能写成"基于 {{time}} 的拉取结果"。

计算方式（`GitSync::remote_status(fetch: bool)`）：

```
1. !is_init()                → Err(NotInitialized)
2. !has_remote()             → Ok(error_kind=NoRemote)
3. upstream = branch_upstream("HEAD")
     若 None 且 fetch=true  → ensure_upstream()
         返回 true（已 fetch）→ fetched=true，重查 upstream
         返回 false（未 fetch）→ 重查 upstream
         返回 Err          → Ok(error_kind=FetchFailed, error_detail=sanitize(e))
     若仍 None              → Ok(error_kind=NoUpstream)
4. fetch=true 且上一步未执行 fetch → git fetch
     成功 → fetched=true；失败 → error_kind=FetchFailed, error_detail=sanitize(e)
5. ahead/behind = git rev-list --left-right --count HEAD...<upstream>
     失败 → error_kind 保持已有值，否则为 CompareFailed；计数保持 None
6. checked_at = now_ms()
```

补充：除 `NotInitialized` 外的内部错误（`has_remote` / `branch_upstream` 失败等）也必须降级为 `Ok(error_kind=CompareFailed)`，不得从命令抛出。

### 4.3 展示态派生（前端纯函数）

新增 `src/vault/core/syncDisplay.ts`，把 `(Readyness, RemoteStatus, syncError)` 归一成一个展示态，供状态栏与设置页共用，避免两处文案再次分叉。

```ts
export type SyncDisplay =
  | { kind: "unknown" }
  | { kind: "failed"; message: string }
  | { kind: "repoState"; state: RepositoryState }
  | { kind: "noSync" }
  | { kind: "noRemote" }
  | { kind: "dirty" }
  | { kind: "ahead"; ahead: number }
  | { kind: "behind"; behind: number }
  | { kind: "diverged"; ahead: number; behind: number }
  | { kind: "inSync"; checkedAt: number; fetched: boolean }
  | { kind: "readyUnchecked"; reason: "notChecked" | "checkFailed" };

export function deriveSyncDisplay(input: {
  readyness: Readyness | null;
  remoteStatus: RemoteStatus | null;
  syncError: string | null;
}): SyncDisplay;
```

优先级（自高到低）：

| 序 | 条件 | 展示态 | 说明 |
| --- | --- | --- | --- |
| 1 | `syncError !== null` | `failed` | 失败信息优先于一切 |
| 2 | `readyness === null` | `unknown` | 尚未取到状态 |
| 3 | `readyness` 为对象 | `repoState` | merge/rebase 中间态 |
| 4 | `readyness === "noSync"` | `noSync` | |
| 5 | `readyness === "noRemote"` | `noRemote` | 新增 |
| 6 | `readyness === "dirty"` | `dirty` | |
| 7 | `ahead===0 && behind===0` 且 `errorKind` 不是 `fetchFailed`/`compareFailed` | `inSync` | **唯一可以说"已与远程一致"的态** |
| 8 | `ahead>0 && behind===0` | `ahead` | |
| 9 | `ahead===0 && behind>0` | `behind` | |
| 10 | `ahead>0 && behind>0` | `diverged` | |
| 11 | 其余（无 RemoteStatus 或计数为 None） | `readyUnchecked` | `reason` 由 `remoteStatus?.errorKind` 决定 |

补充规则：

- `remoteStatus.errorKind` 存在但计数已算出（fetch 失败但本地引用可用）时，**计数照常展示**，error 只进 Tooltip；但 `ahead==0 && behind==0` 且 `errorKind` 为 `fetchFailed`/`compareFailed` 时**不得**派生 `inSync`，必须降级为 `readyUnchecked(checkFailed)`，否则又会声称"已与远程一致"。
- `readyUnchecked` 的 `reason`：`remoteStatus === null` → `"notChecked"`；`remoteStatus.errorKind !== null` 且计数为 `None`，或计数为 `0/0` 但 `errorKind` 属于 fetch/compare 失败 → `"checkFailed"`。
- 派生态只依赖 `readyness`/`remoteStatus`/`syncError` 三个输入，不读 store 其余字段，便于纯函数测试。
- `syncing === true` 不参与派生，由调用方叠加"同步中"的 Loading 态。

### 4.4 刷新时机

| 触发 | 动作 | 是否联网 |
| --- | --- | --- |
| 进入保险库列表页 | `loadSyncReadyness()` + `loadSyncLastTime()` | 否 |
| 进入列表页 | **不自动** `loadRemoteStatus(true)` | — |
| 条目增删改后 | `loadSyncReadyness()` | 否 |
| `pull` / `push` / `sync` / `forcePush` / `confirmRemovals` 成功后 | `loadSyncReadyness()` + `loadRemoteStatus(false)` | 否（刚拉/推过，引用新鲜） |
| 同步动作开始时 | `set({ syncError: null })`，避免重试期间仍显示上一次失败 | 否 |
| 用户点「检查远程」 | `loadRemoteStatus(true)` | 是 |
| `gitInit` / `gitClone` 成功后 | `loadSyncReadyness()` + `loadSyncConfig()` + `loadRemoteStatus(false)` | 否 |
| `gitAddRemote` / `gitSetRemote` / `saveSyncConfig` 成功后 | `loadSyncReadyness()` + `loadRemoteStatus(false)` | 否 |
| `gitRemoveRemote` 成功后 | `loadSyncReadyness()` + `loadRemoteStatus(false)` | 否 |

理由：`pull`/`push` 成功后本地 `origin/<branch>` 引用已是最新，`fetch=false` 即可得到准确的 ahead/behind，**零额外网络成本**就把"已与远程一致"这件事说清楚。进入页面自动 fetch 会让打开保险库变慢且可能触发凭据问题，故只在显式点击时联网。远程配置的增删改会改变 `has_remote` / 上游，必须同步刷新 `RemoteStatus`，否则解绑远程后仍会显示旧的 ahead/behind。

---

## 5. 关键决策

### D1 Readyness 语义收敛：只描述本地，新增 `NoRemote`

- **现状**：`Ready` 被文案包装成"同步就绪/已同步"。
- **方案**：`Ready` 的官方语义固定为"**本地仓库干净、已绑定远程、可以开始同步**"；新增 `NoRemote` 覆盖"已初始化但无远程"。
- **理由**：语义边界清晰，且不联网，代价可忽略。
- **否决**：在 Readyness 里加 `ahead/behind` —— 会让一个高频调用的命令变成联网命令。

### D2 Git 失败携带 stderr，错误按"是否有冲突"分类

- **现状**：`git()` 用 `.status()`，丢弃输出；`SyncError::GitStatus(i32)` 只有码；`pull()` 把所有非零退出都当冲突。
- **方案**：
  1. `SyncError::GitStatus(i32)` 替换为
     ```rust
     #[error("Git operation failed (exit {code}): {stderr}")]
     GitFailed { code: i32, stderr: String },
     ```
     新增 `git::sanitize_git_stderr(repo, raw) -> String`：`trim` → URL userinfo 脱敏（`scheme://user:pass@host` → `scheme://***@host`）→ 仓库绝对路径替换为 `<vault>` → 截断到 2000 字符。`git()` 改用 `.output()`，`cmd_assert_status` 改为接收 `&Output` 与 `repo`，失败时统一调用脱敏函数。注意：`git()` 原先继承 stdio，改为捕获后 stdout/stderr 不再上屏，现有命令均已带 `-q`，无行为差异。
  2. `pull()` / `sync_now()` 的失败分支改为：
     ```rust
     Err(e) => {
         let conflicts = collect_conflicts(&git);
         let blocked = !conflicts.is_empty() || !matches!(git.state()?, RepositoryState::Clean);
         if !blocked { return Err(e); }
         attach_conflict_titles(security_db, &mut conflicts);
         return Ok(SyncResult::Conflict(conflicts));
     }
     ```
- **理由**：`git pull` 的合并冲突必然留下 unmerged 路径；"本地改动会被覆盖""认证失败""无法解析主机"都不会留下 unmerged 路径，可稳定区分，不依赖 stderr 关键字匹配。
- **否决**：解析 stderr 里的 `CONFLICT` 字符串 —— 受 Git 版本与本地化影响。

### D3 新增 `vault_sync_remote_status`

- **现状**：前端完全不知道远程状态。
- **方案**：新命令 + 新 DTO，见 §6。错误用 `error_kind` 枚举 + 脱敏后的 `error_detail`，不再直接透出英文短语。
- **理由**：`RemoteStatus` 只有后端能算（需要 Git 引用）；把它做成只读查询，不污染 `vault_sync` 的返回契约。
- **否决**：把 `ahead/behind` 塞进 `SyncResult` —— 只在同步完成时才有值，覆盖不到"刚打开页面"的场景。

### D4 单独「推送」= 导出 + `finalize()`

- **现状**：`push()` 只 `git.push(None, None)`，未提交改动不会被推送；无上游直接失败。
- **方案**：复用已有的 `GitSync::finalize()`——它本身已经包含"有改动则提交 + 上游引导 + 推送"，只是从未被 `push()` 调用：
  ```rust
  pub fn push(&self, security_db: &SecurityDbState) -> Result<SyncResult, SyncError> {
      if let Some(pending) = self.pending_removals() { return Ok(SyncResult::PendingRemovals(pending)); }
      {
          let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());
          if !git.is_init() { return Err(SyncError::NotInitialized); }
          if !git.has_remote()? { return Err(SyncError::NoRemoteConfigured); }
      } // 锁在此释放，导出不占 Git 锁
      self.export_all_entries(security_db, false, &[])?; // 先落盘，再提交
      self.finalize("vault: sync")?;
      Ok(SyncResult::Success)
  }
  ```
  `finalize()` 现有行为（无需改动）：`git_has_changes()` → `commit_all(msg)` → 无远程或 `safe_need_to_push()` 为假则提前返回 → 无上游且仅一个远程时 fetch 并取同名远端分支 → 有则 `git push`（无则带 `--set-upstream`）。
- **理由**：`finalize` 已实现"提交 + 引导 + 推送"的全部语义，`push()` 缺的只是"先导出、再走 finalize"。新增 `push_now`/`push_target` 会造成两份引导逻辑，故不采纳。
- **否决**：新增独立「提交」按钮 —— 提交是同步的内部步骤，单独暴露会让用户面对"提交了但没推"的中间态，收益低、概念负担高。
- **风险与已知行为**：`finalize` 的 `safe_need_to_push()` 有 30 秒时间窗优化（`GIT_PULL_OUTDATED`），在刚同步过且 HEAD 未变时会跳过实际推送——此时确实无可推送内容，语义正确。
- **补充**：`force_push()` 无法复用 `finalize`（`finalize` 走普通 push）。新增 `GitSync::finalize_force(msg)`：
  ```
  1. git_has_changes() → commit_all(msg, false)
  2. 无远程 → 直接返回 Ok（调用方已先判 has_remote 并报错）
  3. 无上游且仅一个远程 → ensure_upstream()（见 D5）
  4. git::git(repo, ["push", "-q", "--force"], true)
  ```
  无上游且多远程 → `Err(NoUpstreamConfigured)`。

### D5 单独「拉取」补上游引导

- **方案**：`pull()` 在 `has_remote()` 通过后、`git.pull()` 之前调用 `git.ensure_upstream()`。
  `ensure_upstream()` 由现有 `prepare()` 中的引导逻辑提取而成：
  ```rust
  /// 尝试自动建立上游。返回 Ok(true) 表示过程中执行了一次 fetch，
  /// 调用方（remote_status）可据此跳过重复 fetch；条件不满足时静默返回 Ok(false)。
  pub fn ensure_upstream(&self) -> Result<bool, SyncError> {
      if !self.is_init() || !self.has_remote()? { return Ok(false); }
      if git::git_branch_upstream(&self.repo_path, "HEAD")?.is_some() { return Ok(false); }
      let remotes = self.tracked_remote_or_remotes()?;
      if remotes.len() != 1 { return Ok(false); }
      let remote = &remotes[0];
      git::git_fetch(&self.repo_path, Some(remote))?;
      let remote_branches = git::git_branch_remote(&self.repo_path)?;
      if remote_branches.is_empty() { return Ok(true); }
      let branch = git::git_current_branch(&self.repo_path)?;
      let upstream_ref = format!("{remote}/{branch}");
      if !remote_branches.contains(&upstream_ref) { return Ok(true); }
      git::git_branch_set_upstream(&self.repo_path, None, &upstream_ref)?;
      Ok(true)
  }
  ```
  同时 `prepare()` 简化为：
  ```rust
  pub fn prepare(&self) -> Result<(), SyncError> {
      if !self.is_init() || !self.has_remote()? { return Ok(()); }
      self.ensure_upstream()?; // bool 返回值在此不需要
      self.pull()
  }
  ```
- **报错归属**：`ensure_upstream()` 自身从不因"多远程"报错；只有 `finalize_force()` 在引导后仍无上游时返回 `Err(NoUpstreamConfigured)`。`pull()` 在同样情况下不报错，而是由 `git pull` 自身的失败（`GitFailed`）带出真实原因（`fatal: no upstream configured`）。
- **理由**：消除"同步能用、拉取不能用"的行为不一致；引导逻辑只有一份实现。

### D6 文案：禁止在无远程比对时声称"一致"

- **现状**：`status_ready = 本地改动已全部提交（未与远程比对）`、`state_ready = 本地改动已提交`。
- **方案**：见 §8 文案矩阵。核心原则：
  - `Ready` + 无 RemoteStatus → **"本地工作区干净（未检查远程）"**（只陈述本地事实；不得写"无待推送改动"，否则会被读成 `ahead==0`）。
  - `Ready` + `ahead==0 && behind==0` → **"已与远程一致"**，并附检查时间。
  - `readyUnchecked` 使用中性色 `syncOff`，不得复用绿色 `syncReady`，避免视觉上再次暗示"已同步"。
  - 永不再出现"同步就绪/已同步"这类可被读成"与远程一致"的裸词。

### D7 「同步」在无远程时也提交

- **现状**：`sync_now()` 在 `!has_remote` 分支只 `export_all_entries()` 然后返回 `NoRemote`，工作区保持脏。
- **方案**：该分支改为 `export_all_entries()` → `finalize("vault: sync")` → 返回 `NoRemote`（`finalize` 内部已判断无远程则跳过推送）。
- **理由**：让"同步"在没有远程时仍有确定结果（本地已提交），与 `Ready` 的定义自洽；否则用户看到"同步完成"但状态仍显示脏。
- **连带**：`result_no_remote` 文案需改为"未绑定远程仓库：已提交本地改动，未推送"。

### D8 新增错误码 `noUpstream`

- **现状**：错误码到文案是固定映射（`SYNC_ERROR_KEY_MAP`），复用 `noRemoteConfigured` 会显示"未配置远程仓库地址"，与真实原因不符。
- **方案**：`VaultErrorCode` 增加 `NoUpstream`，映射 `SyncError::NoUpstreamConfigured`（`#[error("No upstream branch configured")]`），文案"未配置上游分支，请先执行一次「同步」。若已配置多个远程仓库，请在设置中确认上游。"（多远程场景下"执行一次同步"不足以修复，文案必须覆盖）。
- **影响面**：`VAULT_ERROR_CODES` 数组、`SYNC_ERROR_KEY_MAP`、i18n 各语言各一条。`is_retryable` 不变（不重试）。

---

## 6. 接口契约

### 6.1 域类型（`assistant-sync`）

| 类型 | 文件 | 变更 |
| --- | --- | --- |
| `SyncError` | `src/types/mod.rs` | `GitStatus(i32)` → `GitFailed{code,stderr}`；新增 `NoUpstreamConfigured` |
| `Readyness` | `src/types/mod.rs` | 新增 `NoRemote` |
| `RemoteStatus` | `src/types/mod.rs` | 新增（见 §4.2） |

### 6.2 DTO（`assistant-vault`，ts-rs 生成）

`src/dto/sync.rs` 新增：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub enum RemoteStatusErrorKindDto {
    NoRemote,
    NoUpstream,
    FetchFailed,
    CompareFailed,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct RemoteStatusDto {
    pub upstream: Option<String>,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub fetched: bool,
    pub checked_at: i64,
    pub error_kind: Option<RemoteStatusErrorKindDto>,
    pub error_detail: Option<String>,
}
```

`src/dto/convert.rs` 增加 `impl From<RemoteStatusErrorKind> for RemoteStatusErrorKindDto` 与 `impl From<RemoteStatus> for RemoteStatusDto`。

说明：

- `error_kind` 是唯一可被 UI 直接用于选择文案的字段（如 `noRemote` / `noUpstream`），保证本地化且不受 Git 版本语言影响。
- `error_detail` 只放脱敏后的诊断信息，**只能出现在 Tooltip / 折叠详情中**，不得与标题拼接成主文案。
- 这两个字段都保留底层真相但不失真：UI 不再出现"远程检查失败：no upstream"这种中英混杂文案。

生成产物 `src/vault/core/contract/vault.contract.ts` 由 `npm run contract:gen` 产出，**不得手改**。

### 6.3 IPC 命令（`src-tauri/src/cmd/vault/sync.rs`）

```rust
// precondition: AnyState
#[command]
pub async fn vault_sync_remote_status(
    app: AppHandle,
    fetch: Option<bool>,
) -> Result<RemoteStatusDto, VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let engine = super::sync_state(&app)?;
        let status = engine
            .remote_status(fetch.unwrap_or(false))
            .map_err(VaultErrorDto::from)?;
        Ok(RemoteStatusDto::from(status))
    })
    .await
    .map_err(|e| super::internal_error(format!("Vault remote status task join failed: {}", e)))?
}
```

注册位置：`src-tauri/src/vault_plugin.rs` 的 `invoke_handler` 列表（与 `vault_sync_readyness` 同批）。

契约要点：

- 只有 `NotInitialized` 会返回 `Err`；其余情况一律 `Ok`，用 `error_kind` / `error_detail` 表达原因。
- 命令是只读的（`fetch=true` 仅更新引用，不改工作区）。
- **前置条件必须是 `AnyState`**：不需要解锁，也不访问 DB，故不得调用 `with_unlocked`。`crates/assistant-vault/tests/command_gate.rs` 会校验 `// precondition:` 标记与 `with_unlocked`/`get_conn`/`sync_state` 的一致性，漏写标记会直接失败。
- `fetch=true` 含网络 I/O，故用 `async` + `spawn_blocking`，与 `vault_sync` 保持同一模式。

### 6.4 `vault_sync` 动作语义（变更后）

| action | 前置条件 | 行为 | 可能返回 |
| --- | --- | --- | --- |
| `pull` | 已初始化 | 无远程 → `NoRemote`；有远程：`ensure_upstream` → `git pull` → 导入 → 待确认删除 | `Success` / `Conflict` / `PendingRemovals` / `UnreadableEntries` / `NoRemote` / `Err` |
| `push` | 已初始化 | 无远程 → `Err(NoRemoteConfigured)`；有远程：导出 → 提交 → 上游引导 → 推送 | `Success` / `PendingRemovals` / `Err(NoRemoteConfigured)` / `Err(NoUpstreamConfigured)` / `Err` |
| `sync` | 已初始化 | 无远程：导出+提交 → `NoRemote`；有远程：`prepare` → 导入 → 导出 → `finalize` | 全部 |
| `forcePush` | 已初始化 | 无远程 → `Err(NoRemoteConfigured)`；有远程：导出 → 提交 → 上游引导 → `push --force` | `Success` / `Err` |

`sync_last_at` 的写入规则保持不变：仅 `Success` 与 `UnreadableEntries` 视为"一次成功同步"。

### 6.5 前端服务与 Store

`src/vault/core/vault.service.ts`：

```ts
syncRemoteStatus: (fetch = false): Promise<RemoteStatus> =>
  invoke("vault_sync_remote_status", { fetch }),
```

`src/vault/core/slices/vaultSyncSlice.ts` 新增状态：

```ts
remoteStatus: RemoteStatus | null;
remoteChecking: boolean;
loadRemoteStatus: (fetch?: boolean) => Promise<void>;
```

`loadRemoteStatus` 规则：

1. `set({ remoteChecking: true })`；
2. 调用失败（`NotInitialized` 或 IPC 失败）→ `set({ remoteStatus: null })`，不污染 `error`；
3. 成功 → `set({ remoteStatus })`；
4. `finally` → `remoteChecking: false`。

各同步动作成功后追加一次 `loadRemoteStatus(false)`（不阻塞 `syncing` 复位）。同步动作开始时同时 `set({ syncError: null })`，避免重试期间派生函数仍返回上一次 `failed`。远程配置的 `gitInit` / `gitClone` / `gitAddRemote` / `gitSetRemote` / `gitRemoveRemote` / `saveSyncConfig` 成功后也必须调用 `loadRemoteStatus(false)`。

---

## 7. 实现改动清单

### 7.1 后端

| 文件 | 改动 |
| --- | --- |
| `crates/assistant-sync/src/types/mod.rs` | `SyncError` 变体替换 + 新增；`Readyness::NoRemote`；新增 `RemoteStatus` |
| `crates/assistant-sync/src/git_sync/git.rs` | `git()` 由 `.status()` 改为 `.output()`；`cmd_assert_status` 改为接收 `&Output` + `repo` 并返回脱敏后的 `GitFailed{code,stderr}`（5 处调用点同步改造）；`git_log` 内的直接构造改为 `GitFailed`；新增 `git_rev_list_left_right_count`；新增 `sanitize_git_stderr`（URL userinfo 脱敏 + 仓库路径替换 + 截断） |
| `crates/assistant-sync/src/git_sync/mod.rs` | `readyness()` 插入 `NoRemote` 分支；从 `prepare()` 提取 `ensure_upstream()`；新增 `remote_status(fetch)`、`finalize_force(msg)`、`state()` 访问器 |
| `crates/assistant-vault/src/sync_engine/pull_push.rs` | `pull` 补 `ensure_upstream` + 冲突分类；`push`/`force_push` 改签名为带 `security_db` 并走"导出→`finalize`/`finalize_force`"；`sync_now` 无远程分支补 `finalize` |
| `crates/assistant-vault/src/sync_engine/remote_admin.rs` | 新增 `pub fn remote_status(&self, fetch: bool)` 转发到 `GitSync::remote_status`（持 `git_sync` 锁） |
| `crates/assistant-vault/src/dto/sync.rs` | 新增 `RemoteStatusDto` 与 `RemoteStatusErrorKindDto` |
| `crates/assistant-vault/src/dto/error.rs` | `VaultErrorCode::NoUpstream` |
| `crates/assistant-vault/src/dto/convert.rs` | `SyncError` 映射更新；`RemoteStatusDto` 转换 |
| `src-tauri/src/cmd/vault/sync.rs` | 新增命令；`Push`/`ForcePush` 传 `db` |
| `src-tauri/src/vault_plugin.rs` | 在 `invoke_handler` 中注册新命令 |

注意：`cmd_assert_status` 当前有 5 处引用（`git_branch_upstream`、`git()`、`git_stdout_ok` 内的两处、定义本身），改为接收 `&Output` 后，原先传 `output.status` 的调用点统一改传 `&output`；`git()` 自身改用 `.output()` 并把结果交给 `cmd_assert_status`。

### 7.2 前端

| 文件 | 改动 |
| --- | --- |
| `src/vault/core/contract/vault.contract.ts` | 重新生成（勿手改） |
| `src/vault/core/vault.types.ts` | 导出 `RemoteStatus` / `RemoteStatusErrorKind` 类型 |
| `src/vault/core/vault.service.ts` | 新增 `syncRemoteStatus` |
| `src/vault/core/vaultStore.ts` / `slices/vaultSyncSlice.ts` | 新增 `remoteStatus` / `remoteChecking` / `loadRemoteStatus`；同步动作开始清 `syncError`；各动作与远程配置变更后刷新 |
| `src/vault/core/syncDisplay.ts` | **新增** 派生纯函数（§4.3） |
| `src/vault/VaultStatusBar.tsx` | 改用 `deriveSyncDisplay`；新增 `onOpenSettings?: () => void` prop，`noRemote` 时渲染为可点击；`inSync` / `readyUnchecked` Tooltip 使用检查时间或错误 kind |
| `src/vault/VaultApp.tsx` | 向 `VaultStatusBar` 传 `onOpenSettings={() => setSettingsOpen(true)}`（`VaultSettings` 默认页签已是 `sync`） |
| `src/vault/settings/VaultSyncSettings.tsx` | 改用派生态；`noRemote` 显示引导 Alert + 聚焦远程地址输入框；新增「检查远程」按钮（loading=`remoteChecking`）与时间/错误 Tooltip；推送菜单项加 Tooltip |
| `src/vault/settings/DangerZoneTab.tsx` | `hasRemote` 仅看 `syncConfig?.remoteUrl`，不再由 `!isNoSync` 推导 |
| `src/vault/core/vaultError.ts` | `VAULT_ERROR_CODES` 增加 `"noUpstream"` |
| `src/i18n/locales/zh-CN.json`、`en-US.json` | 见 §8 |

---

## 8. UI 与文案

### 8.1 展示矩阵

| 展示态 | 状态栏（`VaultStatusBar`） | 设置页 Alert | 颜色 |
| --- | --- | --- | --- |
| `failed` | `state_failed` + Tooltip 详情，点击清除 | error Alert | `leak` |
| `repoState` | `state_conflict` + Tooltip 状态名 | warning Alert + 现有操作入口 | `leak` |
| `noSync` | `state_off` | info Alert | `syncOff` |
| `noRemote` | `state_no_remote`（点击 → 打开设置抽屉的 sync 页签） | warning Alert + 「绑定远程仓库」按钮（聚焦远程地址输入框） | `syncOff` |
| `dirty` | `state_dirty` | warning Alert | `syncDirty` |
| `ahead` | `state_ahead`（count） | warning Alert | `syncDirty` |
| `behind` | `state_behind`（count） | warning Alert | `syncDirty` |
| `diverged` | `state_diverged`（ahead/behind） | warning Alert | `leak` |
| `inSync` | `state_in_sync` + Tooltip 检查时间；`fetched=false` 时用 `remote_checked_stale` | success Alert + 检查时间/stale 时间 | `syncReady` |
| `readyUnchecked` | `state_ready`（不声称一致）；检查失败时 Tooltip 显示本地化 `error_kind`，`error_detail` 只进详情 | info Alert + `remote_check_hint`；检查失败时显示 `remote_check_failed` + 折叠详情 | `syncOff`（中性色，不得用绿色） |

不新增 CSS 类：沿用 `VAULT_TONES` 已有的 `syncReady` / `syncDirty` / `syncOff` / `leak`。

`error_detail` 的展示约束：只能出现在 Tooltip、Popover 或折叠详情中；主标题必须是 `error_kind` 对应的本地化短语。`failed` 态的 `state_failed_tip` 沿用现有处理，但其中的 message 已由后端脱敏。

### 8.2 i18n 键（新增/修改）

其中 `status_ready`、`state_ready`、`result_no_remote` 是**修改既有键**（当前取值分别为"本地改动已全部提交（未与远程比对）"、"本地改动已提交"、"未配置远程仓库，本地改动已写入仓库但未提交"），其余为新增键。两个语言文件必须同步补齐。

zh-CN：

```json
"status_no_remote": "已初始化，但未绑定远程仓库",
"status_ready": "本地工作区干净（未检查远程）",
"state_no_remote": "未绑定远程",
"state_ready": "本地工作区干净",
"state_in_sync": "已与远程一致",
"state_ahead": "{{count}} 个提交待推送",
"state_behind": "远程有 {{count}} 个更新待拉取",
"state_diverged": "待推送 {{ahead}} / 待拉取 {{behind}}",
"remote_check": "检查远程",
"remote_checking": "检查中…",
"remote_checked_at": "检查于 {{time}}",
"remote_checked_stale": "离线比对于 {{time}}，未联网核对",
"remote_check_failed": "远程检查失败：{{message}}",
"remote_check_no_remote": "尚未绑定远程仓库",
"remote_check_no_upstream": "未配置上游分支",
"remote_check_hint": "点击「检查远程」与远程比对",
"remote_not_bound_tip": "当前仓库仅存在于本机，无法拉取或推送。绑定远程仓库后即可多设备同步。",
"remote_not_bound_cta": "绑定远程仓库",
"err_no_upstream": "未配置上游分支，请先执行一次「同步」。若已配置多个远程仓库，请在设置中确认上游。",
"result_no_remote": "未绑定远程仓库：已提交本地改动，未推送",
"push_hint_commits_first": "推送前会先提交本地改动"
```

en-US：

```json
"status_no_remote": "Initialized, but no remote is bound",
"status_ready": "Local working tree is clean (remote not checked)",
"state_no_remote": "No remote bound",
"state_ready": "Local working tree clean",
"state_in_sync": "In sync with remote",
"state_ahead": "{{count}} commit(s) to push",
"state_behind": "{{count}} update(s) to pull",
"state_diverged": "{{ahead}} to push / {{behind}} to pull",
"remote_check": "Check remote",
"remote_checking": "Checking…",
"remote_checked_at": "Checked at {{time}}",
"remote_checked_stale": "Compared offline at {{time}}; not verified online",
"remote_check_failed": "Remote check failed: {{message}}",
"remote_check_no_remote": "No remote is bound",
"remote_check_no_upstream": "No upstream branch configured",
"remote_check_hint": "Click \"Check remote\" to compare with the remote",
"remote_not_bound_tip": "This repository exists only on this device. Bind a remote to sync across devices.",
"remote_not_bound_cta": "Bind remote",
"err_no_upstream": "No upstream branch configured. Run \"Sync\" once first. If multiple remotes are configured, choose an upstream in Settings.",
"result_no_remote": "No remote bound: local changes committed, not pushed",
"push_hint_commits_first": "Local changes are committed before pushing"
```

`VaultStatusBar` 的 `SyncStatusKey` 联合类型需同步扩充；`getSyncStatus` 由 `deriveSyncDisplay` 取代。

---

## 9. 测试

### 9.1 后端

`crates/assistant-sync/tests/git_sync.rs`（需修改现有断言）：

| 测试 | 变更 |
| --- | --- |
| `readyness_is_no_sync_before_init_and_ready_after` | 建议改名 `readyness_is_no_sync_then_no_remote_after_init`；`init()` 后**无远程** → 期望 `NoRemote`（原 `Ready`） |
| `dirty_after_change_and_ready_after_commit` | 改为先 `remote add`：改动 → `Dirty`，提交后 → `Ready`；另新增 `dirty_without_remote_reports_no_remote` 覆盖"无远程时即使脏也报 `NoRemote`" |
| `finalize_without_remote_only_commits` | 提交后期望 `NoRemote`（原 `Ready`） |
| `push_then_clone_roundtrip` | clone 带 `origin` → 保持 `Ready`，无需改动 |

**连带影响提示**：`git.init()` 内部即产生一个初始提交且不绑定远程，因此"init 之后就是 Ready"这一假设在所有测试中都不再成立。新增 `NoRemote` 后，任何在无远程仓库上断言 `Ready` 的用例都必须改为 `NoRemote`；反之，只有显式 `add_remote_url` 的仓库才可能得到 `Ready`。

新增：

- `readyness_no_remote_then_ready_after_remote_added`：`remote add` 后 `Ready`。
- `remote_status_ahead_behind_after_divergence`：两仓库各自提交后，`ahead==1 && behind==1`。
- `remote_status_offline_uses_existing_refs`：`fetch=false` 时 `fetched==false` 且计数正确。
- `remote_status_no_upstream_reports_error`：无上游 → `ahead/behind` 为 `None`、`error_kind==NoUpstream`。
- `dirty_without_remote_reports_no_remote`：无远程时工作区脏 → 仍为 `NoRemote`（优先级高于 `Dirty`）。
- `push_bootstraps_upstream_and_commits_dirty_changes`：首次绑定远程后直接 push，既建立上游又导出/提交脏数据，远端可克隆到条目文件。
- `sync_now_without_remote_commits_local_changes`：无远程 sync → `NoRemote` 且 `Readyness::NoRemote`（工作区干净）。
- `pull_failure_is_not_conflict`：把 remote URL 改为不存在路径 → 返回 `Err(GitFailed)`，且 `VaultErrorDto.code == gitUnavailable`。
- `git_failed_carries_stderr`：断言 `SyncError::GitFailed { stderr }` 非空。
- `sanitize_git_stderr_redacts_credentials`：含 `https://user:token@host/repo.git` 的 stderr → 不含 `user` / `token`，保留 `https://***@host/repo.git`。
- `sanitize_git_stderr_replaces_repo_path`：stderr 中的仓库绝对路径被替换为 `<vault>`。
- `sanitize_git_stderr_truncates`：超长 stderr 截断到 2000 字符。

`crates/assistant-vault/tests/dto_contract.rs`：

- `readyness_dto_is_one_to_one()` 的遍历列表加入 `Readyness::NoRemote`；
- 新增 `remote_status_dto_serializes_camel_case()`（含 `errorKind` / `checkedAt`）；
- 新增 `remote_status_error_kind_wire_is_frozen()`（四个变体的 camelCase 字符串）；
- 新增 `vault_error_code_includes_no_upstream()`；
- `git_failed_maps_to_git_unavailable()`：`GitFailed` → `gitUnavailable`，`NoUpstreamConfigured` → `noUpstream`。

`crates/assistant-vault/tests/command_registry.rs`：该测试自动扫描"命令定义 ↔ 注册"双向匹配，新命令必须在 `vault_plugin.rs` 注册，否则 `vault_commands_and_registrations_match_both_ways` 直接失败（无需手写断言）。

`crates/assistant-vault/tests/command_gate.rs`：同样自动扫描，新命令必须带 `// precondition:` 标记且与实际门禁调用一致；`vault_sync_remote_status` 标 `AnyState`、不调用 `with_unlocked`。

### 9.2 前端

新增 `src/vault/core/__tests__/syncDisplay.test.ts`，覆盖 §4.3 优先级全表，至少包含：

- `Ready` + `remoteStatus === null` → `readyUnchecked(notChecked)`（**回归 P1**）；
- `Ready` + `ahead=0,behind=0` → `inSync`（携带 `checkedAt` / `fetched`）；
- `Ready` + `ahead=2` → `ahead(2)`；
- `Ready` + `behind=3` → `behind(3)`；
- `Ready` + `ahead=1,behind=1` → `diverged`；
- `Ready` + `remoteStatus.errorKind=NoUpstream` → `readyUnchecked(checkFailed)`；
- `noRemote` 优先于 `dirty`；
- `syncError` 优先于一切。

`src/vault/core/__tests__/vaultStore.test.ts`：`resetStore()` 增加 `remoteStatus: null`、`remoteChecking: false`；新增用例"push 成功后以 `fetch=false` 调用 `vault_sync_remote_status`"。

### 9.3 必跑命令

```powershell
cd src-tauri
cargo test -p assistant-sync --features git-tests --test git_sync
cargo test -p assistant-vault --features git-tests --test sync_engine
cargo test -p assistant-vault --test dto_contract
cargo test -p assistant-vault --test command_registry
cargo test -p assistant-vault --test command_gate
```

```powershell
npm run contract:gen
npm run typecheck
npm run lint
npm run test
```

（`contract:gen` 必须在 `typecheck` 之前，否则前端引用不到新类型。）

---

## 10. 验收标准

1. 全新仓库、未绑定远程：状态栏显示"未绑定远程"，设置页出现引导 Alert，且「拉取/推送」不因无上游而报"冲突"。
2. 绑定远程后首次「推送」：本地未提交改动被提交并推送成功，远端出现对应提交。
3. 两台设备各提交一次后：`vault_sync_remote_status(fetch=true)` 返回 `ahead=1, behind=1`，状态栏显示"待推送 1 / 待拉取 1"。
4. 断开网络点「检查远程」：命令仍 `Ok`，`error_kind == fetchFailed`，`ahead/behind` 保留上次可用值（若引用存在），UI 提示检查失败但不清空状态；`error_detail` 只出现在 Tooltip。
5. `remote` 指向不存在的地址后点「拉取」：报 `gitUnavailable` 且消息包含脱敏后的 Git stderr，**不出现**冲突提示。
6. 真正制造文本冲突：仍返回 `Conflict`，冲突文件列表与现有流程一致。
7. 未绑定远程点「同步」：返回 `NoRemote`，工作区干净（`Readyness == NoRemote`，而非 `Dirty`）。
8. 状态栏与设置页在任意组合下文案一致（均由 `deriveSyncDisplay` 派生）。
9. 所有既有 DTO 线格式（`Readyness` 旧四态、`SyncResult`、`ConflictFile`）保持不变；新增字段均为 `Option` / 新枚举变体。
10. 远程 URL 含凭据、或 stderr 含仓库绝对路径时：UI 与错误消息中不出现用户名、密码、token 或 `<vault>` 之外的绝对路径。

---

## 11. 实施顺序与风险

建议按依赖顺序分四个提交，每步可独立验证：

1. **错误与状态基座**：`SyncError::GitFailed`/`NoUpstreamConfigured`、`Readyness::NoRemote`、`ensure_upstream` 提取、错误码与映射。风险点：`GitFailed` 替换会触及所有 `SyncError::GitStatus` 匹配点（共 5 处），漏改会编译失败（可静态发现，安全）。
2. **远程状态能力**：`RemoteStatus` + `remote_status()` + DTO + 命令 + 契约测试。风险点：`rev-list` 在上游引用不存在时退出非零，必须降级为 `error_kind` 而非 `Err`；`has_remote` / `branch_upstream` 的异常也必须降级；除 `NotInitialized` 外命令不得返回 `Err`。
3. **pull/push/sync 行为修正**：`push(security_db)`、`force_push(security_db)`、无远程 `sync` 提交、冲突分类。风险点：`push` 从只读变写，需要 Tooltip 提示；`export_all_entries` 需在持有 `git_sync` 锁之前执行，避免锁内做 IO。
4. **前端与文案**：契约重生成、`syncDisplay`、状态栏与设置页、i18n。风险点：`Readyness` 新增枚举值后，所有对 `Readyness` 做穷举的分支都要补 `noRemote`：
   - `VaultStatusBar.getSyncStatus()` 直接由 `deriveSyncDisplay` 取代（否则 `noRemote` 会落到 `state_off`）。
   - `VaultSyncSettings.readynessText()` 增加 `noRemote` 分支，并统一改走 `deriveSyncDisplay`。
   - `DangerZoneTab`：`isNoSync` 必须保持严格等于 `"noSync"`——`noRemote` 时本地仓库仍存在，「放弃本地改动」应保持可用；`hasRemote` 改为仅看 `syncConfig?.remoteUrl`，不再用 `!isNoSync` 推导。
   - `RemoteStatus` 的 `error_kind` 必须穷举映射到本地化文案，不能让未知 kind 直接显示原始 detail。

回滚策略：`Readyness::NoRemote` 是唯一可能影响既有 UI 分支的契约变更；如需灰度，可先落地 2/3/4 步中的文案与行为，再单独引入 `noRemote`。

---

## 12. 已知遗留（不在本规格范围内）

- 冲突解决仍是整文件"选本地/选远程"，无行级合并。
- `need_to_push` 的 30 秒时间窗优化保留：`finalize` 在 30 秒内且 HEAD 未变时跳过 `git push`，因此"同步"可能不产生实际网络推送（状态正确，仅减少请求）。
- 多远程场景（`remotes.len() > 1` 且无上游）不做自动选择：`ensure_upstream()` 静默跳过，`push`/`forcePush` 由 `finalize_force` 报 `noUpstream`，`pull` 由 Git 自身报 `fatal: no upstream configured`（`gitUnavailable`）。用户需在设置页明确指定上游。
- `Readyness::NoRemote` 的引入改变了"init 后即 Ready"的既有语义，属于面向用户的状态机变更；`docs/achievement2/` 下历史文档中的相关描述保持原样（存档不改）。
- `sync_last_at` 不区分 pull/push/sync，只记录最后一次成功时间。