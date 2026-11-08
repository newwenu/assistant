use std::path::Path;

pub(super) struct LinkInfo {
    pub is_link: bool,
    pub kind: Option<String>,
    pub target: Option<String>,
}

pub(super) fn check_link(path: &Path) -> Result<LinkInfo, String> {
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
