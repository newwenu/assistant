//! 命令分类：纯函数，无 IO、无环境依赖（I56 §5.2）。
//!
//! 分类结果**只用于展示与 P10-b 输入**，不参与任何放行/拒绝决策：
//! 真正决定执行边界的是沙箱，不是这段字符串匹配。因此宁可归入 `Unknown`，
//! 也不能把危险命令归成「无害」——错误方向必须更危险。

/// 命令类别。开放集合：未知命令一律 `Unknown`，不得窄化。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandClass {
    ReadOnly,
    Network,
    Destructive,
    Unknown,
}

impl CommandClass {
    /// 稳定标识（P10-b 的事实输入）；wire 不承载，前端不窄化。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "readOnly",
            Self::Network => "network",
            Self::Destructive => "destructive",
            Self::Unknown => "unknown",
        }
    }

    /// 审批卡摘要用的中文标签（I56 §5.3）。
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::ReadOnly => "只读",
            Self::Network => "联网",
            Self::Destructive => "破坏性",
            Self::Unknown => "无法判定",
        }
    }
}

/// 只读程序清单：命中 argv[0] 即归只读。
const READONLY_PROGRAMS: &[&str] = &[
    "ls", "dir", "cat", "type", "rg", "grep", "find", "where", "head", "tail", "wc", "pwd", "echo",
];

/// 只读子命令（argv[0] = `git`）。
const READONLY_GIT_SUBCOMMANDS: &[&str] = &["status", "log", "diff", "show"];

/// 联网程序清单。
const NETWORK_PROGRAMS: &[&str] = &["curl", "wget", "ssh", "scp"];

/// 联网子命令（argv[0] = `git`）。
const NETWORK_GIT_SUBCOMMANDS: &[&str] = &["clone", "push", "fetch"];

/// 联网子命令（包管理器）。
const NETWORK_PACKAGE_MANAGERS: &[&str] = &["npm", "pip", "pip3"];

/// 破坏性程序清单。
const DESTRUCTIVE_PROGRAMS: &[&str] =
    &["rm", "del", "rd", "rmdir", "format", "shutdown", "taskkill"];

/// 破坏性子命令（`argv[0] = git`）。
const DESTRUCTIVE_GIT_SUBCOMMANDS: &[&str] = &["clean"];

/// 只读程序里能拉起外部程序或直接删改的参数：出现即不能再算只读。
///
/// `find -exec/-execdir/-ok/-okdir/-delete`、`rg --pre`、`git --ext-diff`
/// 都能借「只读程序」的壳执行任意命令或删文件；只按 `argv[0]` 判定就会把
/// 危险命令归成「无害」，方向正好反了（§5.2 硬约束 1）。
const EXEC_CAPABLE_FLAGS: &[&str] = &[
    "-exec",
    "-execdir",
    "-ok",
    "-okdir",
    "-delete",
    "--pre",
    "--ext-diff",
];

/// 版本查询类参数：出现即视为只读（见 `classify` 的保守条件）。
const VERSION_FLAGS: &[&str] = &["--version", "-V"];

/// 命令分类（I56 §5.2 的固定顺序）。
///
/// 1. 含 shell 控制操作符 → `Unknown`（拼接语义无法静态判定）；
/// 2. argv[0] / 子命令命中只读清单 → `ReadOnly`；
/// 3. 命中联网清单 → `Network`（**先于**第 4 步：`git clean` 这类复合语义
///    宁可归联网也不归无害）；
/// 4. 命中破坏性清单 → `Destructive`；
/// 5. 其余 → `Unknown`。
pub fn classify(command: &str) -> CommandClass {
    if contains_control_operator(command) {
        return CommandClass::Unknown;
    }
    let tokens = tokens(command);
    let Some(head) = tokens.first() else {
        return CommandClass::Unknown;
    };
    let program = program_name(head);
    let sub = tokens.get(1).map(|token| token.as_str());

    if is_readonly(&program, sub, &tokens) {
        return CommandClass::ReadOnly;
    }
    if is_network(&program, sub) {
        return CommandClass::Network;
    }
    if is_destructive(&program, sub, &tokens) {
        return CommandClass::Destructive;
    }
    CommandClass::Unknown
}

