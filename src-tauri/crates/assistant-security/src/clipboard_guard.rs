use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::types::VaultError;
use log::{info, warn};

pub struct ClipboardGuard {
    original: Arc<Mutex<Option<String>>>,
    current_password: Arc<Mutex<Option<String>>>,
}

impl Default for ClipboardGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardGuard {
    pub fn new() -> Self {
        Self {
            original: Arc::new(Mutex::new(None)),
            current_password: Arc::new(Mutex::new(None)),
        }
    }

    pub fn copy_and_guard(&self, password: &str, timeout_secs: u64) -> Result<(), VaultError> {
        self.force_clear()?;

        let original_content = match read_clipboard() {
            Ok(content) => content,
            Err(e) => {
                warn!("Failed to read clipboard before copy: {}", e);
                None
            }
        };

        write_clipboard(password)?;
        info!(
            "Vault secret copied to clipboard (auto-clear in {}s)",
            timeout_secs
        );

        {
            let mut orig = self.original.lock().unwrap();
            *orig = original_content;
        }
        {
            let mut cur = self.current_password.lock().unwrap();
            *cur = Some(password.to_string());
        }

        let original = Arc::clone(&self.original);
        let current_password = Arc::clone(&self.current_password);
        let password_owned = password.to_string();
        let timeout = Duration::from_secs(timeout_secs);

        let _ = thread::spawn(move || {
            thread::sleep(timeout);

            let should_clear = {
                let cur = current_password.lock().unwrap();
                match cur.as_ref() {
                    Some(p) => p == &password_owned,
                    None => false,
                }
            };

            if should_clear {
                let original_content = {
                    let mut orig = original.lock().unwrap();
                    orig.take()
                };
                if let Some(ref text) = original_content {
                    let _ = write_clipboard_internal(text);
                } else {
                    let _ = clear_clipboard_internal();
                }
                {
                    let mut cur = current_password.lock().unwrap();
                    *cur = None;
                }
            }
        });

        Ok(())
    }

    pub fn force_clear(&self) -> Result<(), VaultError> {
        {
            let mut cur = self.current_password.lock().unwrap();
            *cur = None;
        }

        let original_content = {
            let mut orig = self.original.lock().unwrap();
            orig.take()
        };

        if let Some(original) = original_content {
            write_clipboard(&original)?;
        } else {
            clear_clipboard_internal().map_err(|e| {
                warn!("Clipboard clear failed: {}", e);
                VaultError::Clipboard(e.to_string())
            })?;
        }

        info!("Clipboard force-cleared");
        Ok(())
    }
}

fn read_clipboard() -> Result<Option<String>, VaultError> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| VaultError::Clipboard(e.to_string()))?;
    match clipboard.get_text() {
        Ok(text) => Ok(Some(text)),
        Err(arboard::Error::ContentNotAvailable) => Ok(None),
        Err(e) => Err(VaultError::Clipboard(e.to_string())),
    }
}

fn write_clipboard(text: &str) -> Result<(), VaultError> {
    write_clipboard_internal(text).map_err(|e| VaultError::Clipboard(e.to_string()))
}

fn write_clipboard_internal(text: &str) -> Result<(), arboard::Error> {
    let mut clipboard = arboard::Clipboard::new()?;
    #[cfg(target_os = "windows")]
    {
        use arboard::SetExtWindows;
        clipboard.set().exclude_from_monitoring().text(text)?;
    }
    #[cfg(target_os = "macos")]
    {
        use arboard::SetExtApple;
        clipboard.set().exclude_from_history().text(text)?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        clipboard.set_text(text)?;
    }
    Ok(())
}

fn clear_clipboard_internal() -> Result<(), arboard::Error> {
    let mut clipboard = arboard::Clipboard::new()?;
    clipboard.clear()?;
    Ok(())
}
