use std::sync::{Arc, Mutex};

pub(super) fn windows_listen(
    locked: Arc<Mutex<bool>>,
    on_lock: impl Fn() + Send + Sync + 'static,
    on_unlock: impl Fn() + Send + Sync + 'static,
) {
    // NOTE: 已知问题 — 在部分 Windows 环境下 WM_WTSSESSION_CHANGE 消息可能无法可靠到达，
    // 导致 Win+L 锁屏检测偶发失效。已参照 BongoCat/KeePassXC 的做法创建隐藏窗口并
    // 在 WM_NCCREATE 中设置 GWLP_USERDATA，但消息循环是否稳定接收仍依赖系统行为。
    // 用户锁屏后物理环境通常安全，vault 锁屏保护为辅助防线而非唯一防线。
    use windows::core::{w, GUID};
    use windows::Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        System::{
            LibraryLoader::GetModuleHandleW,
            Power::{
                PowerSettingUnregisterNotification, RegisterPowerSettingNotification,
                POWERBROADCAST_SETTING,
            },
            RemoteDesktop::{
                WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
                NOTIFY_FOR_THIS_SESSION,
            },
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
            PostQuitMessage, RegisterClassW, TranslateMessage, CREATESTRUCTW,
            DEVICE_NOTIFY_WINDOW_HANDLE, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_DESTROY,
            WM_NCCREATE, WM_NCDESTROY, WM_POWERBROADCAST, WM_WTSSESSION_CHANGE, WNDCLASSW,
        },
    };

    const WTS_SESSION_LOCK: u32 = 0x5;
    const WTS_SESSION_UNLOCK: u32 = 0x7;
    const WTS_CONSOLE_DISCONNECT: u32 = 0x2;
    const PBT_APMSUSPEND: u32 = 0x4;
    const PBT_APMRESUMEAUTOMATIC: u32 = 0x7;
    const PBT_POWERSETTINGCHANGE: u32 = 32787;
    const WINDOW_CLASS: windows::core::PCWSTR = w!("AssistantScreenLockWnd");

    static GUID_LIDSWITCH_STATE_CHANGE: GUID = GUID::from_values(
        0x4A4A0D5C,
        0x8C5E,
        0x4636,
        [0xA1, 0x6B, 0x8F, 0x5C, 0x75, 0x3B, 0x9C, 0x0E],
    );

    type StatePtr = *mut (
        Arc<Mutex<bool>>,
        Box<dyn Fn() + Send + Sync>,
        Box<dyn Fn() + Send + Sync>,
    );

    const GWLP_USERDATA: i32 = -21;

    #[link(name = "user32")]
    extern "system" {
        fn GetWindowLongPtrW(hwnd: HWND, nindex: i32) -> isize;
        fn SetWindowLongPtrW(hwnd: HWND, nindex: i32, newlong: isize) -> isize;
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_NCCREATE => {
                let create = lparam.0 as *const CREATESTRUCTW;
                if !create.is_null() {
                    let state = (*create).lpCreateParams as StatePtr;
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_WTSSESSION_CHANGE => {
                let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as StatePtr;
                if !state_ptr.is_null() {
                    let (locked, on_lock, on_unlock) = &*state_ptr;
                    match wparam.0 as u32 {
                        WTS_SESSION_LOCK | WTS_CONSOLE_DISCONNECT => {
                            let was = {
                                let mut guard = locked.lock().unwrap_or_else(|e| e.into_inner());
                                let was = *guard;
                                if !was {
                                    *guard = true;
                                }
                                was
                            };
                            if !was {
                                on_lock();
                            }
                        }
                        WTS_SESSION_UNLOCK => {
                            let was = {
                                let mut guard = locked.lock().unwrap_or_else(|e| e.into_inner());
                                let was = *guard;
                                if was {
                                    *guard = false;
                                }
                                was
                            };
                            if was {
                                on_unlock();
                            }
                        }
                        _ => {}
                    }
                }
                LRESULT(0)
            }
            WM_POWERBROADCAST => {
                let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as StatePtr;
                if !state_ptr.is_null() {
                    let (locked, on_lock, on_unlock) = &*state_ptr;
                    match wparam.0 as u32 {
                        PBT_APMSUSPEND => {
                            let was = {
                                let mut guard = locked.lock().unwrap_or_else(|e| e.into_inner());
                                let was = *guard;
                                if !was {
                                    *guard = true;
                                }
                                was
                            };
                            if !was {
                                on_lock();
                            }
                        }
                        PBT_APMRESUMEAUTOMATIC => {
                            let was = {
                                let mut guard = locked.lock().unwrap_or_else(|e| e.into_inner());
                                let was = *guard;
                                if was {
                                    *guard = false;
                                }
                                was
                            };
                            if was {
                                on_unlock();
                            }
                        }
                        PBT_POWERSETTINGCHANGE => {
                            let setting = lparam.0 as *const POWERBROADCAST_SETTING;
                            if !setting.is_null()
                                && (*setting).PowerSetting == GUID_LIDSWITCH_STATE_CHANGE
                            {
                                let lid_state = (*setting).Data.as_ptr() as *const u32;
                                if !lid_state.is_null() && *lid_state == 0 {
                                    let was = {
                                        let mut guard =
                                            locked.lock().unwrap_or_else(|e| e.into_inner());
                                        let was = *guard;
                                        if !was {
                                            *guard = true;
                                        }
                                        was
                                    };
                                    if !was {
                                        on_lock();
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                LRESULT(1)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    let instance = unsafe { GetModuleHandleW(None) }.unwrap_or_default();
    let hinstance = HINSTANCE(instance.0);

    let class = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance,
        lpszClassName: WINDOW_CLASS,
        ..Default::default()
    };

    if unsafe { RegisterClassW(&class) } == 0 {
        log::warn!("ScreenLockListener: RegisterClassW failed");
        return;
    }

    let state: StatePtr = Box::into_raw(Box::new((
        locked,
        Box::new(on_lock) as Box<dyn Fn() + Send + Sync>,
        Box::new(on_unlock) as Box<dyn Fn() + Send + Sync>,
    )));

    let hwnd = match unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            WINDOW_CLASS,
            w!("AssistantScreenLock"),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinstance),
            Some(state as *mut std::ffi::c_void),
        )
    } {
        Ok(h) => h,
        Err(e) => {
            log::warn!("ScreenLockListener: CreateWindowExW failed: {}", e);
            unsafe { drop(Box::from_raw(state)) };
            return;
        }
    };

    if unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) }.is_err() {
        log::warn!("ScreenLockListener: WTSRegisterSessionNotification failed");
        unsafe {
            let _ = DestroyWindow(hwnd);
            drop(Box::from_raw(state));
        }
        return;
    }

    let power_notify = match unsafe {
        RegisterPowerSettingNotification(
            hwnd.into(),
            &GUID_LIDSWITCH_STATE_CHANGE,
            DEVICE_NOTIFY_WINDOW_HANDLE,
        )
    } {
        Ok(h) => Some(h),
        Err(e) => {
            log::warn!(
                "ScreenLockListener: RegisterPowerSettingNotification failed: {}",
                e
            );
            None
        }
    };

    log::debug!("ScreenLockListener: hidden window created, listening for session changes");

    unsafe {
        let mut msg = std::mem::zeroed();
        loop {
            let ret = GetMessageW(&mut msg, None, 0, 0).0;
            if ret == 0 {
                break;
            }
            if ret == -1 {
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    unsafe {
        if let Some(h) = power_notify {
            let _ = PowerSettingUnregisterNotification(h);
        }
        let _ = WTSUnRegisterSessionNotification(hwnd);
        let _ = DestroyWindow(hwnd);
        drop(Box::from_raw(state));
    }
}