/// 控制操作符：`|`（含 `||`）、`&`（含 `&&`、后台）、`;`、`>`（含 `>>`）、
/// `<`、反引号、`$(`、换行。
///
/// 单个 `&` 与 `&&` 一样是拼接语义：`ls & del a.txt` 在 `sh` 与 `cmd` 下都会
/// 执行两条命令，漏掉它就会把后半段的破坏性命令一起归成「只读」。
fn contains_control_operator(command: &str) -> bool {
    command
        .chars()
        .any(|ch| matches!(ch, '|' | '&' | ';' | '>' | '<' | '`' | '\n' | '\r'))
        || command.contains("$(")
}

fn is_readonly(program: &str, sub: Option<&str>, tokens: &[String]) -> bool {
    // 带执行类参数时，程序本身再「只读」也只是壳：`find -exec rm` 的真实
    // 语义是 `rm`，不是 `find`。
    if has_exec_capable_flag(tokens) {
        return false;
    }
    // 版本查询只在程序本身不属于联网/破坏性清单时算只读：
    // `rm --version` 归只读就是错误方向，宁可 `Unknown`。
    if has_version_flag(tokens) && !is_risky_program(program) {
        return true;
    }
    if READONLY_PROGRAMS.contains(&program) {
        return true;
    }
    match (program, sub) {
        ("git", Some(sub)) => READONLY_GIT_SUBCOMMANDS.contains(&sub),
        _ => false,
    }
}

/// 执行类参数判定：`--pre=cmd` 这种内联写法也算命中。
fn has_exec_capable_flag(tokens: &[String]) -> bool {
    tokens.iter().skip(1).any(|token| {
        let token = token.as_str();
        EXEC_CAPABLE_FLAGS.iter().any(|flag| {
            token == *flag
                || token
                    .strip_prefix(flag)
                    .is_some_and(|rest| rest.starts_with('='))
        })
    })
}

fn is_network(program: &str, sub: Option<&str>) -> bool {
    if NETWORK_PROGRAMS.contains(&program) {
        return true;
    }
    match (program, sub) {
        ("git", Some(sub)) => NETWORK_GIT_SUBCOMMANDS.contains(&sub),
        _ => NETWORK_PACKAGE_MANAGERS.contains(&program) && sub == Some("install"),
    }
}

fn is_destructive(program: &str, sub: Option<&str>, tokens: &[String]) -> bool {
    if DESTRUCTIVE_PROGRAMS.contains(&program) {
        return true;
    }
    // 只读程序带执行类参数：真实动作由参数决定，按破坏性处理。
    if has_exec_capable_flag(tokens) {
        return true;
    }
    match (program, sub) {
        ("git", Some("reset")) => tokens.iter().any(|token| token == "--hard"),
        ("git", Some(sub)) => DESTRUCTIVE_GIT_SUBCOMMANDS.contains(&sub),
        ("reg", Some("delete")) => true,
        _ => false,
    }
}

fn is_risky_program(program: &str) -> bool {
    NETWORK_PROGRAMS.contains(&program) || DESTRUCTIVE_PROGRAMS.contains(&program)
}

fn has_version_flag(tokens: &[String]) -> bool {
    tokens
        .iter()
        .skip(1)
        .any(|token| VERSION_FLAGS.contains(&token.as_str()))
}

/// argv[0] 归一：去引号、取 basename（`/` 与 `\` 都算分隔符）、去 `.exe`、小写。
///
/// 用户可能写 `C:\Windows\System32\dir.exe` 这种全路径形式，不归一就会漏判。
fn program_name(token: &str) -> String {
    let trimmed = token.trim_matches(|ch| ch == '"' || ch == '\'');
    let base = trimmed
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(trimmed)
        .to_ascii_lowercase();
    base.strip_suffix(".exe").unwrap_or(&base).to_string()
}

