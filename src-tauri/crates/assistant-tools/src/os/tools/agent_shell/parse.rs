//! 命令结构解析：只产出**可证的结构事实**，不做程序语义推断（I72 §6.7）。
//!
//! 与 `classify` 的分工：`classify` 回答「这是什么程序」，本模块回答
//! 「这条命令原文由几条语句、几个重定向、哪段内嵌载荷组成，写目标落在哪」。
//! 两者都只用于展示与审批输入，不参与任何放行决策。
//!
//! 解析失败不是错误：所有字段都有保守缺省（语句数至少 1、置信降为 `unknown`）。

use std::path::Path;

use crate::os::path::{canonicalize_lenient, ensure_within_root};
use crate::r#trait::ShellPreview;

use super::classify::CommandClass;
use super::schema::ShellInput;
use super::spawn::ShellFlavor;

/// 内嵌载荷的启动程序（深度 1）。
const PAYLOAD_PROGRAMS: &[&str] = &["sh", "bash", "zsh", "dash", "cmd", "powershell", "pwsh"];

/// 载荷参数：出现即其后全部是子命令原文。
const PAYLOAD_FLAGS: &[&str] = &["-c", "/c", "-command", "/command", "-Command", "/C"];

/// 会写文件系统的程序（用于决定是否产出写目标范围）。
const WRITE_PROGRAMS: &[&str] = &[
    "mkdir", "touch", "cp", "mv", "rm", "del", "rd", "rmdir", "ren", "rename", "tee", "ln",
    "chmod", "chown", "dd", "truncate", "xcopy", "robocopy", "sed",
];

/// 空设备：写入它们不改变外部状态，不作为重定向目标呈现。
const NULL_DEVICES: &[&str] = &["nul", "/dev/null"];

/// 解析出的结构事实（内部形态）。
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct CommandFacts {
    pub(crate) statements: u32,
    pub(crate) redirects: Vec<String>,
    pub(crate) payload: Option<String>,
    pub(crate) program: Option<String>,
    pub(crate) redirect_raw: Vec<String>,
    pub(crate) args: Vec<String>,
}

impl CommandFacts {
    pub(crate) fn chains(&self) -> u32 {
        self.statements.saturating_sub(1)
    }

    /// 是否可能写文件系统：破坏性类别、非空重定向、写类程序三者任一。
    fn writes(&self, class: CommandClass) -> bool {
        class == CommandClass::Destructive
            || !self.redirect_raw.is_empty()
            || self
                .program
                .as_deref()
                .is_some_and(|program| WRITE_PROGRAMS.contains(&program))
    }
}

/// 组装审批预览（`summary` 由调用方置空，此处只回填结构）。
///
/// `flavor` 决定变量语法：`cmd` 的 `%X%` 与 `sh` 的 `$X` 都算动态成分，
/// 目标含动态成分时只能给 `unknown`，不得当成工作区内（I72 §6.6 规则 2）。
pub(crate) fn preview(
    input: &ShellInput,
    flavor: ShellFlavor,
    class: CommandClass,
) -> ShellPreview {
    let facts = scan(&input.command);
    let targets_scope = targets_scope(input, flavor, class, &facts);
    let confidence = if class == CommandClass::Unknown {
        "unknown"
    } else if facts.payload.is_some() {
        "inferred"
    } else {
        "certain"
    };
    ShellPreview {
        statements: facts.statements,
        chains: facts.chains(),
        class: class.as_str().to_string(),
        redirects: facts.redirects.clone(),
        payload: facts.payload.clone(),
        targets_scope,
        confidence: confidence.to_string(),
    }
}

