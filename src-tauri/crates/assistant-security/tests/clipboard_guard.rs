use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use assistant_security::clipboard_guard::{ClipboardBackend, ClipboardGuard};

#[derive(Clone, Default)]
struct MockBackend {
    content: Arc<Mutex<Option<String>>>,
}

impl MockBackend {
    fn with_initial(text: &str) -> Self {
        Self {
            content: Arc::new(Mutex::new(Some(text.to_string()))),
        }
    }

    fn snapshot(&self) -> Option<String> {
        self.content.lock().unwrap().clone()
    }
}

impl ClipboardBackend for MockBackend {
    fn read(&self) -> Result<Option<String>, String> {
        Ok(self.snapshot())
    }

    fn write(&self, text: &str) -> Result<(), String> {
        *self.content.lock().unwrap() = Some(text.to_string());
        Ok(())
    }

    fn clear(&self) -> Result<(), String> {
        *self.content.lock().unwrap() = None;
        Ok(())
    }
}

/// T-R5：复制后锁定（`force_clear`）→ 剪贴板恢复原内容，内部记录被清空。
#[test]
fn t_r5_force_clear_restores_original_clipboard() {
    let backend = MockBackend::with_initial("original-text");
    let guard = ClipboardGuard::with_backend(Arc::new(backend.clone()));

    guard.copy_and_guard("secret-password", 3600).unwrap();
    assert_eq!(backend.snapshot().as_deref(), Some("secret-password"));

    guard.force_clear().unwrap();
    assert_eq!(
        backend.snapshot().as_deref(),
        Some("original-text"),
        "force_clear 必须把剪贴板恢复到复制前的内容"
    );

    // 已无在管秘密时再调用是幂等的，不会误动用户自己的剪贴板内容。
    guard.force_clear().unwrap();
    assert_eq!(backend.snapshot().as_deref(), Some("original-text"));
}

/// T-R5：自动清理到期后同样不得残留秘密。
#[test]
fn t_r5_auto_clear_after_timeout_removes_secret() {
    let backend = MockBackend::with_initial("original-text");
    let guard = ClipboardGuard::with_backend(Arc::new(backend.clone()));

    guard.copy_and_guard("secret-password", 1).unwrap();
    assert_eq!(backend.snapshot().as_deref(), Some("secret-password"));

    thread::sleep(Duration::from_millis(1200));
    assert_eq!(
        backend.snapshot().as_deref(),
        Some("original-text"),
        "超时后必须恢复原内容而不是保留秘密"
    );
    drop(guard);
}

/// T-C5：`vault_clipboard_clear_seconds = 0` → 不自动清除（锁定仍会兜底清理）。
#[test]
fn t_c5_zero_timeout_disables_auto_clear() {
    let backend = MockBackend::with_initial("original-text");
    let guard = ClipboardGuard::with_backend(Arc::new(backend.clone()));

    guard.copy_and_guard("secret-password", 0).unwrap();
    thread::sleep(Duration::from_millis(150));
    assert_eq!(
        backend.snapshot().as_deref(),
        Some("secret-password"),
        "0 表示关闭自动清除，超时后不得被动清理"
    );

    guard.force_clear().unwrap();
    assert_eq!(
        backend.snapshot().as_deref(),
        Some("original-text"),
        "锁定/退出路径仍必须兜底恢复剪贴板"
    );
}

/// T-R5：`Drop` 也会清理剪贴板（进程内最后一处引用释放时）。
#[test]
fn t_r5_drop_clears_secret() {
    let backend = MockBackend::default();
    {
        let guard = ClipboardGuard::with_backend(Arc::new(backend.clone()));
        guard.copy_and_guard("secret-password", 3600).unwrap();
        assert_eq!(backend.snapshot().as_deref(), Some("secret-password"));
    }
    assert_eq!(backend.snapshot(), None, "guard 释放后剪贴板不得保留秘密");
}

/// 复制新秘密时先清掉旧秘密，避免两条秘密同时存在于后端记录里。
#[test]
fn copy_twice_keeps_single_tracked_secret() {
    let backend = MockBackend::with_initial("original-text");
    let guard = ClipboardGuard::with_backend(Arc::new(backend.clone()));

    guard.copy_and_guard("first-secret", 3600).unwrap();
    guard.copy_and_guard("second-secret", 3600).unwrap();
    assert_eq!(backend.snapshot().as_deref(), Some("second-secret"));

    guard.force_clear().unwrap();
    assert_eq!(
        backend.snapshot().as_deref(),
        Some("original-text"),
        "两次复制后原始内容仍应被正确恢复"
    );
}
