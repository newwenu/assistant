use async_trait::async_trait;
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::context::ToolUseContext;
use crate::r#trait::{BehaviorHints, PermissionIntent, Tool, ToolTier};
use crate::result::ToolResult;

const TOOL_NAME: &str = "fs_link";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkKind {
    Symlink,
    Hardlink,
    #[cfg(windows)]
    Junction,
}

impl LinkKind {
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "symlink" => Ok(Self::Symlink),
            "hardlink" => Ok(Self::Hardlink),
            #[cfg(windows)]
            "junction" => Ok(Self::Junction),
            _ => Err(format!(
                "unsupported link kind '{}'; expected 'symlink' or 'hardlink'{}",
                s,
                if cfg!(windows) { " or 'junction'" } else { "" }
            )),
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Symlink => "symlink",
            Self::Hardlink => "hardlink",
            #[cfg(windows)]
            Self::Junction => "junction",
        }
    }
}

pub struct FsLinkTool;

impl FsLinkTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for FsLinkTool {
    fn name(&self) -> &str {
        TOOL_NAME
    }

    async fn description(&self) -> Result<String, String> {
        Ok("Create or check filesystem links (symlink, hardlink, junction)".to_string())
    }

    fn input_schema(&self) -> Value {
        let mut kind_enum = vec!["symlink".to_string(), "hardlink".to_string()];
        #[cfg(windows)]
        kind_enum.push("junction".to_string());

        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "check"],
                    "description": "Action to perform"
                },
                "source": {
                    "type": "string",
                    "description": "Source path (existing file/dir for create; link path for check)"
                },
                "target": {
                    "type": "string",
                    "description": "Target path (where the link points to)"
                },
                "kind": {
                    "type": "string",
                    "enum": kind_enum,
                    "description": "Link type"
                }
            },
            "required": ["action", "source"],
            "additionalProperties": false
        })
    }

    fn tier(&self) -> ToolTier {
        ToolTier::Os
    }

    fn behavior_hints(&self) -> BehaviorHints {
        BehaviorHints {
            read_only: false,
            destructive: false,
            open_world: true,
        }
    }

    fn permission_intents(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        let action = input.get("action").and_then(Value::as_str).unwrap_or("");

        if action == "check" {
            return Ok(vec![]);
        }

        let source = input
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        let target = input
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");

        Ok(vec![
            PermissionIntent::new("fs_link_create", source),
            PermissionIntent::new("fs_link_target", target),
        ])
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        let action = input
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing 'action' field".to_string())?;

        match action {
            "create" => self.exec_create(input, context).await,
            "check" => self.exec_check(input, context).await,
            _ => Err(format!("unknown action '{}'", action)),
        }
    }
}

impl FsLinkTool {
    fn resolve_path(path_str: &str, context: &ToolUseContext) -> Result<PathBuf, String> {
        let path = PathBuf::from(path_str);
        if path.is_absolute() {
            validate_path(&path)?;
            return Ok(path);
        }

        if let Some(root) = &context.workspace_root {
            let resolved = PathBuf::from(root).join(&path);
            validate_path(&resolved)?;
            return Ok(resolved);
        }

        Err(format!(
            "relative path '{}' requires workspace_root in context",
            path_str
        ))
    }

    async fn exec_create(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Result<ToolResult, String> {
        let source_str = input
            .get("source")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing 'source' field".to_string())?;
        let target_str = input
            .get("target")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing 'target' field".to_string())?;
        let kind_str = input
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing 'kind' field".to_string())?;

        let source = Self::resolve_path(source_str, context)?;
        let target = Self::resolve_path(target_str, context)?;
        let kind = LinkKind::from_str(kind_str)?;

        if !target.exists() {
            return Ok(ToolResult::err(format!(
                "target '{}' does not exist",
                target.display()
            )));
        }

        create_link(&source, &target, kind)?;

        Ok(ToolResult::ok(serde_json::json!({
            "source": source_str,
            "target": target_str,
            "kind": kind.as_str(),
            "created": true
        })))
    }

    async fn exec_check(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Result<ToolResult, String> {
        let source_str = input
            .get("source")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing 'source' field".to_string())?;

        let source = Self::resolve_path(source_str, context)?;

        let info = check_link(&source)?;

        Ok(ToolResult::ok(serde_json::json!({
            "source": source_str,
            "isLink": info.is_link,
            "kind": info.kind,
            "target": info.target,
        })))
    }
}

struct LinkInfo {
    is_link: bool,
    kind: Option<String>,
    target: Option<String>,
}

fn validate_path(path: &Path) -> Result<(), String> {
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(format!(
            "path '{}' contains '..' traversal, use a canonical path instead",
            path.display()
        ));
    }

    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    let forbidden_unix: &[&str] = &["/etc", "/usr", "/bin", "/sbin", "/System", "/Library"];
    for prefix in forbidden_unix {
        if canonical.starts_with(prefix) {
            return Err(format!(
                "path '{}' is inside a protected system directory",
                path.display()
            ));
        }
    }

    #[cfg(windows)]
    {
        let forbidden_win: &[&str] = &[
            r"C:\Windows",
            r"C:\Program Files",
            r"C:\Program Files (x86)",
            r"C:\ProgramData",
        ];
        let path_str = canonical.to_string_lossy();
        for prefix in forbidden_win {
            if path_str.starts_with(prefix) {
                return Err(format!(
                    "path '{}' is inside a protected system directory",
                    path.display()
                ));
            }
        }
    }

    Ok(())
}

fn create_link(source: &Path, target: &Path, kind: LinkKind) -> Result<(), String> {
    match kind {
        LinkKind::Symlink => create_symlink(source, target),
        LinkKind::Hardlink => create_hardlink(source, target),
        #[cfg(windows)]
        LinkKind::Junction => create_junction(source, target),
    }
}

fn create_symlink(source: &Path, target: &Path) -> Result<(), String> {
    let is_dir = target.is_dir();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, source)
            .map_err(|e| format!("symlink create failed: {}", e))
    }
    #[cfg(windows)]
    {
        if is_dir {
            std::os::windows::fs::symlink_dir(target, source)
                .map_err(|e| format!("symlink_dir create failed: {} (may require Developer Mode or SeCreateSymbolicLinkPrivilege)", e))
        } else {
            std::os::windows::fs::symlink_file(target, source)
                .map_err(|e| format!("symlink_file create failed: {} (may require Developer Mode or SeCreateSymbolicLinkPrivilege)", e))
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (source, target, is_dir);
        Err("symlink not supported on this platform".to_string())
    }
}