/// 扫描命令原文：切语句、取重定向目标、取内嵌载荷。
pub(crate) fn scan(command: &str) -> CommandFacts {
    let segments = split_statements(command);
    let statements = segments.len().max(1) as u32;

    let mut redirects = Vec::new();
    let mut redirect_raw = Vec::new();
    let mut payload = None;

    for segment in &segments {
        let tokens = tokens(segment);
        if tokens.is_empty() {
            continue;
        }
        if payload.is_none() {
            payload = extract_payload(&tokens);
        }
        collect_redirects(segment, &mut redirect_raw);
    }
    for raw in &redirect_raw {
        if is_null_device(raw) {
            continue;
        }
        if let Some(name) = last_segment(raw) {
            if !redirects.contains(&name) {
                redirects.push(name);
            }
        }
    }
    redirect_raw.retain(|raw| !is_null_device(raw));

    let program = segments
        .first()
        .and_then(|segment| tokens(segment).first().cloned())
        .map(|token| program_name(&token));
    let args = segments
        .first()
        .map(|segment| tokens(segment).into_iter().skip(1).collect::<Vec<_>>())
        .unwrap_or_default();

    CommandFacts {
        statements,
        redirects,
        payload,
        program,
        redirect_raw,
        args,
    }
}

/// 写目标范围：`inside` / `outside` / `unknown`；无写意图时 `None`。
///
/// 判定复用执行期同一套规范化（[`canonicalize_lenient`] + [`ensure_within_root`]），
/// 避免「卡片说区内、执行说越界」。任一目标越界即 `outside`（宁可更危险）。
fn targets_scope(
    input: &ShellInput,
    flavor: ShellFlavor,
    class: CommandClass,
    facts: &CommandFacts,
) -> Option<String> {
    if !facts.writes(class) {
        return None;
    }
    let mut candidates: Vec<&str> = facts.redirect_raw.iter().map(String::as_str).collect();
    for arg in &facts.args {
        if arg.starts_with('-') {
            continue;
        }
        candidates.push(arg.as_str());
    }

    let marker = flavor.variable_marker();
    let mut inside = false;
    let mut outside = false;
    let mut unresolved = false;
    for candidate in candidates {
        if candidate.is_empty() || is_dynamic(candidate, marker) {
            unresolved = true;
            continue;
        }
        match canonicalize_lenient(&input.cwd.join(candidate)) {
            Ok(path) => {
                if ensure_within_root(&path, &input.writable_root).is_ok() {
                    inside = true;
                } else {
                    outside = true;
                }
            }
            Err(_) => unresolved = true,
        }
    }

    let scope = if outside {
        "outside"
    } else if inside && !unresolved {
        "inside"
    } else {
        "unknown"
    };
    Some(scope.to_string())
}

/// 含动态成分（本方言变量、通配符）的路径无法静态求交。
fn is_dynamic(candidate: &str, marker: char) -> bool {
    candidate.contains(marker) || candidate.contains('*') || candidate.contains('?')
}

/// 按顶层控制操作符切语句（引号内的操作符不切）。
fn split_statements(command: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = command.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\'' | '"' if quote == Some(ch) => {
                quote = None;
                current.push(ch);
            }
            '\'' | '"' if quote.is_none() => {
                quote = Some(ch);
                current.push(ch);
            }
            _ if quote.is_some() => current.push(ch),
            ';' | '\n' | '\r' => push_segment(&mut out, &mut current),
            '&' => {
                if chars.peek() == Some(&'&') {
                    chars.next();
                }
                push_segment(&mut out, &mut current);
            }
            '|' => {
                if chars.peek() == Some(&'|') {
                    chars.next();
                }
                push_segment(&mut out, &mut current);
            }
            _ => current.push(ch),
        }
    }
    push_segment(&mut out, &mut current);
    out
}

fn push_segment(out: &mut Vec<String>, current: &mut String) {
    let segment = current.trim();
    if !segment.is_empty() {
        out.push(segment.to_string());
    }
    current.clear();
}

/// 取语句内所有 `>` / `>>` 的重定向目标原文（引号外）。
fn collect_redirects(segment: &str, out: &mut Vec<String>) {
    let mut quote: Option<char> = None;
    for (index, ch) in segment.char_indices() {
        match ch {
            '\'' | '"' if quote == Some(ch) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(ch),
            '>' if quote.is_none() => {
                let mut end = index + 1;
                if segment[end..].starts_with('>') || segment[end..].starts_with('|') {
                    end += 1;
                }
                let rest = &segment[end..];
                let start = end + rest.len() - rest.trim_start().len();
                let target: String = segment[start..]
                    .chars()
                    .take_while(|ch| !ch.is_whitespace() && !matches!(ch, ';' | '|' | '&'))
                    .collect();
                let target = target
                    .trim_matches(|ch| ch == '"' || ch == '\'')
                    .to_string();
                if !target.is_empty() {
                    out.push(target);
                }
            }
            _ => {}
        }
    }
}

