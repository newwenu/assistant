use std::path::{Component, Path, PathBuf};

use crate::context::ToolUseContext;
use crate::r#trait::PermissionIntent;

use super::membership::ensure_within_root;
use super::protected::reject_system_dirs_as;
use super::resolve::{
    canonicalize_lenient_as, resolve, resolve_leaf_preserving as resolve_leaf_preserving_path,
    resolve_readable as resolve_readable_path, ReadablePath, ReadableScope,
};

/// 本会话输出域的 scheme 名（I73 §3.1）。
pub const TOOL_OUTPUT_SCHEME: &str = "tool-output";
/// shell 会话临时区的 scheme 名（I73 §3.8）。
pub const SCRATCH_SCHEME: &str = "scratch";

/// 写方向的拒绝文案：按 scheme 的**类别**（[`SchemeNature`]）派生，不按 scheme 名硬编码（I73 §4 V7）。
///
/// 三句对应两种类别：
/// - `Namespace`（平铺内容域）：域本身不可写，**没有**替代通道，因此不提示任何写法；
/// - `Location`（可写目录重定向）：域可写，拒绝只源于传输层（本工具只收普通路径）；
///   该域若声明了真实通道（`shell` 临时区）就指向它，没声明则只陈述事实。
///
/// 共同前提：**不得写「must be plain paths」**。对域引用来说，物理路径正是模型不知道
/// 也不该知道的东西；把它写成输入要求等于让模型去猜绝对路径——这正是 §2 第 16 行缺陷的起点。
const WRITE_NAMESPACE_REJECTED: &str =
    "this tool cannot write through a path reference. Files under this scheme are produced by the run that generated them and cannot be edited afterwards";

const WRITE_LOCATION_SHELL_REJECTED: &str =
    "this tool cannot write through a path reference. Files in the shell temp area are created by shell commands: write them there (the command's temp dir is the same directory) and read them back with the reference";

const WRITE_LOCATION_NO_CHANNEL_REJECTED: &str =
    "this tool cannot write through a path reference. This location has no write channel from here";

/// 工作目录方向（`shell` 的 `cwd`）遇到引用时的拒绝文案（I73 §4 V7）。
///
/// 与写方向分开：这里要求「工作区内」的普通路径，而工作区根对模型已知，说明落点不构成泄漏，
/// 也不诱导它猜域根。
pub const CWD_REJECTED: &str =
    "a working directory cannot be a path reference: pass a plain path inside the workspace";

/// 浏览方向（`list` / `grep` / `glob` 的基准目录）遇到引用时的统一拒绝文案（I73 §4）。
///
/// 与写方向分成两句：浏览不是写入，套用写方向文案会让模型以为自己调错了工具类别，
/// 转而重试写工具或改写路径——这正是要避免的试探行为。
/// 允许提示 `read`：那是**真实可用**的替代通道，不是边界试探线索。
pub const BROWSE_REJECTED: &str = "this path scheme is not supported by this tool yet: it takes a plain path only; to open a specific file under that scheme, use read";

/// 写方向的拒绝文案（I73 §4 V7）：唯一派生点是 [`Scheme::nature`]。
///
/// 写成「按类别派生」而不是「按 scheme 名匹配」：新增 scheme 时只声明它的类别，
/// 文案自动就位；反过来，`Namespace` 在类型上不可能携带通道（[`SchemeNature`]），
/// 「给不可写域提示写通道」这种假提示因此不可表达。
pub fn write_rejected(scheme: Scheme) -> String {
    match scheme.nature() {
        SchemeNature::Namespace => WRITE_NAMESPACE_REJECTED,
        SchemeNature::Location { write_alternate } => match write_alternate {
            Some(WriteAlternate::ShellTempArea) => WRITE_LOCATION_SHELL_REJECTED,
            None => WRITE_LOCATION_NO_CHANNEL_REJECTED,
        },
    }
    .to_string()
}

const NO_TOOL_OUTPUT_ROOT: &str = "no tool output root is available for this call";
const NO_SCRATCH_ROOT: &str =
    "scratch not found: no session scratch root is available for this call";

/// 封闭的 scheme 集合（I73 §3.1 G1）。
///
/// 新增变体必须同时改解析、拒绝文案、接入矩阵与验收；本枚举是这四处的唯一锚点。
/// 「属于哪一类」由 [`Scheme::nature`] 声明——那里是新增变体的第一个必改点。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    ToolOutput,
    Scratch,
}

