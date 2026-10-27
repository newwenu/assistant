# I2e Vision Resolution — 模型视觉能力解析设计

> **前置**：I2e Phase 1 ✅（图片附件完整链路）
> **问题**：当前 `infer_support_vision` 使用 `model_id.contains("4o")` 等字符串匹配，存在误判风险；Gemini/Anthropic 硬编码全 `true`，老模型被误标
> **目标**：消除所有硬编码字符串匹配，改用 **API 内省 + models.dev 目录 + 用户覆盖** 三层解析

---

## 1. 数据源分析

### 1.1 各 Provider `/models` API 能力字段

| Provider | API 端点 | 返回能力信息 | 当前项目是否解析 |
|----------|---------|-------------|----------------|
| OpenAI 官方 | `/v1/models` | ❌ 无（仅 id/owned/created） | — |
| OpenRouter | `/api/v1/models` | ✅ `architecture.modality`（如 `"text+image→text"`） | ❌ 未解析 |
| Anthropic | `/v1/models` | ❌ 无（仅 id/display_name/context） | — |
| Gemini | `/v1beta/models` | ❌ 无显式声明（仅 `supportedGenerationMethods`） | — |

**结论**：仅 OpenRouter 兼容端点返回可用的能力声明字段，其余 Provider 的官方 API 不提供。

### 1.2 models.dev 开源目录