/// 内嵌载荷：`bash -c "..."` / `cmd /c ...` 等，只取深度 1。
fn extract_payload(tokens: &[String]) -> Option<String> {
    let program = program_name(tokens.first()?);
    if !PAYLOAD_PROGRAMS.contains(&program.as_str()) {
        return None;
    }
    let position = tokens
        .iter()
        .skip(1)
        .position(|token| PAYLOAD_FLAGS.contains(&token.as_str()))?;
    let rest: Vec<&str> = tokens[position + 2..].iter().map(String::as_str).collect();
    if rest.is_empty() {
        return None;
    }
    Some(rest.join(" "))
}

/// 极简分词：按空白切分，引号内不切分，引号本身不保留。
fn tokens(segment: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in segment.chars() {
        match ch {
            '\'' | '"' if quote == Some(ch) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(ch),
            ch if ch.is_whitespace() && quote.is_none() => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            ch => current.push(ch),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn program_name(token: &str) -> String {
    let trimmed = token.trim_matches(|ch| ch == '"' || ch == '\'');
    let base = trimmed
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(trimmed)
        .to_ascii_lowercase();
    base.strip_suffix(".exe").unwrap_or(&base).to_string()
}

fn is_null_device(raw: &str) -> bool {
    let normalized = raw
        .trim_matches(|ch| ch == '"' || ch == '\'')
        .to_ascii_lowercase();
    NULL_DEVICES.contains(&normalized.as_str())
}

/// 末段：卡片只留目录名 / 文件名，不带本机绝对路径。
fn last_segment(raw: &str) -> Option<String> {
    let path = Path::new(raw);
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_command_has_one_statement() {
        let facts = scan("ls -la");
        assert_eq!(facts.statements, 1);
        assert_eq!(facts.chains(), 0);
        assert!(facts.redirects.is_empty());
        assert!(facts.payload.is_none());
    }

    #[test]
    fn chains_count_separators() {
        let facts = scan("cargo build && cargo test || echo failed");
        assert_eq!(facts.statements, 3);
        assert_eq!(facts.chains(), 2);
    }

    #[test]
    fn quotes_do_not_split_statements() {
        let facts = scan("echo 'a && b; c'");
        assert_eq!(facts.statements, 1);
    }

    #[test]
    fn pipe_splits_statements() {
        let facts = scan("cat a.txt | rg needle");
        assert_eq!(facts.statements, 2);
    }

    #[test]
    fn redirect_target_keeps_only_last_segment() {
        let facts = scan("echo hi > C:/tmp/out/result.txt");
        assert_eq!(facts.redirects, vec!["result.txt".to_string()]);
    }

    #[test]
    fn append_redirect_is_recognised() {
        let facts = scan("echo hi >> build.log");
        assert_eq!(facts.redirects, vec!["build.log".to_string()]);
    }

    #[test]
    fn null_device_redirect_is_not_a_write_target() {
        let facts = scan("echo hi > /dev/null");
        assert!(facts.redirects.is_empty());
        assert!(facts.redirect_raw.is_empty());
    }

    #[test]
    fn payload_is_extracted_depth_one() {
        let facts = scan("bash -c 'rm -rf build'");
        assert_eq!(facts.payload.as_deref(), Some("rm -rf build"));
        assert_eq!(facts.statements, 1);
    }

    #[test]
    fn non_shell_program_has_no_payload() {
        let facts = scan("rg -c needle src");
        assert!(facts.payload.is_none());
    }

    #[test]
    fn program_is_normalised() {
        let facts = scan("C:\\Windows\\System32\\cmd.exe /c dir");
        assert_eq!(facts.program.as_deref(), Some("cmd"));
        assert_eq!(facts.payload.as_deref(), Some("dir"));
    }
}