/// scheme 的**类别**：与「是哪个 scheme」正交的轴（I73 §3.1 G1 / §4 V7）。
///
/// 两类 scheme 的差别不在名字与域根，而在域的本质：
/// - `Namespace`：body 是**平铺**的成员名，域内容由产出方生成，域本身不可写；
/// - `Location`：body 是域内相对路径，域**只是一个可写目录**，写方向拒绝只源于传输层。
///
/// 这条轴只承载两类**确有差异**的事实（写方向能否提示替代通道），不承担「哪个方向接受引用」
/// 的矩阵：那张矩阵按方向统一、与 scheme 无关（§4 接入矩阵），按 scheme 声明它只会
/// 变成 O(scheme×方向) 的冗余数据，并且把「方向策略」这一层本该唯一的判断拆散。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemeNature {
    /// 平铺内容域：成员名平铺、内容由产出方生成、域本身不可写。
    Namespace,
    /// 可写目录重定向：域内相对路径、域可写，写方向拒绝只源于传输层。
    ///
    /// `write_alternate` 是该域**真实可用**的写入通道；`None` = 没有，文案不得提示。
    /// 嵌在类别里而不是并列成独立字段：`Namespace` 携带通道是非法状态，
    /// 放进类别后它在类型上不可表达，「给不可写域提示写通道」这类假提示不会出现。
    Location {
        write_alternate: Option<WriteAlternate>,
    },
}

/// 写方向的真实替代通道（I73 §4 V7）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteAlternate {
    /// 子进程临时目录：`shell` 命令的 `TMPDIR` / `TEMP` / `TMP` 就是同一目录（§3.8 S3）。
    ShellTempArea,
}

impl Scheme {
    /// 本集合的全部变体：给测试对**闭集**做不变量断言用。
    ///
    /// 新增变体必须同步加进来。`nature()` 的穷尽匹配会在编译期提醒漏分类，
    /// 但本数组不会——它是这条轴上唯一的「手工清单」，改动时一并核对。
    pub const ALL: [Scheme; 2] = [Scheme::ToolOutput, Scheme::Scratch];

    pub fn as_str(self) -> &'static str {
        match self {
            Scheme::ToolOutput => TOOL_OUTPUT_SCHEME,
            Scheme::Scratch => SCRATCH_SCHEME,
        }
    }

    /// 类别（与「是哪个 scheme」正交的轴）。新增变体必须在此完成分类。
    pub const fn nature(self) -> SchemeNature {
        match self {
            // 输出域是**产出物**的平铺命名空间：内容由 run 生成，工具不写、有保留期。
            Scheme::ToolOutput => SchemeNature::Namespace,
            // 临时区只是「工作区之外的一个可写目录」：域可写，且子进程通道真实存在。
            Scheme::Scratch => SchemeNature::Location {
                write_alternate: Some(WriteAlternate::ShellTempArea),
            },
        }
    }

    pub(crate) fn parse(name: &str) -> Option<Self> {
        match name {
            TOOL_OUTPUT_SCHEME => Some(Scheme::ToolOutput),
            SCRATCH_SCHEME => Some(Scheme::Scratch),
            _ => None,
        }
    }
}

/// 路径引用：原始串 + 拆出的 scheme 与 body。
///
/// 字段私有、只能经 [`parse_ref`] 构造（I73 §3.3 E2）：工具若绕过它直接读
/// `args["path"]`，串会带着 `://` 一路走到 `join_input`，在那里被断言式拒绝（E1）。
/// 不变量：`scheme.is_some() == body.is_some()`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathRef {
    raw: String,
    scheme: Option<Scheme>,
    body: Option<String>,
}

impl PathRef {
    /// 原始串。回显诊断与写回结果数据用它，绝不用它做 IO。
    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn scheme(&self) -> Option<Scheme> {
        self.scheme
    }

    /// scheme 之后的 body；无 scheme 时为 `None`。
    pub fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    /// 模型可见错误里的路径标签（I73 §3.10 D4）。
    ///
    /// 有 scheme：回显调用方原串——域根物理路径含用户名与会话哈希，不得进入模型上下文；
    /// 无 scheme：回显解析落点——工作区根对模型已知，落点即诊断信息（D3）。
    pub fn error_label(&self, resolved: &Path) -> String {
        match self.scheme {
            Some(_) => self.raw.clone(),
            None => resolved.display().to_string(),
        }
    }

