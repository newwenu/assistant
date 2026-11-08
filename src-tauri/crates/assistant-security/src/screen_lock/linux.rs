use std::sync::{Arc, Mutex};

pub(super) fn linux_listen(
    locked: Arc<Mutex<bool>>,
    on_lock: impl Fn() + Send + Sync,
    on_unlock: impl Fn() + Send + Sync,
) {
    fn query_locked_hint() -> bool {
        use std::process::Command;
        let output = Command::new("loginctl")
            .args(["show-session", "-p", "LockedHint", "auto"])
            .output();
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                stdout.contains("yes")
            }
            Err(_) => false,
        }
    }

    let mut last = false;
    loop {
        let now = query_locked_hint();
        if now != last {
            if now {
                *locked.lock().unwrap_or_else(|e| e.into_inner()) = true;
                on_lock();
                log::info!("ScreenLockListener: screen locked (LockedHint=yes)");
            } else {
                *locked.lock().unwrap_or_else(|e| e.into_inner()) = false;
                on_unlock();
                log::info!("ScreenLockListener: screen unlocked (LockedHint=no)");
            }
            last = now;
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
