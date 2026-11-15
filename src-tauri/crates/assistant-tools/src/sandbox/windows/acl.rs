use std::path::Path;

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Authorization::{
    ConvertStringSidToSidW, GetExplicitEntriesFromAclW, GetNamedSecurityInfoW, SetEntriesInAclW,
    SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, GRANT_ACCESS, SET_ACCESS, SE_FILE_OBJECT, TRUSTEE_W,
};
use windows::Win32::Security::{
    EqualSid, ACL, DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
};

use crate::sandbox::{SandboxError, SandboxPlan};

/// 沙箱专用合成 SID（写边界锚点）。
///
/// 必须是**固定值**：下一次 `spawn` 的准备要能按 SID 找回并替换上次崩溃留下的 ACE。
/// 取值在 NT 权威下的私有子权威段，不与应用自身的任何真实账户重叠。
pub(crate) const SANDBOX_SID: &str = "S-1-5-99-24773-4163";

const STANDARD_RIGHTS_WRITE: u32 = 0x0002_0000;
const FILE_GENERIC_WRITE_MASK: u32 = 0x0012_0116;
const DELETE_MASK: u32 = 0x0001_0000;
const FILE_DELETE_CHILD_MASK: u32 = 0x0000_0040;

/// 可写根上的唯一授权掩码：写 + 删 + 删子项，排除 `WRITE_DAC` / `WRITE_OWNER`。
///
/// 只授 `FILE_GENERIC_WRITE` 会让根内删除 / 改名失败（该掩码本身不含删除位，
/// 而删除是工具的正常操作）。`STANDARD_RIGHTS_WRITE` 是 `READ_CONTROL` 的别名，
/// `FILE_GENERIC_WRITE` 自带该位，留在掩码里唯一的效果是多给「读 DACL」这项元数据
/// 权限，因此一并清掉；真正会越界的是 `WRITE_DAC`（`0x00040000`）与 `WRITE_OWNER`
/// （`0x00080000`），两者从未出现在掩码里。读位与执行位不需要授：
/// pass-2 只检查写类访问，读 / 执行走 pass-1 原有 SID（规格 §5.1 / §12）。
pub(crate) const WRITE_GRANT_MASK: u32 =
    (FILE_GENERIC_WRITE_MASK | DELETE_MASK | FILE_DELETE_CHILD_MASK) & !STANDARD_RIGHTS_WRITE;

const INHERIT: windows::Win32::Security::ACE_FLAGS =
    windows::Win32::Security::SUB_CONTAINERS_AND_OBJECTS_INHERIT;

fn to_wide(value: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    let mut units: Vec<u16> = value.as_os_str().encode_wide().collect();
    units.push(0);
    units
}

/// 分配沙箱 SID；调用方负责 `LocalFree`。
pub(crate) fn sandbox_sid() -> Result<PSID, SandboxError> {
    let wide = to_wide(Path::new(SANDBOX_SID));
    let mut sid = PSID::default();
    unsafe { ConvertStringSidToSidW(PCWSTR::from_raw(wide.as_ptr()), &mut sid) }
        .map_err(|_| SandboxError::SetupFailed("acl_convert_string_sid".to_string()))?;
    Ok(sid)
}

pub(crate) fn free_sid(sid: PSID) {
    unsafe {
        LocalFree(Some(HLOCAL(sid.0)));
    }
}

/// 一次读取的显式 ACE 列表，连同它依赖的两块分配一起持有。
pub(crate) struct ExplicitEntries {
    descriptor: PSECURITY_DESCRIPTOR,
    list: *mut EXPLICIT_ACCESS_W,
    count: u32,
}

impl ExplicitEntries {
    pub(crate) fn as_slice(&self) -> &[EXPLICIT_ACCESS_W] {
        if self.list.is_null() || self.count == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.list, self.count as usize) }
        }
    }
}

impl Drop for ExplicitEntries {
    fn drop(&mut self) {
        unsafe {
            if !self.list.is_null() {
                LocalFree(Some(HLOCAL(self.list as *mut core::ffi::c_void)));
            }
            if !self.descriptor.0.is_null() {
                LocalFree(Some(HLOCAL(self.descriptor.0)));
            }
        }
    }
}

