use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PromptTemplateRow {
    pub id: String,
    pub name: String,
    pub category: String,
    pub content: String,
    pub variables_json: Option<String>,
    pub is_builtin: bool,
}

pub fn insert_template(conn: &Connection, row: &PromptTemplateRow) -> Result<(), String> {
    let is_builtin = if row.is_builtin { 1 } else { 0 };
    conn.execute(
        "INSERT OR IGNORE INTO prompt_templates (id, name, category, content, variables, is_builtin)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![row.id, row.name, row.category, row.content, row.variables_json, is_builtin],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_templates(conn: &Connection) -> Result<Vec<PromptTemplateRow>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, category, content, variables, is_builtin FROM prompt_templates ORDER BY category, name")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(PromptTemplateRow {
                id: row.get(0)?,
                name: row.get(1)?,
                category: row.get(2)?,
                content: row.get(3)?,
                variables_json: row.get(4)?,
                is_builtin: row.get::<_, i64>(5)? != 0,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn seed_builtin_templates(conn: &Connection) -> Result<(), String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM prompt_templates WHERE is_builtin = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if count > 0 {
        return Ok(());
    }
    let builtins = [
        PromptTemplateRow {
            id: "builtin-translate".to_string(),
            name: "翻译助手".to_string(),
            category: "text".to_string(),
            content: "请将以下内容翻译为{{target_language}}：\n\n{{content}}".to_string(),
            variables_json: Some(r#"["target_language","content"]"#.to_string()),
            is_builtin: true,
        },
        PromptTemplateRow {
            id: "builtin-code-explain".to_string(),
            name: "代码解释".to_string(),
            category: "dev".to_string(),
            content: "请解释以下代码的功能：\n\n```{{language}}\n{{code}}\n```".to_string(),
            variables_json: Some(r#"["language","code"]"#.to_string()),
            is_builtin: true,
        },
        PromptTemplateRow {
            id: "builtin-summarize".to_string(),
            name: "文本摘要".to_string(),
            category: "text".to_string(),
            content: "请用{{length}}字以内总结以下内容：\n\n{{content}}".to_string(),
            variables_json: Some(r#"["length","content"]"#.to_string()),
            is_builtin: true,
        },
        PromptTemplateRow {
            id: "builtin-continue-writing".to_string(),
            name: "续写".to_string(),
            category: "text".to_string(),
            content: "请根据以下内容继续写作：\n\n{{content}}".to_string(),
            variables_json: Some(r#"["content"]"#.to_string()),
            is_builtin: true,
        },
        PromptTemplateRow {
            id: "builtin-brainstorm".to_string(),
            name: "头脑风暴".to_string(),
            category: "daily".to_string(),
            content: "请针对以下主题给出{{count}}个创意想法：\n\n{{topic}}".to_string(),
            variables_json: Some(r#"["count","topic"]"#.to_string()),
            is_builtin: true,
        },
    ];
    for t in &builtins {
        insert_template(conn, t)?;
    }
    Ok(())
}