fn create_hardlink(source: &Path, target: &Path) -> Result<(), String> {
    if target.is_dir() {
        return Err("hardlink cannot target a directory".to_string());
    }
    std::fs::hard_link(target, source).map_err(|e| format!("hardlink create failed: {}", e))
}

#[cfg(windows)]
fn create_junction(source: &Path, target: &Path) -> Result<(), String> {
    if !target.is_dir() {
        return Err("junction can only target a directory".to_string());
    }
    if source.exists() {
        return Err(format!(
            "junction source '{}' already exists",
            source.display()
        ));
    }

    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let output = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(source)
        .arg(target)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("failed to execute mklink: {}", e))?;

    if !output.status.success() {
        let stderr = super::process_output::decode_process_output(&output.stderr);
        let stdout = super::process_output::decode_process_output(&output.stdout);
        return Err(format!(
            "junction create failed for '{}' -> '{}' (exit code: {}, stderr: {}, stdout: {})",
            source.display(),
            target.display(),
            output.status.code().unwrap_or(-1),
            stderr,
            stdout
        ));
    }
    Ok(())
}

fn check_link(path: &Path) -> Result<LinkInfo, String> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LinkInfo {
                is_link: false,
                kind: None,
                target: None,
            });
        }
        Err(e) => return Err(format!("failed to read metadata: {}", e)),
    };

    if metadata.file_type().is_symlink() {
        let target = std::fs::read_link(path)
            .map(|p| p.to_string_lossy().to_string())
            .ok();
        return Ok(LinkInfo {
            is_link: true,
            kind: Some("symlink".to_string()),
            target,
        });
    }

    #[cfg(windows)]
    {
        if metadata.file_type().is_dir() {
            if let Some(target) = read_junction_target(path) {
                return Ok(LinkInfo {
                    is_link: true,
                    kind: Some("junction".to_string()),
                    target: Some(target),
                });
            }
        }
    }

    if is_hardlink(path, &metadata) {
        return Ok(LinkInfo {
            is_link: true,
            kind: Some("hardlink".to_string()),
            target: None,
        });
    }

    Ok(LinkInfo {
        is_link: false,
        kind: None,
        target: None,
    })
}

fn is_hardlink(path: &Path, metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        metadata.nlink() > 1
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };

        let Ok(file) = std::fs::File::open(path) else {
            return false;
        };
        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        unsafe {
            let Ok(()) = GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut information)
            else {
                return false;
            };
        }
        information.nNumberOfLinks > 1
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        false
    }
}

#[cfg(windows)]
fn read_junction_target(path: &Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_READ_ATTRIBUTES, OPEN_EXISTING,
    };

    let wide_path: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide_path.as_ptr()),
            FILE_READ_ATTRIBUTES.0,
            windows::Win32::Storage::FileSystem::FILE_SHARE_READ
                | windows::Win32::Storage::FileSystem::FILE_SHARE_WRITE
                | windows::Win32::Storage::FileSystem::FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    };

    let handle = match handle {
        Ok(h) if h != INVALID_HANDLE_VALUE => h,
        _ => return None,
    };

    let result = unsafe { read_final_path(handle) };
    let _ = unsafe { CloseHandle(handle) };
    result
}

#[cfg(windows)]
unsafe fn read_final_path(handle: windows::Win32::Foundation::HANDLE) -> Option<String> {
    use windows::Win32::Storage::FileSystem::{GetFinalPathNameByHandleW, FILE_NAME_NORMALIZED};

    let flags = FILE_NAME_NORMALIZED;

    let needed = GetFinalPathNameByHandleW(handle, &mut [], flags);
    if needed == 0 {
        return None;
    }

    let buffer_len = needed as usize;
    let mut buffer = vec![0u16; buffer_len];

    let written = GetFinalPathNameByHandleW(handle, &mut buffer, flags);

    if written == 0 {
        return None;
    }

    let written = written as usize;
    if written > buffer_len {
        return None;
    }

    let path_str = String::from_utf16(&buffer[..written]).ok()?;
    Some(path_str.trim_start_matches(r"\\?\").to_string())
}