pub(crate) fn read_entries(path: &Path) -> Result<ExplicitEntries, SandboxError> {
    let wide = to_wide(path);
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    let mut dacl: *mut ACL = std::ptr::null_mut();
    let status = unsafe {
        GetNamedSecurityInfoW(
            PCWSTR::from_raw(wide.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut dacl),
            None,
            &mut descriptor,
        )
    };
    if status.is_err() {
        return Err(SandboxError::SetupFailed("acl_read_dacl".to_string()));
    }
    if dacl.is_null() {
        // NULL DACL = 全员完全访问。以它为底重建 ACL 会顺手放开别人，
        // 因此这里判「无法建立写边界」，交给 fail-closed 路径（F-S3）。
        unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
        return Err(SandboxError::SetupFailed("acl_null_dacl".to_string()));
    }
    let mut count: u32 = 0;
    let mut list: *mut EXPLICIT_ACCESS_W = std::ptr::null_mut();
    let status = unsafe { GetExplicitEntriesFromAclW(dacl, &mut count, &mut list) };
    if status.is_err() {
        unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
        return Err(SandboxError::SetupFailed("acl_read_entries".to_string()));
    }
    Ok(ExplicitEntries {
        descriptor,
        list,
        count,
    })
}

fn entry_sid(entry: &EXPLICIT_ACCESS_W) -> PSID {
    PSID(entry.Trustee.ptstrName.0 as *mut core::ffi::c_void)
}

/// 该 ACE 是否属于沙箱 SID（不区分 allow / deny）。
fn is_sandbox_entry(entry: &EXPLICIT_ACCESS_W, sid: PSID) -> bool {
    if entry.Trustee.ptstrName.0.is_null() {
        return false;
    }
    unsafe { EqualSid(entry_sid(entry), sid) }.is_ok()
}

fn desired_entry(
    sid: PSID,
    mode: windows::Win32::Security::Authorization::ACCESS_MODE,
    mask: u32,
) -> EXPLICIT_ACCESS_W {
    EXPLICIT_ACCESS_W {
        grfAccessPermissions: mask,
        grfAccessMode: mode,
        grfInheritance: INHERIT,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: windows::Win32::Security::Authorization::NO_MULTIPLE_TRUSTEE,
            TrusteeForm: windows::Win32::Security::Authorization::TRUSTEE_IS_SID,
            TrusteeType: windows::Win32::Security::Authorization::TRUSTEE_IS_UNKNOWN,
            ptstrName: PWSTR(sid.0 as *mut u16),
        },
    }
}

/// 把可写根上的沙箱 ACE 收敛为唯一期望 ACE。
///
/// 已存在完全一致的 ACE 时**一次写入都不做**，因此重复 `prepare` 的 ACE 计数不变（A-S7）。
/// 否则用 `SET_ACCESS`：同一 trustee 的旧 ACE（崩溃残留、旧掩码）由期望 ACE 替换，
/// 一次 `SetEntriesInAclW` + 一次 `SetNamedSecurityInfoW` 完成，不依赖
/// 「`REVOKE_ACCESS` 后再 `GRANT_ACCESS`」的两步时序。合并始终以磁盘上的旧 DACL
/// 为底，继承 ACE 不会被重建丢失。
fn converge_ace(path: &Path, sid: PSID, mask: u32) -> Result<(), SandboxError> {
    let entries = read_entries(path)?;
    let has_same = entries.as_slice().iter().any(|e| {
        is_sandbox_entry(e, sid)
            && e.grfAccessMode == GRANT_ACCESS
            && e.grfAccessPermissions == mask
            && e.grfInheritance == INHERIT
    });
    if has_same {
        return Ok(());
    }

    let wide = to_wide(path);
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    let mut dacl: *mut ACL = std::ptr::null_mut();
    let status = unsafe {
        GetNamedSecurityInfoW(
            PCWSTR::from_raw(wide.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut dacl),
            None,
            &mut descriptor,
        )
    };
    if status.is_err() || dacl.is_null() {
        if !descriptor.0.is_null() {
            unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
        }
        return Err(SandboxError::SetupFailed("acl_reload_dacl".to_string()));
    }

    let desired = [desired_entry(sid, SET_ACCESS, mask)];
    let mut acl_final: *mut ACL = std::ptr::null_mut();
    let status =
        unsafe { SetEntriesInAclW(Some(&desired), Some(dacl as *const ACL), &mut acl_final) };
    unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
    if status.is_err() {
        return Err(SandboxError::SetupFailed("acl_converge".to_string()));
    }

    let status = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR::from_raw(wide.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(acl_final as *const ACL),
            None,
        )
    };
    unsafe { LocalFree(Some(HLOCAL(acl_final as *mut core::ffi::c_void))) };
    if status.is_err() {
        return Err(SandboxError::SetupFailed("acl_write_dacl".to_string()));
    }
    Ok(())
}

