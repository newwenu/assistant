use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use assistant_crypto::error::VaultError;
use log::{info, warn};
use zeroize::Zeroizing;

/// 剪贴板读写后端；生产用 `ArboardBackend`，测试可注入内存实现。
pub trait ClipboardBackend: Send + Sync {
    fn read(&self) -> Result<Option<String>, String>;
    fn write(&self, text: &str) -> Result<(), String>;
    fn clear(&self) -> Result<(), String>;
}

pub struct ArboardBackend;

impl ClipboardBackend for ArboardBackend {
    fn read(&self) -> Result<Option<String>, String> {
        let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        match clipboard.get_text() {
            Ok(text) => Ok(Some(text)),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn write(&self, text: &str) -> Result<(), String> {
        let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        #[cfg(target_os = "windows")]
        {
            use arboard::SetExtWindows;
            clipboard
                .set()
                .exclude_from_monitoring()
                .text(text)
                .map_err(|e| e.to_string())
        }
        #[cfg(target_os = "macos")]
        {
            use arboard::SetExtApple;
            clipboard
                .set()
                .exclude_from_history()
                .text(text)
                .map_err(|e| e.to_string())
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            clipboard.set_text(text).map_err(|e| e.to_string())
        }
    }

    fn clear(&self) -> Result<(), String> {
        let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        clipboard.clear().map_err(|e| e.to_string())
    }
}

pub struct ClipboardGuard<B: ClipboardBackend = ArboardBackend> {
    backend: Arc<B>,
    original: Arc<Mutex<Option<Zeroizing<String>>>>,
    current_password: Arc<Mutex<Option<Zeroizing<String>>>>,
}

impl ClipboardGuard<ArboardBackend> {
    pub fn new() -> Self {
        Self::with_backend(Arc::new(ArboardBackend))
    }
}

impl<B: ClipboardBackend + 'static> ClipboardGuard<B> {
    pub fn with_backend(backend: Arc<B>) -> Self {
        Self {
            backend,
            original: Arc::new(Mutex::new(None)),
            current_password: Arc::new(Mutex::new(None)),
        }
    }

    pub fn copy_and_guard(&self, password: &str, timeout_secs: u64) -> Result<(), VaultError> {
        // 已有在管的秘密先还原，避免把上一条秘密当成「用户原始内容」记录下来。
        if self.has_tracked_secret() {
            self.force_clear()?;
        }

        let original_content = match self.backend.read() {
            Ok(content) => content,
            Err(e) => {
                warn!("Failed to read clipboard before copy: {}", e);
                None
            }
        };

        self.backend
            .write(password)
            .map_err(VaultError::Clipboard)?;
        if timeout_secs > 0 {
            info!(
                "Vault secret copied to clipboard (auto-clear in {}s)",
                timeout_secs
            );
        } else {
            info!("Vault secret copied to clipboard (auto-clear disabled)");
        }

        {
            let mut orig = self.original.lock().unwrap_or_else(|e| e.into_inner());
            *orig = original_content.map(Zeroizing::new);
        }
        {
            let mut cur = self
                .current_password
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *cur = Some(Zeroizing::new(password.to_string()));
        }

        if timeout_secs == 0 {
            return Ok(());
        }

        let backend = Arc::clone(&self.backend);
        let original = Arc::clone(&self.original);
        let current_password = Arc::clone(&self.current_password);
        let password_owned = Zeroizing::new(password.to_string());
        let timeout = Duration::from_secs(timeout_secs);

        let _ = thread::spawn(move || {
            thread::sleep(timeout);

            let should_clear = {
                let cur = current_password.lock().unwrap_or_else(|e| e.into_inner());
                match cur.as_ref() {
                    Some(p) => p.as_str() == password_owned.as_str(),
                    None => false,
                }
            };

            if should_clear {
                let original_content = {
                    let mut orig = original.lock().unwrap_or_else(|e| e.into_inner());
                    orig.take()
                };
                match original_content {
                    Some(ref text) => {
                        let _ = backend.write(text.as_str());
                    }
                    None => {
                        let _ = backend.clear();
                    }
                }
                {
                    let mut cur = current_password.lock().unwrap_or_else(|e| e.into_inner());
                    *cur = None;
                }
            }
        });

        Ok(())
    }
}

impl<B: ClipboardBackend> ClipboardGuard<B> {
    fn has_tracked_secret(&self) -> bool {
        let cur = self
            .current_password
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        cur.is_some()
    }

    /// 还原/清空在管的秘密。没有在管秘密时不动剪贴板（避免抹掉用户自己的内容）。
    pub fn force_clear(&self) -> Result<(), VaultError> {
        let had_secret = {
            let mut cur = self
                .current_password
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            cur.take().is_some()
        };

        let original_content = {
            let mut orig = self.original.lock().unwrap_or_else(|e| e.into_inner());
            orig.take()
        };

        if !had_secret && original_content.is_none() {
            return Ok(());
        }

        match original_content {
            Some(original) => self
                .backend
                .write(original.as_str())
                .map_err(VaultError::Clipboard)?,
            None => self.backend.clear().map_err(|e| {
                warn!("Clipboard clear failed: {}", e);
                VaultError::Clipboard(e)
            })?,
        }

        info!("Clipboard force-cleared");
        Ok(())
    }
}

impl Default for ClipboardGuard<ArboardBackend> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B: ClipboardBackend> Drop for ClipboardGuard<B> {
    fn drop(&mut self) {
        if let Err(e) = self.force_clear() {
            warn!("Clipboard force_clear on drop failed: {}", e);
        }
    }
}
