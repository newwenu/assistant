use rusqlite::{params, Connection, OptionalExtension};

use super::{read_msg_row, MessageRow, MSG_SELECT};

/// 分页每页最多 turn 数（I46 §7.1）：只影响第 1 步的 turn 集合大小。
pub const TURNS_PER_PAGE: i64 = 12;

pub fn insert_message(conn: &Connection, row: &MessageRow) -> Result<(), String> {
    conn.execute(
        &format!(
            "INSERT INTO messages ({}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
            MSG_SELECT
        ),
        params![
            row.id,
            row.conversation_id,
            row.role,
            row.content,
            row.tokens,
            row.timestamp,
            row.parent_id,
            row.is_pinned as i64,
            row.reasoning,
            row.tool_calls_json,
            row.sibling_index,
            row.provider_id,
            row.model_id,
            row.tool_call_id,
            row.is_error as i64,
            row.status,
            row.mode_id,
            row.turn_index,
            row.iteration
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_messages(conn: &Connection, conversation_id: &str) -> Result<Vec<MessageRow>, String> {
    let sql = format!(
        "SELECT {} FROM messages WHERE conversation_id = ?1 ORDER BY (turn_index IS NULL) ASC, turn_index ASC, iteration ASC, timestamp ASC, rowid ASC",
        MSG_SELECT
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![conversation_id], read_msg_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn update_message_content(
    conn: &Connection,
    id: &str,
    content: &str,
    tokens: Option<i64>,
    reasoning: Option<&str>,
) -> Result<(), String> {
    conn.execute(
        "UPDATE messages SET content = ?2, tokens = ?3, reasoning = ?4 WHERE id = ?1",
        params![id, content, tokens, reasoning],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 写 `messages.tool_calls_json`；在 assistant 行 `finalize` 之前调用。
pub fn update_message_tool_calls(
    conn: &Connection,
    id: &str,
    tool_calls_json: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE messages SET tool_calls_json = ?2 WHERE id = ?1",
        params![id, tool_calls_json],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 声明工具调用（D27）：assistant 行的 `tool_calls_json` 落库后插入占位结果行。
///
/// 占位行的正文为空、`status = 'pending'`，并按序挂在 `parent_id` 之下（首条挂 owner 行，
/// 其后各条挂前一条）。拓扑在声明时即定死，结果到达时只回填正文，不再插入新行——这是
/// 「中断后可自愈」的前提：行已存在，投影有落脚点。
///
/// `rows` 由调用方给出 `(占位行 id, call_id)`，id 生成归宿主适配层（storage 不持 uuid）。
pub fn insert_tool_placeholders(
    conn: &Connection,
    conversation_id: &str,
    parent_id: &str,
    rows: &[(String, String)],
    turn_index: Option<i64>,
    iteration: i64,
    timestamp: i64,
) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let mut parent = Some(parent_id.to_string());
    for (id, call_id) in rows {
        insert_message(
            &tx,
            &MessageRow {
                id: id.clone(),
                conversation_id: conversation_id.to_string(),
                role: "tool".to_string(),
                content: String::new(),
                tokens: None,
                timestamp,
                parent_id: parent.clone(),
                is_pinned: false,
                reasoning: None,
                tool_calls_json: None,
                sibling_index: 0,
                provider_id: None,
                model_id: None,
                tool_call_id: Some(call_id.clone()),
                is_error: false,
                status: super::TOOL_ROW_PENDING.to_string(),
                mode_id: None,
                turn_index,
                iteration,
            },
        )?;
        super::branch::advance_active_branch(&tx, conversation_id, parent.as_deref(), id)?;
        parent = Some(id.clone());
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 标记「已进入执行队列」：占位行 `pending` → `running`。
///
/// 只做单向推进（`WHERE status = 'pending'`），任何情况下都不回退：中断落在执行中时
/// 状态必须停在 `running`，否则会退化成「未执行」诱导模型重复副作用操作。
pub fn mark_tool_rows_running(
    conn: &Connection,
    conversation_id: &str,
    call_ids: &[String],
) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    for call_id in call_ids {
        tx.execute(
            "UPDATE messages SET status = ?3 WHERE conversation_id = ?1 AND role = 'tool' \
             AND tool_call_id = ?2 AND status = ?4",
            params![
                conversation_id,
                call_id,
                super::TOOL_ROW_RUNNING,
                super::TOOL_ROW_PENDING
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 把在途占位行预先写定为「结果未知」：只补正文与错误标记，`status` 保持原状。
///
/// 专供「状态推进写失败」的降级：调用方随后仍会执行工具，此时无法再断言「未开始」，
/// 于是先落保守文案。真正的结果到达时由 `fill_tool_row` 整行覆盖，不影响正常路径；
/// 进程若在此窗口退出，投影期读到非空正文同样按「结果未知」处理，不会退化成可重试提示。
pub fn mark_tool_rows_unknown(
    conn: &Connection,
    conversation_id: &str,
    call_ids: &[String],
) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    for call_id in call_ids {
        tx.execute(
            "UPDATE messages SET content = ?3, is_error = 1 WHERE conversation_id = ?1 \
             AND role = 'tool' AND tool_call_id = ?2 AND status IN (?4, ?5)",
            params![
                conversation_id,
                call_id,
                super::INTERRUPTED_TOOL_RESULT,
                super::TOOL_ROW_PENDING,
                super::TOOL_ROW_RUNNING
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 定位某调用的在途占位行；没有则返回 `None`（存量行 / 声明未成功）。
pub fn find_tool_placeholder(
    conn: &Connection,
    conversation_id: &str,
    call_id: &str,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT id FROM messages WHERE conversation_id = ?1 AND role = 'tool' \
         AND tool_call_id = ?2 AND status IN ('pending','running') ORDER BY rowid DESC LIMIT 1",
        params![conversation_id, call_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

/// 回填结果正文：占位行 `status` 推到 `completed`，正文与错误标记一次写定。
pub fn fill_tool_row(
    conn: &Connection,
    id: &str,
    content: &str,
    is_error: bool,
) -> Result<(), String> {
    conn.execute(
        "UPDATE messages SET content = ?2, is_error = ?3, status = 'completed' WHERE id = ?1",
        params![id, content, is_error as i64],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 写 `messages.status`；取值由调用方给出（`streaming` / `completed` / `interrupted` / `failed`）。
pub fn update_message_status(conn: &Connection, id: &str, status: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE messages SET status = ?2 WHERE id = ?1",
        params![id, status],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_message_status(conn: &Connection, id: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT status FROM messages WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

/// 取会话内下一个 turn 序号（I46 §2.2）：先读 `next_turn_index` 再自增。
///
/// 三条硬约束：`UPDATE` 影响行数为 0 时报「会话不存在」，不返回默认值；
/// 禁止 `SELECT MAX(turn_index) + 1`（坐标必须单调且不随历史行变化）；
/// 本函数不开事务、不 `commit`，事务边界归调用方。
pub fn allocate_turn_index(conn: &Connection, conversation_id: &str) -> Result<u32, String> {
    let next: i64 = conn
        .query_row(
            "SELECT next_turn_index FROM conversations WHERE id = ?1",
            params![conversation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("conversation {} not found", conversation_id))?;
    let updated = conn
        .execute(
            "UPDATE conversations SET next_turn_index = next_turn_index + 1 WHERE id = ?1",
            params![conversation_id],
        )
        .map_err(|e| e.to_string())?;
    if updated == 0 {
        return Err(format!("conversation {} not found", conversation_id));
    }
    u32::try_from(next).map_err(|_| format!("turn_index overflow: {next}"))
}

/// 取该 turn 内下一个迭代序号（I46 §2.2）：`COALESCE(MAX(iteration), 0) + 1`。
/// 唯一允许的算术，且必须带 `turn_index` 等值条件；事务边界同样归调用方。
pub fn allocate_iteration(
    conn: &Connection,
    conversation_id: &str,
    turn_index: u32,
) -> Result<u32, String> {
    let max_iteration: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(iteration), 0) FROM messages \
             WHERE conversation_id = ?1 AND turn_index = ?2",
            params![conversation_id, i64::from(turn_index)],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    u32::try_from(max_iteration + 1).map_err(|_| format!("iteration overflow: {max_iteration}"))
}

/// 分页读取（I46 §7.1）：游标是 turn 而不是时间戳，保证一页内不切开 turn。
///
/// 两步查询：先取上一页的 turn 集合，再取这些 turn 的全部消息。
/// `before_turn_index` 为 `None` 表示最新一页；`turn_index IS NULL` 的存量段
/// 在 turn 游标耗尽后单独成页。
pub fn get_messages_paginated(
    conn: &Connection,
    conversation_id: &str,
    limit: i64,
    before_turn_index: Option<u32>,
) -> Result<Vec<MessageRow>, String> {
    let turn_filter = before_turn_index.map(i64::from);
    let include_unresolved = turn_filter.is_none();

    let turn_ids: Vec<i64> = {
        let sql = if turn_filter.is_some() {
            "SELECT DISTINCT turn_index FROM messages \
             WHERE conversation_id = ?1 AND turn_index IS NOT NULL AND turn_index < ?2 \
             ORDER BY turn_index DESC LIMIT ?3"
        } else {
            "SELECT DISTINCT turn_index FROM messages \
             WHERE conversation_id = ?1 AND turn_index IS NOT NULL \
             ORDER BY turn_index DESC LIMIT ?2"
        };
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = vec![&conversation_id];
        if let Some(t) = turn_filter.as_ref() {
            params_vec.push(t);
        }
        params_vec.push(&TURNS_PER_PAGE);
        let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params_vec.as_slice(), |row| row.get::<_, i64>(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<i64>, _>>()
            .map_err(|e| e.to_string())?
    };

    if turn_ids.is_empty() && !include_unresolved {
        return Ok(Vec::new());
    }

    let turn_placeholders = vec!["?"; turn_ids.len()].join(",");
    let sql = if turn_ids.is_empty() {
        format!(
            "SELECT {} FROM messages WHERE conversation_id = ?1 AND turn_index IS NULL \
             ORDER BY timestamp ASC, rowid ASC",
            MSG_SELECT
        )
    } else if include_unresolved {
        format!(
            "SELECT {} FROM messages WHERE conversation_id = ?1 \
               AND (turn_index IN ({}) OR turn_index IS NULL) \
             ORDER BY (turn_index IS NULL) ASC, turn_index ASC, iteration ASC, timestamp ASC, rowid ASC",
            MSG_SELECT, turn_placeholders
        )
    } else {
        format!(
            "SELECT {} FROM messages WHERE conversation_id = ?1 \
               AND turn_index IN ({}) \
             ORDER BY turn_index ASC, iteration ASC, timestamp ASC, rowid ASC",
            MSG_SELECT, turn_placeholders
        )
    };

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::new();
    params_vec.push(&conversation_id);
    for id in &turn_ids {
        params_vec.push(id);
    }
    let rows = stmt
        .query_map(params_vec.as_slice(), read_msg_row)
        .map_err(|e| e.to_string())?;
    let collected: Vec<MessageRow> = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    // `limit` 是「本页最多消息数」的护栏：只在 turn 边界截断，绝不切开 turn。
    // 截断从「旧」端开始，保留最新的整轮窗口（游标因此仍指向本页最小 turn，
    // 旧轮由下一页补回，不会产生空洞）；单个 turn 自身超过护栏时整轮返回。
    // 存量 `turn_index IS NULL` 段按排序规则垫底，恒保留（它只在首屏出现，
    // 无法靠 turn 游标补回）。
    if let Ok(cap) = usize::try_from(limit) {
        if collected.len() > cap {
            let tail = collected.len()
                - collected
                    .iter()
                    .rev()
                    .take_while(|m| m.turn_index.is_none())
                    .count();
            let mut start = tail;
            while start > 0 {
                let mut begin = start - 1;
                while begin > 0
                    && collected[begin - 1].turn_index == collected[start - 1].turn_index
                {
                    begin -= 1;
                }
                let kept = collected.len() - begin;
                if kept > cap {
                    if start == tail {
                        start = begin;
                    }
                    break;
                }
                start = begin;
            }
            if start > 0 {
                return Ok(collected.into_iter().skip(start).collect());
            }
        }
    }
    Ok(collected)
}

pub fn get_message_count(conn: &Connection, conversation_id: &str) -> Result<i64, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
            params![conversation_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(count)
}

pub fn get_conversation_id_by_message(
    conn: &Connection,
    message_id: &str,
) -> Result<Option<String>, String> {
    let result = conn
        .query_row(
            "SELECT conversation_id FROM messages WHERE id = ?1",
            params![message_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(result)
}

pub fn get_message_by_id(conn: &Connection, id: &str) -> Result<Option<MessageRow>, String> {
    let sql = format!("SELECT {} FROM messages WHERE id = ?1", MSG_SELECT);
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let mut rows = stmt.query(params![id]).map_err(|e| e.to_string())?;
    if let Some(row) = rows.next().map_err(|e| e.to_string())? {
        Ok(Some(read_msg_row(row).map_err(|e| e.to_string())?))
    } else {
        Ok(None)
    }
}

pub fn get_tool_results(
    conn: &Connection,
    conversation_id: &str,
) -> Result<Vec<(String, String, bool)>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT tool_call_id, content, is_error FROM messages WHERE conversation_id = ?1 AND role = 'tool' AND tool_call_id IS NOT NULL AND status NOT IN ('pending','running')",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![conversation_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
            ))
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn get_sibling_count(
    conn: &Connection,
    conversation_id: &str,
    parent_id: Option<&str>,
) -> Result<i64, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1 AND parent_id IS ?2",
            params![conversation_id, parent_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(count)
}

pub fn get_sibling_counts_batch(
    conn: &Connection,
    conversation_id: &str,
    message_ids: &[String],
) -> Result<std::collections::HashMap<String, i64>, String> {
    let mut result = std::collections::HashMap::new();
    if message_ids.is_empty() {
        return Ok(result);
    }

    let mut parent_ids: std::collections::HashSet<Option<String>> =
        std::collections::HashSet::new();
    let mut stmt = conn
        .prepare("SELECT parent_id FROM messages WHERE id = ?1")
        .map_err(|e| e.to_string())?;
    for mid in message_ids {
        let pid: Option<String> = stmt
            .query_row(params![mid], |row| row.get(0))
            .optional()
            .map_err(|e| e.to_string())?
            .flatten();
        parent_ids.insert(pid);
    }

    let mut count_stmt = conn
        .prepare("SELECT COUNT(*) FROM messages WHERE conversation_id = ?1 AND parent_id IS ?2")
        .map_err(|e| e.to_string())?;
    let mut pid_counts: std::collections::HashMap<Option<String>, i64> =
        std::collections::HashMap::new();
    for pid in &parent_ids {
        let count: i64 = count_stmt
            .query_row(params![conversation_id, pid], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        pid_counts.insert(pid.clone(), count);
    }

    let mut pid_stmt = conn
        .prepare("SELECT parent_id FROM messages WHERE id = ?1")
        .map_err(|e| e.to_string())?;
    for mid in message_ids {
        let pid: Option<String> = pid_stmt
            .query_row(params![mid], |row| row.get(0))
            .optional()
            .map_err(|e| e.to_string())?
            .flatten();
        let count = pid_counts.get(&pid).copied().unwrap_or(1);
        result.insert(mid.clone(), count);
    }

    Ok(result)
}