    /// 读方向：工作区 / 输出域 / 临时区统一走这一条。
    ///
    /// 无 scheme 时逐字节沿用既有 `resolve_readable`（I73 §3.1 G3）；
    /// 有 scheme 时由域根拼根，再走同一条「归一 → 受保护目录 → 域边界」流水线。
    pub fn resolve_readable(&self, context: &ToolUseContext) -> Result<ReadablePath, String> {
        let Some(scheme) = self.scheme else {
            return resolve_readable_path(&self.raw, context);
        };

        let (root, scope) = domain_root(scheme, context)?;
        // 尾部单个 `/` 只是「目录根」标记（I73 §3.8「语法」）：拼根前剥掉，
        // 否则 `Path::join("/")` 会把根替换成绝对路径（Windows 上尤其危险）。
        let body = self.body.as_deref().unwrap_or_default();
        let body = body.strip_suffix('/').unwrap_or(body);
        let joined = root.join(body);
        let canonical = canonicalize_lenient_as(&joined, self.raw())?;
        reject_system_dirs_as(&canonical, self.raw())?;
        ensure_within_root(&canonical, root).map_err(|_| escape_message(self.raw(), scheme))?;

        Ok(ReadablePath {
            path: canonical,
            scope,
        })
    }

    /// 写方向：引用一律拒绝（I73 §4）。
    pub fn resolve_write(&self, context: &ToolUseContext) -> Result<PathBuf, String> {
        match self.scheme {
            None => resolve(&self.raw, context),
            Some(scheme) => Err(write_rejected(scheme)),
        }
    }

    /// 工作目录方向（`shell` 的 `cwd`）：引用一律拒绝，文案与写方向不同（I73 §4 V7）。
    pub fn resolve_working_directory(&self, context: &ToolUseContext) -> Result<PathBuf, String> {
        match self.scheme {
            None => resolve(&self.raw, context),
            Some(_) => Err(CWD_REJECTED.to_string()),
        }
    }

    /// 浏览方向（`list` / `grep` / `glob` 的基准目录）：引用一律拒绝（I73 §4）。
    ///
    /// 无 scheme 时与写方向同一条落点解析（浏览只需要 `PathBuf`，不需要 `ReadableScope`），
    /// 有 scheme 时给出浏览专用文案——不复用写方向文案（§4 V5）。
    pub fn resolve_browse(&self, context: &ToolUseContext) -> Result<PathBuf, String> {
        match self.scheme {
            None => resolve(&self.raw, context),
            Some(_) => Err(BROWSE_REJECTED.to_string()),
        }
    }

    /// 叶子保留方向（操作链接本身）：引用一律拒绝，文案与写方向同源（I73 §4 V7）。
    pub fn resolve_leaf_preserving(&self, context: &ToolUseContext) -> Result<PathBuf, String> {
        match self.scheme {
            None => resolve_leaf_preserving_path(&self.raw, context),
            Some(scheme) => Err(write_rejected(scheme)),
        }
    }
}

impl ReadablePath {
    /// 审批意图：`ReadableScope → (action, resource)` 的唯一映射（I73 §3.5 P1）。
    ///
    /// `read` 与 `super_read` 共用它，两种写法（绝对路径 / `tool-output://`）因此
    /// 命中同一授权键（P3）。
    pub fn intent(&self, context: &ToolUseContext) -> Result<PermissionIntent, String> {
        match self.scope {
            ReadableScope::Workspace => Ok(PermissionIntent::new(
                "read",
                self.path.display().to_string(),
            )),
            ReadableScope::ToolOutput => {
                let root = context
                    .tool_output_root
                    .as_deref()
                    .ok_or_else(|| NO_TOOL_OUTPUT_ROOT.to_string())?;
                Ok(PermissionIntent::new(
                    "readToolOutput",
                    root.display().to_string(),
                ))
            }
            // 一次授权覆盖本会话全部临时区（I73 §3.8 P5）：`resource` 取会话目录本身。
            ReadableScope::Scratch => {
                let root = context
                    .tool_scratch_root
                    .as_deref()
                    .ok_or_else(|| NO_SCRATCH_ROOT.to_string())?;
                Ok(PermissionIntent::new(
                    "readScratch",
                    root.display().to_string(),
                ))
            }
        }
    }
}