/// 极简分词：按空白切分，单/双引号内不切分，引号本身不保留。
///
/// 只服务于分类，不需要与真实 shell 的转义规则一致：任何解析不确定的形态
/// 都会被上一步的控制操作符判定挡成 `Unknown`。
fn tokens(command: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in command.chars() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_operator_degrades_to_unknown() {
        for command in [
            "git log | head",
            "git status && git log",
            "git status || true",
            "ls; rm -rf build",
            "echo hi > out.txt",
            "type a.txt < b.txt",
            "echo `whoami`",
            "echo $(whoami)",
            "git status\ngit log",
        ] {
            assert_eq!(classify(command), CommandClass::Unknown, "{command}");
        }
    }

    /// 单个 `&` 也是拼接语义：漏掉它会把后半段的破坏性命令一起归成只读。
    #[test]
    fn single_ampersand_is_a_control_operator() {
        for command in [
            "ls & del a.txt",
            "dir & rd /s /q build",
            "echo hi & rm -rf target",
            "git status & git clean -fdx",
            // 引号内的 `&` 也会降级：不做引号感知是刻意的保守取舍，
            // 宁可把带 `&` 的 URL 显示成「无法判定」，也不放过真实后台拼接。
            "curl 'https://example.com?a=1&b=2'",
        ] {
            assert_eq!(classify(command), CommandClass::Unknown, "{command}");
        }
    }

    /// 只读程序带执行类参数时，真实动作由参数决定，必须按破坏性处理。
    #[test]
    fn readonly_programs_with_exec_capable_flags_are_destructive() {
        for command in [
            "find . -delete",
            "find . -exec rm {} +",
            "find . -execdir rm {} +",
            "find . -ok rm {} +",
            "rg --pre 'rm -rf target' pattern",
            "rg --pre=rm pattern",
            "git log --ext-diff",
            "git show --ext-diff",
        ] {
            assert_eq!(classify(command), CommandClass::Destructive, "{command}");
        }
    }

    /// 同一批程序不带执行类参数时仍是只读，不能一刀切降级。
    #[test]
    fn readonly_programs_without_exec_flags_stay_readonly() {
        for command in [
            "find . -name '*.rs'",
            "find . -type f -newer a.txt",
            "rg -n pattern",
            "rg --pre-glob '*.txt' pattern",
            "git log --oneline",
            "git diff --stat",
            "git show HEAD",
        ] {
            assert_eq!(classify(command), CommandClass::ReadOnly, "{command}");
        }
    }

    #[test]
    fn readonly_commands() {
        for command in [
            "git status",
            "git log --oneline -n 3",
            "git diff",
            "git show HEAD",
            "ls -la",
            "dir",
            "cat Cargo.toml",
            "rg TODO",
            "grep -rn foo .",
            "find . -name '*.rs'",
            "head -n 5 a.txt",
            "wc -l a.txt",
            "pwd",
            "echo hello",
            "rg --version",
            "git --version",
        ] {
            assert_eq!(classify(command), CommandClass::ReadOnly, "{command}");
        }
    }

    #[test]
    fn network_commands() {
        for command in [
            "curl https://example.com",
            "wget https://example.com/x.zip",
            "ssh host uptime",
            "scp a.txt host:/tmp",
            "git clone https://example.com/r.git",
            "git push origin main",
            "git fetch --all",
            "npm install",
            "pip install requests",
        ] {
            assert_eq!(classify(command), CommandClass::Network, "{command}");
        }
    }

    #[test]
    fn destructive_commands() {
        for command in [
            "rm -rf target",
            "del a.txt",
            "rd /s /q build",
            "rmdir build",
            "shutdown /s",
            "taskkill /pid 123 /f",
            "git clean -fdx",
            "git reset --hard HEAD~1",
            "reg delete HKCU\\Software\\Foo",
        ] {
            assert_eq!(classify(command), CommandClass::Destructive, "{command}");
        }
    }

    #[test]
    fn version_flag_never_whitens_risky_program() {
        assert_eq!(classify("rm --version"), CommandClass::Destructive);
        assert_eq!(classify("curl --version"), CommandClass::Network);
        assert_eq!(classify("taskkill /? --version"), CommandClass::Destructive);
    }

    #[test]
    fn full_path_and_suffix_are_normalized() {
        assert_eq!(
            classify(r"C:\Windows\System32\dir.exe /b"),
            CommandClass::ReadOnly
        );
        assert_eq!(classify("/usr/bin/git status"), CommandClass::ReadOnly);
    }

    #[test]
    fn quoted_program_is_normalized() {
        assert_eq!(
            classify("\"C:/Program Files/Git/bin/git\" status"),
            CommandClass::ReadOnly
        );
    }

    #[test]
    fn unknown_by_default() {
        for command in ["", "   ", "mytool --flag", "git rebase -i HEAD~2"] {
            assert_eq!(classify(command), CommandClass::Unknown, "{command}");
        }
    }
}
