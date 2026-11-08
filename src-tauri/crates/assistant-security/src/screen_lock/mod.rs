use std::sync::{Arc, Mutex};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub struct ScreenLockMonitor {
    locked: Arc<Mutex<bool>>,
}

impl Default for ScreenLockMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenLockMonitor {
    pub fn new() -> Self {
        Self {
            locked: Arc::new(Mutex::new(false)),
        }
    }

    pub fn is_locked(&self) -> bool {
        *self.locked.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn spawn_listener(
        &self,
        on_lock: impl Fn() + Send + Sync + 'static,
        on_unlock: impl Fn() + Send + Sync + 'static,
    ) {
        let locked = Arc::clone(&self.locked);

        #[cfg(target_os = "windows")]
        {
            std::thread::spawn(move || {
                windows::windows_listen(locked, on_lock, on_unlock);
            });
        }

        #[cfg(target_os = "macos")]
        {
            std::thread::spawn(move || {
                macos::macos_listen(locked, on_lock, on_unlock);
            });
        }

        #[cfg(target_os = "linux")]
        {
            std::thread::spawn(move || {
                linux::linux_listen(locked, on_lock, on_unlock);
            });
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            let _ = (locked, on_lock, on_unlock);
            log::info!("Screen lock listener not supported on this platform");
        }

        log::info!("Screen lock listener spawned");
    }
}
