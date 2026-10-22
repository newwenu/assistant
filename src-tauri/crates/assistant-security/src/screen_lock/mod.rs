use std::sync::{Arc, Mutex};

pub struct ScreenLockMonitor {
    locked: Arc<Mutex<bool>>,
}

impl ScreenLockMonitor {
    pub fn new() -> Self {
        Self {
            locked: Arc::new(Mutex::new(false)),
        }
    }

    pub fn is_locked(&self) -> bool {
        *self.locked.lock().unwrap()
    }

    pub fn spawn_listener(&self, on_lock: impl Fn() + Send + Sync + 'static) {
        let locked = Arc::clone(&self.locked);

        #[cfg(target_os = "windows")]
        {
            std::thread::spawn(move || {
                windows_listen(locked, on_lock);
            });
        }

        #[cfg(target_os = "macos")]
        {
            std::thread::spawn(move || {
                macos_listen(locked, on_lock);
            });
        }

        #[cfg(target_os = "linux")]
        {
            std::thread::spawn(move || {
                linux_listen(locked, on_lock);
            });
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            let _ = (locked, on_lock);
            log::info!("Screen lock listener not supported on this platform");
        }

        log::info!("Screen lock listener spawned");
    }
}

#[cfg(target_os = "windows")]
fn windows_listen(locked: Arc<Mutex<bool>>, on_lock: impl Fn() + Send + Sync) {
    use std::ptr;

    type HWND = *mut std::ffi::c_void;

    #[link(name = "wtsapi32")]
    extern "system" {
        fn WTSRegisterSessionNotification(hwnd: HWND, flags: u32) -> i32;
        fn WTSUnRegisterSessionNotification(hwnd: HWND) -> i32;
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> HWND;
        fn GetMessageW(lpmsg: *mut u8, hwnd: HWND, wmsgfiltermin: u32, wmsgfiltermax: u32) -> i32;
    }

    const NOTIFY_FOR_THIS_SESSION: u32 = 0;
    const WM_WTSSESSION_CHANGE: u32 = 0x02B1;
    const WTS_SESSION_LOCK: u32 = 0x5;
    const WM_POWERBROADCAST: u32 = 0x0218;
    const PBT_APMSUSPEND: u32 = 0x4;
    const MSG_SIZE: usize = 48;

    let hwnd = unsafe { GetForegroundWindow() };

    if hwnd.is_null() {
        log::warn!("ScreenLockListener: no foreground window handle");
        return;
    }

    unsafe {
        WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
    }

    let mut msg_buf = [0u8; MSG_SIZE];
    loop {
        let ret = unsafe { GetMessageW(msg_buf.as_mut_ptr(), ptr::null_mut(), 0, 0) };
        if ret == 0 {
            break;
        }

        let message = u32::from_ne_bytes(msg_buf[8..12].try_into().unwrap_or([0; 4]));
        let wparam = usize::from_ne_bytes(msg_buf[16..24].try_into().unwrap_or([0; 8]));

        if message == WM_WTSSESSION_CHANGE && wparam as u32 == WTS_SESSION_LOCK {
            *locked.lock().unwrap() = true;
            on_lock();
        } else if message == WM_POWERBROADCAST && wparam as u32 == PBT_APMSUSPEND {
            *locked.lock().unwrap() = true;
            on_lock();
        }
    }

    unsafe {
        WTSUnRegisterSessionNotification(hwnd);
    }
}

#[cfg(target_os = "macos")]
fn macos_listen(locked: Arc<Mutex<bool>>, on_lock: impl Fn() + Send + Sync) {
    use std::process::Command;

    loop {
        let output = Command::new("python3")
            .arg("-c")
            .arg("import Quartz; print(Quartz.CGSessionScreenIsBlank())")
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if stdout == "True" || stdout == "1" {
                let was_locked = *locked.lock().unwrap();
                if !was_locked {
                    *locked.lock().unwrap() = true;
                    on_lock();
                }
            } else {
                *locked.lock().unwrap() = false;
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}

#[cfg(target_os = "linux")]
fn linux_listen(locked: Arc<Mutex<bool>>, on_lock: impl Fn() + Send + Sync) {
    use std::process::Command;

    loop {
        let output = Command::new("loginctl")
            .args(["show-session", "-p", "IdleHint", "auto"])
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.contains("yes") {
                let was_locked = *locked.lock().unwrap();
                if !was_locked {
                    *locked.lock().unwrap() = true;
                    on_lock();
                }
            } else {
                *locked.lock().unwrap() = false;
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}