/// 解析一个路径串：无 scheme 原样保留；命中已知 scheme 则校验 body；含 `://` 但未命中则拒绝。
pub fn parse_ref(raw: &str) -> Result<PathRef, String> {
    let Some((scheme_name, body)) = raw.split_once("://") else {
        return Ok(PathRef {
            raw: raw.to_string(),
            scheme: None,
            body: None,
        });
    };

    let scheme = Scheme::parse(scheme_name).ok_or_else(|| {
        format!(
            "unsupported path scheme '{}' in '{}': the only schemes are '{}://' and '{}://'",
            scheme_name, raw, TOOL_OUTPUT_SCHEME, SCRATCH_SCHEME
        )
    })?;
    validate_body(scheme, body, raw)?;

    Ok(PathRef {
        raw: raw.to_string(),
        scheme: Some(scheme),
        body: Some(body.to_string()),
    })
}

/// 产出侧的格式串工具：由落盘文件名派生模型可见的引用（I73 §3.6 R1）。
pub fn tool_output_ref(file_name: &str) -> String {
    format!("{}://{}", TOOL_OUTPUT_SCHEME, file_name)
}

fn validate_body(scheme: Scheme, body: &str, raw: &str) -> Result<(), String> {
    if body.contains('\0') {
        return Err(invalid_ref(raw, "must not contain NUL"));
    }
    if body != body.trim() {
        return Err(invalid_ref(
            raw,
            "must not have leading or trailing whitespace",
        ));
    }
    match scheme {
        Scheme::ToolOutput => validate_output_body(body, raw),
        Scheme::Scratch => validate_scratch_body(body, raw),
    }
}

/// 输出域 body：恒为单个文件名（I73 §3.4 / N7「平铺」）。
fn validate_output_body(body: &str, raw: &str) -> Result<(), String> {
    if body.is_empty() {
        return Err(invalid_ref(raw, "a file name is required"));
    }
    // `Path::join` 遇到带分隔符或盘符的片段会替换根（`root.join("C:\\x")` = `C:\x`），
    // 因此这些形态必须在拼根之前拒绝，而不是靠 `canonicalize` 事后拦。
    if body.contains('\\') || body.contains('/') || body.contains(':') {
        return Err(invalid_ref(
            raw,
            "the body must be a single file name without path separators",
        ));
    }
    if body == "." || body == ".." {
        return Err(invalid_ref(
            raw,
            "the body must be a file name, not a directory marker",
        ));
    }
    match Path::new(body).components().collect::<Vec<_>>().as_slice() {
        [Component::Normal(_)] => Ok(()),
        _ => Err(invalid_ref(raw, "the body must be a single file name")),
    }
}

/// 临时区 body：域内相对路径，允许嵌套、可为空（表域根）（I73 §3.8「语法」）。
fn validate_scratch_body(body: &str, raw: &str) -> Result<(), String> {
    if body.contains('\\') || body.contains(':') {
        return Err(invalid_ref(
            raw,
            "the body must be a relative path without drive or backslash",
        ));
    }
    // 单个 `/` 与空 body 同义：都指会话临时区根本身（A17）。
    if body == "/" {
        return Ok(());
    }
    if body.starts_with('/') {
        return Err(invalid_ref(raw, "the body must not start with '/'"));
    }

    // 尾部单个 `/` 只标记「目录根」，不产生空段；其余空段仍拒绝。
    let trimmed = body.strip_suffix('/').unwrap_or(body);
    if trimmed.is_empty() {
        return Ok(());
    }

    for segment in trimmed.split('/') {
        if segment.is_empty() {
            return Err(invalid_ref(
                raw,
                "the body must not contain empty path segments",
            ));
        }
        if segment == "." || segment == ".." {
            return Err(invalid_ref(
                raw,
                "the body must not contain '.' or '..' segments",
            ));
        }
    }
    Ok(())
}

fn invalid_ref(raw: &str, reason: &str) -> String {
    format!("invalid path reference '{}': {}", raw, reason)
}

fn escape_message(raw: &str, scheme: Scheme) -> String {
    format!(
        "path reference '{}' escapes the {} root",
        raw,
        scheme.as_str()
    )
}

fn domain_root(scheme: Scheme, context: &ToolUseContext) -> Result<(&Path, ReadableScope), String> {
    match scheme {
        Scheme::ToolOutput => context
            .tool_output_root
            .as_deref()
            .map(|root| (root, ReadableScope::ToolOutput))
            .ok_or_else(|| NO_TOOL_OUTPUT_ROOT.to_string()),
        // 会话临时区根由上下文注入（N1），与注入侧同一实现点；缺根即业务错误。
        Scheme::Scratch => context
            .tool_scratch_root
            .as_deref()
            .map(|root| (root, ReadableScope::Scratch))
            .ok_or_else(|| NO_SCRATCH_ROOT.to_string()),
    }
}
