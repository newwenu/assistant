use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;

use crate::context::ToolUseContext;
use crate::r#trait::{BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier};
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

impl Default for FsLinkTool {
    fn default() -> Self {
        Self::new()
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

    fn audience(&self) -> ToolAudience {
        ToolAudience::TOOLBOX_ONLY
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
            PermissionIntent::new("edit", source),
            PermissionIntent::new("edit", target),
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

        let source = super::path::resolve_leaf_preserving(source_str, context)?;
        let target = super::path::resolve_leaf_preserving(target_str, context)?;
        let kind = LinkKind::from_str(kind_str)?;

        if !target.exists() {
            return Ok(ToolResult::err(format!(
                "target '{}' does not exist",
                target.display()
            )));
        }

        let Some(source_parent) = source.parent() else {
            return Ok(ToolResult::err(format!(
                "source '{}' has no parent directory",
                source.display()
            )));
        };
        if !source_parent.exists() {
            return Ok(ToolResult::err(format!(
                "parent directory '{}' of source does not exist",
                source_parent.display()
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

        let source = super::path::resolve_leaf_preserving(source_str, context)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    fn context_with_root(root: &Path) -> ToolUseContext {
        ToolUseContext::for_toolbox(Some(root.display().to_string()))
    }

    async fn run_create(
        source: &Path,
        target: &Path,
        kind: &str,
        context: &ToolUseContext,
    ) -> Result<ToolResult, String> {
        FsLinkTool
            .execute(
                &json!({
                    "action": "create",
                    "source": source.display().to_string(),
                    "target": target.display().to_string(),
                    "kind": kind,
                }),
                context,
            )
            .await
    }

    async fn run_check(source: &Path, context: &ToolUseContext) -> Result<ToolResult, String> {
        FsLinkTool
            .execute(
                &json!({
                    "action": "check",
                    "source": source.display().to_string(),
                }),
                context,
            )
            .await
    }

    fn try_create_file_symlink(link: &Path, target: &Path) -> bool {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(target, link).is_ok()
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (link, target);
            false
        }
    }

    fn try_create_dir_link(link: &Path, target: &Path) -> bool {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }
        #[cfg(windows)]
        {
            create_junction(link, target).is_ok()
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (link, target);
            false
        }
    }

    fn protected_root_for_test() -> Option<PathBuf> {
        #[cfg(windows)]
        {
            std::env::var_os("SystemRoot").map(PathBuf::from)
        }
        #[cfg(unix)]
        {
            Some(PathBuf::from("/etc"))
        }
        #[cfg(not(any(unix, windows)))]
        {
            None
        }
    }

    #[test]
    fn permission_intents_use_edit_vocabulary() {
        let intents = FsLinkTool
            .permission_intents(
                &json!({"action": "create", "source": "a.txt", "target": "b.txt"}),
                &ToolUseContext::default(),
            )
            .expect("permission intents");
        assert_eq!(intents.len(), 2);
        for intent in &intents {
            assert_eq!(intent.action, "edit");
        }
        assert_eq!(intents[0].resources, vec!["a.txt".to_string()]);
        assert_eq!(intents[1].resources, vec!["b.txt".to_string()]);
    }

    #[test]
    fn permission_intents_are_empty_for_check() {
        let intents = FsLinkTool
            .permission_intents(
                &json!({"action": "check", "source": "a.txt"}),
                &ToolUseContext::default(),
            )
            .expect("permission intents");
        assert!(intents.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn check_reports_regular_file_as_not_link() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("plain.txt");
        std::fs::write(&file, b"data").expect("write file");
        let context = context_with_root(dir.path());

        let result = run_check(&file, &context).await.expect("check");
        assert!(result.ok, "check must succeed: {:?}", result.error);
        let data = result.data.expect("data");
        assert_eq!(data["isLink"], json!(false));
        assert_eq!(data["kind"], json!(null));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn create_reports_missing_target() {
        let dir = tempfile::tempdir().expect("tempdir");
        let context = context_with_root(dir.path());
        let source = dir.path().join("link.txt");
        let target = dir.path().join("missing.txt");

        let result = run_create(&source, &target, "symlink", &context)
            .await
            .expect("create call");
        assert!(!result.ok, "missing target must fail");
        assert!(
            result
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("does not exist"),
            "unexpected error: {:?}",
            result.error
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn create_reports_missing_source_parent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let context = context_with_root(dir.path());
        let target = dir.path().join("target.txt");
        std::fs::write(&target, b"data").expect("write target");
        let source = dir.path().join("missing_dir").join("link.txt");

        let result = run_create(&source, &target, "symlink", &context)
            .await
            .expect("create call");
        assert!(!result.ok, "missing source parent must fail");
        assert!(
            result
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("parent directory"),
            "unexpected error: {:?}",
            result.error
        );
        assert!(!dir.path().join("missing_dir").exists());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn create_rejects_path_outside_workspace() {
        let root_dir = tempfile::tempdir().expect("tempdir");
        let other_dir = tempfile::tempdir().expect("tempdir");
        let context = context_with_root(root_dir.path());
        let target = root_dir.path().join("target.txt");
        std::fs::write(&target, b"data").expect("write target");
        let source = other_dir.path().join("link.txt");

        let error = run_create(&source, &target, "symlink", &context)
            .await
            .expect_err("outside path must be rejected");
        assert!(
            error.contains("outside workspace root"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn create_rejects_parent_dir_traversal() {
        let dir = tempfile::tempdir().expect("tempdir");
        let context = context_with_root(dir.path());
        let target = dir.path().join("target.txt");
        std::fs::write(&target, b"data").expect("write target");
        let source = dir.path().join("sub").join("..").join("link.txt");

        let error = run_create(&source, &target, "symlink", &context)
            .await
            .expect_err("'..' must be rejected");
        assert!(error.contains(".."), "unexpected error: {error}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn create_rejects_escape_through_symlinked_directory() {
        let root_dir = tempfile::tempdir().expect("tempdir");
        let outside_dir = tempfile::tempdir().expect("tempdir");
        let escape = root_dir.path().join("escape");
        if !try_create_dir_link(&escape, outside_dir.path()) {
            return;
        }
        let context = context_with_root(root_dir.path());
        let target = root_dir.path().join("target.txt");
        std::fs::write(&target, b"data").expect("write target");
        let source = escape.join("link.txt");

        let error = run_create(&source, &target, "symlink", &context)
            .await
            .expect_err("escape through symlinked dir must be rejected");
        assert!(
            error.contains("outside workspace root"),
            "unexpected error: {error}"
        );
        assert!(
            !outside_dir.path().join("link.txt").exists(),
            "nothing may be written outside the workspace"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn create_never_writes_through_existing_source_symlink() {
        let root_dir = tempfile::tempdir().expect("tempdir");
        let outside_dir = tempfile::tempdir().expect("tempdir");
        let outside_file = outside_dir.path().join("outside.txt");
        std::fs::write(&outside_file, b"data").expect("write outside file");
        let source = root_dir.path().join("escape.txt");
        if !try_create_file_symlink(&source, &outside_file) {
            return;
        }
        let context = context_with_root(root_dir.path());
        let target = root_dir.path().join("target.txt");
        std::fs::write(&target, b"data").expect("write target");

        let result = run_create(&source, &target, "symlink", &context).await;
        assert!(
            result.is_err() || !result.expect("result").ok,
            "existing source symlink must not be replaced silently"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn check_does_not_follow_source_symlink() {
        let root_dir = tempfile::tempdir().expect("tempdir");
        let outside_dir = tempfile::tempdir().expect("tempdir");
        let outside_file = outside_dir.path().join("outside.txt");
        std::fs::write(&outside_file, b"data").expect("write outside file");
        let source = root_dir.path().join("link.txt");
        if !try_create_file_symlink(&source, &outside_file) {
            return;
        }
        let context = context_with_root(root_dir.path());

        let result = run_check(&source, &context).await.expect("check");
        assert!(result.ok, "check must succeed: {:?}", result.error);
        let data = result.data.expect("data");
        assert_eq!(data["isLink"], json!(true));
        assert_eq!(data["kind"], json!("symlink"));
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread")]
    async fn create_symlink_and_check_reports_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let context = context_with_root(dir.path());
        let target = dir.path().join("target.txt");
        std::fs::write(&target, b"data").expect("write target");
        let source = dir.path().join("link.txt");

        let result = run_create(&source, &target, "symlink", &context)
            .await
            .expect("create call");
        assert!(result.ok, "symlink create must succeed: {:?}", result.error);
        assert!(source
            .symlink_metadata()
            .expect("metadata")
            .file_type()
            .is_symlink());

        let checked = run_check(&source, &context).await.expect("check");
        assert_eq!(checked.data.expect("data")["isLink"], json!(true));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn create_rejects_protected_workspace_root() {
        let Some(protected) = protected_root_for_test() else {
            return;
        };
        let context = context_with_root(&protected);
        let source = protected.join("link.txt");
        let target = protected.join("target.txt");

        let error = run_create(&source, &target, "symlink", &context)
            .await
            .expect_err("protected workspace root must be rejected");
        assert!(
            error.contains("protected system directory"),
            "unexpected error: {error}"
        );
    }

    #[cfg(windows)]
    #[tokio::test(flavor = "multi_thread")]
    async fn create_junction_and_check_reports_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let context = context_with_root(dir.path());
        let target = dir.path().join("target_dir");
        std::fs::create_dir(&target).expect("create target dir");
        let source = dir.path().join("junction");

        let result = run_create(&source, &target, "junction", &context)
            .await
            .expect("create call");
        assert!(
            result.ok,
            "junction create must succeed: {:?}",
            result.error
        );

        let checked = run_check(&source, &context).await.expect("check");
        let data = checked.data.expect("data");
        assert_eq!(data["isLink"], json!(true));
        let reported = data["target"].as_str().unwrap_or_default();
        assert!(
            reported.ends_with("target_dir"),
            "reported target must point at the junction destination: {reported}"
        );
    }

    #[cfg(windows)]
    #[tokio::test(flavor = "multi_thread")]
    async fn create_junction_reports_missing_target_parent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let context = context_with_root(dir.path());
        let target = dir.path().join("missing_dir").join("target_dir");
        let source = dir.path().join("junction");

        let result = run_create(&source, &target, "junction", &context)
            .await
            .expect("create call");
        assert!(!result.ok, "missing target parent must fail");
        assert!(
            result
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("does not exist"),
            "unexpected error: {:?}",
            result.error
        );
        assert!(!source.exists(), "junction must not be created");
    }

    #[cfg(windows)]
    #[tokio::test(flavor = "multi_thread")]
    async fn create_rejects_source_on_nonexistent_drive() {
        let dir = tempfile::tempdir().expect("tempdir");
        let context = context_with_root(dir.path());
        let target = dir.path().join("target.txt");
        std::fs::write(&target, b"data").expect("write target");
        let drive = ('D'..='Z')
            .map(|letter| format!("{}:\\", letter))
            .find(|candidate| !Path::new(candidate).exists());
        let Some(drive) = drive else {
            return;
        };
        let source = PathBuf::from(drive).join("link.txt");

        let error = run_create(&source, &target, "symlink", &context)
            .await
            .expect_err("nonexistent drive must be rejected");
        assert!(
            error.contains("no existing ancestor"),
            "unexpected error: {error}"
        );
    }
}