- **来源**：[models.dev](https://models.dev/)，SST/anomalyco 维护的开源 AI 模型规格数据库
- **API**：`https://models.dev/api.json`，返回全量 provider → models 映射
- **许可**：MIT License（Copyright (c) 2025 models.dev）
- **结构**：按 provider 分组，每个模型声明 `modalities.input` / `modalities.output`
- **覆盖**：openai(48) / anthropic(14) / google(39) / deepseek(4) / alibaba(55) / openrouter(367) 等 16+ provider
- **维护**：社区驱动，新模型发布后由 PR 更新；OpenBitFun 通过 `pnpm run models-dev:update` 定期同步

**示例**（来自实际 `models-dev.json` 快照）：
```json
{
  "openai": {
    "models": {
      "gpt-4o": { "modalities": { "input": ["text", "image"], "output": ["text"] } },
      "gpt-3.5-turbo": { "modalities": { "input": ["text"], "output": ["text"] } }
    }
  },
  "anthropic": {
    "models": {
      "claude-sonnet-4-5": { "modalities": { "input": ["text", "image", "pdf"], "output": ["text"] } },
      "claude-haiku-4-5": { "modalities": { "input": ["text", "image", "pdf"], "output": ["text"] } }
    }
  },
  "google": {
    "models": {
      "gemini-2.5-pro": { "modalities": { "input": ["text", "image", "audio", "video"], "output": ["text", "image"] } },
      "gemini-2.5-flash": { "modalities": { "input": ["text", "image", "audio", "video"], "output": ["text"] } }
    }
  }
}
```

> **注意**：models.dev 仅收录当前活跃模型，已退役模型（`claude-2`、`claude-3-*`、`gemini-1.0-pro` 等）不在目录中。对这些模型，`catalog_supports_vision()` 返回 `None`，`resolve_support_vision()` 兜底为 `false`——用户可通过前端 toggle 覆盖。

### 1.3 参考项目做法

**OpenBitFun**：
- 内置 `models-dev.json` 快照（编译时 `include_str!`）
- 运行时 `ModelsDevCatalogService`：启动加载缓存 → 后台 `refresh_if_stale()` 从 `https://models.dev/api.json` 更新
- 缓存 TTL 6 小时，失败回退内置快照
- `provenance.json` 记录快照来源、SHA256、更新命令

**OpenHuman**：
- 托管模型用精确 ID 集合 `MANAGED_MULTIMODAL_MODELS` + 角色别名 `hint:vision`
- BYOK 模型用 `model_registry.vision` 用户标记
- 合并：`oh_tier_supports_vision(model) || model_vision_enabled(model, config)`

---

## 2. 设计：三层解析架构

```
resolve_support_vision(model_id, provider_kind, api_response?)
│
├─ Layer 1: API 内省（OpenRouter architecture.modality）
│  → api_response 中含 modality 字段时直接判定，零维护
│
├─ Layer 2: models.dev 目录（编译时快照 + 运行时刷新）
│  → 精确 model_id 键值查找，覆盖 OpenAI/Anthropic/Gemini 官方 API 不返回能力的场景
│
├─ Layer 3: 用户覆盖（前端 toggle）
│  → 用户在 Provider 设置页手动标记，持久化到 DB models_json
│
└─ 兜底: false（未知模型默认不支持视觉）
```

### 2.1 Layer 1 — API 内省

扩展 `ModelItem` 解析 OpenRouter 的 `architecture.modality` 字段：

```rust
// openai/types.rs
#[derive(Deserialize, Debug, Default)]
pub(crate) struct ModelArchitecture {
    #[serde(default)]
    pub modality: Option<String>,  // "text+image→text", "text→text", etc.
}

// ModelItem 新增字段
#[serde(default)]
pub architecture: Option<ModelArchitecture>,

impl ModelItem {
    pub(crate) fn resolved_support_vision(&self) -> Option<bool> {
        self.architecture.as_ref().and_then(|arch| {
            arch.modality.as_ref().map(|m| {
                m.to_ascii_lowercase().contains("image")
            })
        })
    }
}
```

**覆盖范围**：OpenRouter、SiliconFlow、NVIDIA 等兼容端点。
**零维护**：新模型自动获得正确能力。

### 2.2 Layer 2 — models.dev 目录

#### 编译时快照

从参考项目复制 `models-dev.json` + `models-dev.LICENSE.txt`，用 `include_str!` 嵌入二进制。

仅提取 `modalities.input` 字段构建精简查找表（减小内存占用）：

```rust
// assistant-providers/src/model_catalog.rs
use std::collections::HashMap;
use std::sync::LazyLock;

static BUNDLED_CATALOG_JSON: &str = include_str!("../assets/models-dev.json");

static VISION_CATALOG: LazyLock<HashMap<String, bool>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    if let Ok(root) = serde_json::from_str::<serde_json::Value>(BUNDLED_CATALOG_JSON) {
        let Some(providers) = root.as_object() else { return map; };
        for (_provider_id, provider_val) in providers {
            let Some(models) = provider_val.get("models").and_then(|v| v.as_object()) else { continue; };
            for (model_id, model_val) in models {
                let has_image = model_val
                    .get("modalities")
                    .and_then(|m| m.get("input"))
                    .and_then(|i| i.as_array())
                    .map_or(false, |arr| {
                        arr.iter().any(|v| v.as_str() == Some("image"))
                    });
                map.insert(model_id.clone(), has_image);
            }
        }
    }
    map
});

pub fn catalog_supports_vision(model_id: &str) -> Option<bool> {
    VISION_CATALOG.get(model_id).copied()
}
```

#### 运行时刷新（V2，可选）

参考 OpenBitFun 的 `ModelsDevCatalogService`：
- 启动时加载磁盘缓存 → 后台从 `https://models.dev/api.json` 刷新
- 缓存 TTL 6h，失败回退编译时快照
- V1 仅用编译时快照，V2 加运行时刷新

### 2.3 Layer 3 — 用户覆盖

当前 `ModelConfig.support_vision: bool` 已存在，前端设置页添加 vision toggle 开关即可。
用户 toggle → 修改 `models_json` → 写回 DB → 下次聊天使用覆盖值。

**Fetch Models 行为**：重新获取模型列表时，Layer 1/2 重新推断，用户覆盖丢失。这是合理的——模型列表已更新，旧覆盖可能已过时。

### 2.4 合并逻辑

```rust
// assistant-providers/src/vision.rs
pub fn resolve_support_vision(
    model_id: &str,
    api_hint: Option<bool>,
) -> bool {
    api_hint
        .or_else(|| model_catalog::catalog_supports_vision(model_id))
        .unwrap_or(false)
}
```

各 Provider discovery 调用：

```rust
// OpenAI discovery
let support_vision = resolve_support_vision(&id, m.resolved_support_vision());

// Gemini discovery — API 不返回能力，仅用目录
let support_vision = resolve_support_vision(&id, None);

// Anthropic discovery — 同上
let support_vision = resolve_support_vision(&id, None);

// Code Assist — 已知全支持，直接 true
support_vision: true,
```

---

## 3. 涉及文件变更

### 3.1 新增文件

| 文件 | 说明 |
|------|------|
| `crates/assistant-providers/assets/models-dev.json` | 从参考项目复制，models.dev 快照 |
| `crates/assistant-providers/assets/models-dev.LICENSE.txt` | MIT 许可证 |
| `crates/assistant-providers/src/model_catalog.rs` | 编译时目录加载 + 查找 |
| `crates/assistant-providers/src/vision.rs` | `resolve_support_vision` 合并逻辑 |

### 3.2 修改文件

| 文件 | 变更 |
|------|------|
| `crates/assistant-providers/src/lib.rs` | 新增 `pub mod model_catalog; pub mod vision;` |
| `crates/assistant-providers/src/openai/types.rs` | `ModelItem` 新增 `architecture` 字段 + `resolved_support_vision()` |
| `crates/assistant-providers/src/openai/discovery.rs` | 删除 `infer_support_vision`，改用 `vision::resolve_support_vision` |
| `crates/assistant-providers/src/gemini/discovery.rs` | `support_vision: true` → `resolve_support_vision(&id, None)` |
| `crates/assistant-providers/src/anthropic/discovery.rs` | `support_vision: true` → `resolve_support_vision(&id, None)` |

### 3.3 不变文件

| 文件 | 原因 |
|------|------|
| `crates/assistant-providers/src/gemini/code_assist.rs` | Code Assist 模型已知全支持视觉，保持 `true` |
| `crates/assistant-providers/src/base.rs` | `ModelConfig.support_vision: bool` 不变 |
| `src-tauri/src/cmd/chat.rs` | 运行时解析逻辑不变（从 DB 读 support_vision） |

---

## 4. models.dev 快照裁剪策略

参考项目 `models-dev.json` 约 717KB，包含 16 个 provider、760+ 模型。

当前项目仅使用 OpenAI 兼容 / Anthropic / Gemini 三种 provider，裁剪策略：

**V1：全量嵌入**（简单，约 717KB 编译时嵌入，运行时仅构建 HashMap<String, bool>）
- 优势：未来支持更多 provider 时无需更新快照
- 劣势：二进制体积增加 ~300KB（JSON 压缩后）

**V2：按需裁剪**（仅保留 openai + anthropic + google + deepseek + alibaba 五个 provider）
- 优势：体积更小
- 劣势：每次新增 provider 需重新裁剪

**选择 V1**：717KB 对桌面应用可忽略，且全量快照保证用户使用任何 OpenRouter 模型时都能查到能力。

---

## 5. 测试策略

### 5.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_known_vision_models() {
        assert_eq!(catalog_supports_vision("gpt-4o"), Some(true));
        assert_eq!(catalog_supports_vision("gpt-4o-mini"), Some(true));
        assert_eq!(catalog_supports_vision("claude-3-5-sonnet-20241022"), Some(true));
        assert_eq!(catalog_supports_vision("gemini-2.5-pro"), Some(true));
    }

    #[test]
    fn catalog_known_text_only_models() {
        assert_eq!(catalog_supports_vision("gpt-3.5-turbo"), Some(false));
        assert_eq!(catalog_supports_vision("claude-2"), Some(false));
    }

    #[test]
    fn catalog_unknown_model_returns_none() {
        assert_eq!(catalog_supports_vision("my-custom-llama-3b"), None);
    }

    #[test]
    fn resolve_prefers_api_hint() {
        assert_eq!(resolve_support_vision("gpt-4o", Some(false)), false);  // API 覆盖目录
        assert_eq!(resolve_support_vision("unknown", Some(true)), true);   // API 填补目录空白
    }

    #[test]
    fn resolve_falls_back_to_catalog() {
        assert_eq!(resolve_support_vision("gpt-4o", None), true);          // 目录命中
        assert_eq!(resolve_support_vision("unknown", None), false);        // 目录未命中，兜底 false
    }

    #[test]
    fn openrouter_modality_parsing() {
        let item: ModelItem = serde_json::from_str(r#"{"id":"x","architecture":{"modality":"text+image→text"}}"#).unwrap();
        assert_eq!(item.resolved_support_vision(), Some(true));
        
        let item: ModelItem = serde_json::from_str(r#"{"id":"y","architecture":{"modality":"text→text"}}"#).unwrap();
        assert_eq!(item.resolved_support_vision(), Some(false));
    }
}
```

### 5.2 集成验证

- Fetch Models（OpenRouter 端点）→ 验证 `architecture.modality` 被正确解析
- Fetch Models（OpenAI 官方端点）→ 验证目录回退生效
- Fetch Models（Anthropic 端点）→ 验证目录回退生效
- Fetch Models（Gemini 端点）→ 验证目录回退生效

---

## 6. 实现结果

### 6.1 已完成

| 项目 | 状态 |
|------|------|
| `model_catalog.rs` — 编译时加载 models-dev.json → `HashMap<String, bool>` | ✅ |
| `vision.rs` — `resolve_support_vision(model_id, api_hint)` 三层合并 | ✅ |
| `openai/types.rs` — `ModelArchitecture` + `resolved_support_vision()` | ✅ |
| `openai/discovery.rs` — 删除 `infer_support_vision`，改用 `resolve_support_vision` | ✅ |
| `gemini/discovery.rs` — `support_vision: true` → `resolve_support_vision(&id, None)` | ✅ |
| `anthropic/discovery.rs` — `support_vision: true` → `resolve_support_vision(&id, None)` | ✅ |
| `assets/models-dev.json` + `assets/models-dev.LICENSE.txt` — 从参考项目复制 | ✅ |
| 单元测试 10 个全通过 | ✅ |
| workspace cargo check 通过 | ✅ |

### 6.2 已知限制

1. **旧模型 ID 不在目录中**：models.dev 快照仅收录当前活跃模型。已退役模型（如 `claude-2`、`claude-3-5-sonnet-20241022`、`gemini-1.0-pro`）不在目录中，`catalog_supports_vision()` 返回 `None`，最终 `resolve_support_vision()` 兜底为 `false`。这是**正确行为**——退役模型不应被默认启用视觉，用户可通过前端 toggle 手动覆盖。

2. **目录更新频率**：V1 使用编译时快照，新模型发布后需更新 `models-dev.json` 并重新编译。V2 可加运行时刷新（参考 OpenBitFun 的 `ModelsDevCatalogService`）。

3. **OpenRouter `architecture.modality` 字段**：当前 `models-dev.json` 快照中不包含 `architecture` 字段（这是 OpenRouter `/api/v1/models` 实时响应的字段）。Layer 1 的 API 内省仅在 Fetch Models 时从实时 API 响应中解析，不依赖快照。

### 6.3 删除的代码

```rust
// 已删除：openai/discovery.rs 中的 infer_support_vision
fn infer_support_vision(model_id: &str) -> bool {
    let lower = model_id.to_ascii_lowercase();
    lower.contains("vision")
        || lower.contains("4o")           // ← 误匹配风险
        || lower.contains("gpt-4-turbo")
        || lower.contains("gpt-4o")
        || lower.contains("gpt-4.1")
        || lower.contains("deepseek-v4.1")
}
```

替代方案：精确键值查找（`HashMap::get`）+ API 内省，零误判。