/// `spawn` 前的幂等准备：把可写根收敛为「唯一沙箱写授权 ACE」。
///
/// 只碰 `plan.writable_root`（规格 §5.1 / §6 硬约束 2）：敏感目录、系统目录、
/// 用户主目录一律不动——Windows 没有读边界，往这些路径写 deny ACE 只是无效的宿主副作用。
/// 可写根不可读 / 不存在即 `SetupFailed`（`acl_read_dacl`），**禁止**代建（A-S5）。
pub(crate) fn prepare(plan: &SandboxPlan) -> Result<(), SandboxError> {
    let sid = sandbox_sid()?;
    let result = converge_ace(&plan.writable_root, sid, WRITE_GRANT_MASK);
    free_sid(sid);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn plan_for(root: &Path) -> SandboxPlan {
        SandboxPlan {
            program: PathBuf::from("C:/Windows/System32/cmd.exe"),
            args: Vec::new(),
            cwd: root.to_path_buf(),
            writable_root: root.to_path_buf(),
            env: Vec::new(),
            network: false,
            stdin: None,
        }
    }

    fn count_sandbox_aces(path: &Path) -> usize {
        let sid = sandbox_sid().expect("sid");
        let entries = read_entries(path).expect("entries");
        let n = entries
            .as_slice()
            .iter()
            .filter(|e| is_sandbox_entry(e, sid))
            .count();
        free_sid(sid);
        n
    }

    fn sandbox_ace_mask(path: &Path) -> Option<u32> {
        let sid = sandbox_sid().expect("sid");
        let entries = read_entries(path).expect("entries");
        let mask = entries
            .as_slice()
            .iter()
            .find(|e| is_sandbox_entry(e, sid))
            .map(|e| e.grfAccessPermissions);
        free_sid(sid);
        mask
    }

    #[test]
    fn prepare_is_idempotent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("canonicalize");
        let plan = plan_for(&root);

        prepare(&plan).expect("first prepare");
        let after_first = count_sandbox_aces(&root);
        assert_eq!(after_first, 1, "可写根上应恰好一条沙箱 ACE");
        assert_eq!(
            sandbox_ace_mask(&root),
            Some(WRITE_GRANT_MASK),
            "掩码必须是写授权集合"
        );

        prepare(&plan).expect("second prepare");
        assert_eq!(
            count_sandbox_aces(&root),
            after_first,
            "重复准备不得叠加 ACE（A-S7）"
        );
        assert_eq!(sandbox_ace_mask(&root), Some(WRITE_GRANT_MASK));
    }

    #[test]
    fn prepare_replaces_stale_ace() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("canonicalize");
        let plan = plan_for(&root);

        let sid = sandbox_sid().expect("sid");
        converge_ace(&root, sid, FILE_GENERIC_WRITE_MASK).expect("stale grant");
        free_sid(sid);
        assert_eq!(count_sandbox_aces(&root), 1, "前置：旧掩码 ACE 已落盘");
        assert_eq!(sandbox_ace_mask(&root), Some(FILE_GENERIC_WRITE_MASK));

        prepare(&plan).expect("prepare");
        assert_eq!(count_sandbox_aces(&root), 1, "旧 ACE 必须被替换，不得叠加");
        assert_eq!(
            sandbox_ace_mask(&root),
            Some(WRITE_GRANT_MASK),
            "崩溃残留的旧掩码必须收敛到期望掩码"
        );
    }

    /// 掩码口径（规格 §5.1 / §12）：常量必须被字面值钉死。
    ///
    /// 只与常量自身比较等于没测——常量改错时那条断言照样通过。
    #[test]
    fn write_grant_mask_shape() {
        assert_eq!(
            WRITE_GRANT_MASK, 0x0011_0156,
            "字面值见规格 §5.1；改动必须同步规格"
        );
        assert_ne!(WRITE_GRANT_MASK & DELETE_MASK, 0, "须含 DELETE");
        assert_ne!(
            WRITE_GRANT_MASK & FILE_DELETE_CHILD_MASK,
            0,
            "须含 FILE_DELETE_CHILD"
        );
        assert_eq!(WRITE_GRANT_MASK & 0x0004_0000, 0, "禁止 WRITE_DAC");
        assert_eq!(WRITE_GRANT_MASK & 0x0008_0000, 0, "禁止 WRITE_OWNER");
        assert_eq!(
            WRITE_GRANT_MASK & STANDARD_RIGHTS_WRITE,
            0,
            "禁止 READ_CONTROL"
        );
    }
}
