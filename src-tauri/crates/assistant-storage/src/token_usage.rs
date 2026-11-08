use rusqlite::{params, Connection, OptionalExtension};

/// 一次 provider 请求的 token 用量记录（契约 §3.2）。
#[derive(Debug, Clone, Default)]
pub struct TokenUsageRecord {
    pub conversation_id: String,
    pub message_id: Option<String>,
    pub attempt_id: Option<String>,
    /// 请求发出时的活跃分支叶子；`None` = 会话未启用分支
    pub branch_leaf_id: Option<String>,
    /// `chat` | `compaction`
    pub kind: String,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    /// `None` = 未上报，不得用 0 替代
    pub cache_read_tokens: Option<u32>,
    pub cache_write_tokens: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    /// 由适配层声明的缓存能力，命中率分母要求它为真
    pub cache_capable: bool,
    /// `None` = 该请求未注入窗口
    pub context_window_tokens: Option<u32>,
    /// 含估算值
    pub estimated: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OccupancyRow {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub context_window_tokens: Option<u32>,
    pub estimated: bool,
}

/// 占用口径：最新一条 chat 请求的输入
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OccupancySummary {
    /// `None` = 该分支无记录，UI 显示「—」
    pub used_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    /// `None` = 窗口未知，不得回退硬编码常量
    pub context_window_tokens: Option<u32>,
    /// 与绝对数互斥：窗口未知时为 `None`
    pub percentage: Option<f32>,
    pub estimated: bool,
}

/// 累计口径：所有请求之和（含压缩与重试，不按分支过滤）
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CumulativeSummary {
    pub total_tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TokenUsageSummary {
    pub occupancy: OccupancySummary,
    /// 该会话零记录时为 `None`，UI 显示「—」
    pub cumulative: Option<CumulativeSummary>,
    pub cache: Option<CacheStats>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheStats {
    pub hit_tokens: i64,
    pub denominator_tokens: i64,
}

impl CacheStats {
    /// 命中率；分母为 0 时不可判定，返回 `None`
    pub fn hit_rate(&self) -> Option<f32> {
        if self.denominator_tokens <= 0 {
            None
        } else {
            Some(self.hit_tokens as f32 / self.denominator_tokens as f32)
        }
    }
}

pub fn insert_record(conn: &Connection, r: &TokenUsageRecord) -> Result<(), String> {
    let context_window_tokens = r.context_window_tokens.filter(|&c| c > 0);
    conn.execute(
        "INSERT INTO token_usage_records (
            conversation_id, message_id, attempt_id, branch_leaf_id, kind,
            provider_id, model_id,
            prompt_tokens, completion_tokens,
            cache_read_tokens, cache_write_tokens, reasoning_tokens,
            cache_capable, context_window_tokens, estimated, created_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
        params![
            r.conversation_id,
            r.message_id,
            r.attempt_id,
            r.branch_leaf_id,
            r.kind,
            r.provider_id,
            r.model_id,
            r.prompt_tokens,
            r.completion_tokens,
            r.cache_read_tokens,
            r.cache_write_tokens,
            r.reasoning_tokens,
            r.cache_capable as i64,
            context_window_tokens,
            r.estimated as i64,
            r.created_at,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

const OCCUPANCY_SQL: &str = "SELECT prompt_tokens, completion_tokens,
            cache_read_tokens, cache_write_tokens, reasoning_tokens,
            context_window_tokens, estimated
     FROM token_usage_records
     WHERE conversation_id = ?1
       AND kind = 'chat'
       AND branch_leaf_id IS ?2";

fn query_latest(
    conn: &Connection,
    conv_id: &str,
    branch_leaf_id: Option<&str>,
    only_reported: bool,
) -> Result<Option<OccupancyRow>, String> {
    let sql = if only_reported {
        format!("{OCCUPANCY_SQL} AND estimated = 0 ORDER BY created_at DESC, id DESC LIMIT 1")
    } else {
        format!("{OCCUPANCY_SQL} ORDER BY created_at DESC, id DESC LIMIT 1")
    };
    conn.query_row(&sql, params![conv_id, branch_leaf_id], |row| {
        Ok(OccupancyRow {
            prompt_tokens: row.get(0)?,
            completion_tokens: row.get(1)?,
            cache_read_tokens: row.get(2)?,
            cache_write_tokens: row.get(3)?,
            reasoning_tokens: row.get(4)?,
            context_window_tokens: row.get::<_, Option<i64>>(5)?.and_then(|v| {
                if v > 0 && v <= u32::MAX as i64 {
                    Some(v as u32)
                } else {
                    None
                }
            }),
            estimated: row.get::<_, i64>(6)? != 0,
        })
    })
    .optional()
    .map_err(|e| e.to_string())
}

/// 上下文占用：当前分支上最新一条 chat 请求的 `prompt_tokens`。
/// 优先非估算记录；该分支无任何非估算记录时才退回最近一条估算记录。
pub fn get_occupancy(
    conn: &Connection,
    conv_id: &str,
    branch_leaf_id: Option<&str>,
) -> Result<Option<OccupancyRow>, String> {
    if let Some(row) = query_latest(conn, conv_id, branch_leaf_id, true)? {
        return Ok(Some(row));
    }
    query_latest(conn, conv_id, branch_leaf_id, false)
}

/// 累计消耗：所有请求之和，含压缩与重试，不按分支过滤。
/// 该会话零记录时返回 `None`（「未知」），不得用 0 冒充「没花过」。
pub fn get_cumulative(conn: &Connection, conv_id: &str) -> Result<Option<i64>, String> {
    Ok(get_cumulative_summary(conn, conv_id)?.map(|s| s.total_tokens))
}

pub fn get_cumulative_summary(
    conn: &Connection,
    conv_id: &str,
) -> Result<Option<CumulativeSummary>, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM token_usage_records WHERE conversation_id = ?1",
            params![conv_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if count == 0 {
        return Ok(None);
    }

    let (total, prompt, completion): (i64, i64, i64) = conn
        .query_row(
            "SELECT COALESCE(SUM(prompt_tokens + completion_tokens), 0),
                    COALESCE(SUM(prompt_tokens), 0),
                    COALESCE(SUM(completion_tokens), 0)
               FROM token_usage_records WHERE conversation_id = ?1",
            params![conv_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| e.to_string())?;
    Ok(Some(CumulativeSummary {
        total_tokens: total,
        prompt_tokens: prompt,
        completion_tokens: completion,
    }))
}

/// 缓存命中统计：分母只含「具备缓存能力且至少上报了一项缓存字段」的请求。
/// 分母为 0 时返回 `None`（不显示命中率），而不是 0。
pub fn get_cache_stats(conn: &Connection, conv_id: &str) -> Result<Option<CacheStats>, String> {
    let (hit, denom): (i64, i64) = conn
        .query_row(
            "SELECT COALESCE(SUM(COALESCE(cache_read_tokens, 0)), 0) AS hit,
                    COALESCE(SUM(prompt_tokens), 0) AS denom
               FROM token_usage_records
              WHERE conversation_id = ?1
                AND cache_capable = 1
                AND (cache_read_tokens IS NOT NULL OR cache_write_tokens IS NOT NULL)",
            params![conv_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    if denom <= 0 {
        Ok(None)
    } else {
        Ok(Some(CacheStats {
            hit_tokens: hit,
            denominator_tokens: denom,
        }))
    }
}

/// 三个口径一次组装，供 IPC 直接消费。
/// `fallback_window` 仅用于「记录里未注入窗口」时的分母补充，取不到仍是 `None`。
pub fn get_summary(
    conn: &Connection,
    conv_id: &str,
    branch_leaf_id: Option<&str>,
    fallback_window: Option<u32>,
) -> Result<TokenUsageSummary, String> {
    let row = get_occupancy(conn, conv_id, branch_leaf_id)?;

    let window = row
        .as_ref()
        .and_then(|r| r.context_window_tokens)
        .or_else(|| fallback_window.filter(|&c| c > 0));

    let occupancy = match row {
        Some(r) => {
            let used = r.prompt_tokens;
            let percentage = window.map(|w| (used as f32 / w as f32).clamp(0.0, 1.0));
            OccupancySummary {
                used_tokens: Some(used),
                completion_tokens: Some(r.completion_tokens),
                cache_read_tokens: r.cache_read_tokens,
                cache_write_tokens: r.cache_write_tokens,
                reasoning_tokens: r.reasoning_tokens,
                context_window_tokens: window,
                percentage,
                estimated: r.estimated,
            }
        }
        None => OccupancySummary {
            context_window_tokens: window,
            ..Default::default()
        },
    };

    Ok(TokenUsageSummary {
        occupancy,
        cumulative: get_cumulative_summary(conn, conv_id)?,
        cache: get_cache_stats(conn, conv_id)?,
    })
}
