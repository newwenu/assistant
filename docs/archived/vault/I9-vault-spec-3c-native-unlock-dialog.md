# I9-3c — 原生解锁对话框（Native Unlock Dialog）

> 本文档为 I9 密码管理箱规格的补充，覆盖原生 Win32 解锁对话框的设计与实现。
> 前置文档：[I9-vault-spec-3-security-enhance.md](./I9-vault-spec-3-security-enhance.md)（§7.7 独立解锁窗口）
> 涉及文件：
- `crates/assistant-security/src/unlock_dialog/mod.rs`
- `crates/assistant-security/src/unlock_dialog/windows.rs`
- `src/cmd/vault/lifecycle.rs`

---

## 1. 设计定位

### 1.1 职责

原生解锁对话框是**安全密码采集控件**，唯一职责是在独立于 Tauri Webview 的原生窗口中安全接收主密码输入。

```
调用方 (lifecycle.rs / setup.rs)
  │
  │  需要安全采集密码
  ▼
┌──────────────────────────────────┐
│  Native Unlock Dialog            │
│  职责：安全采集主密码             │
│                                  │
│  输出：Confirmed                 │
│       或 Cancelled               │
└──────────────────────────────────┘
  │
  │  回调通知结果
  ▼
调用方自行决定后续行为：
  - 调用 vault_unlock(password)
  - 错误处理 / 重试
  - emit 事件 / SSH / 审计
```

### 1.2 与 Tauri WebviewWindow 解锁窗口的区别

| 维度 | Tauri WebviewWindow (VaultUnlock.tsx) | Native Win32 Dialog |
|------|--------------------------------------|---------------------|
| 运行环境 | V8 isolate，依赖 Tauri runtime | 纯 Win32 API，无 runtime 依赖 |
| 功能范围 | 完整解锁流程（密码/生物识别/恢复密钥/灾难恢复） | 仅主密码输入 |
| 触发场景 | 用户主动操作 | vault 锁定时需要解锁入口（如设置页启用生物识别） |
| 安全优势 | V8 isolate 隔离 | 无 JS 运行时，攻击面更小 |

### 1.3 不做的事

- ❌ 调用 `vault_unlock` — 调用方的事
- ❌ 显示解锁错误 / 重试逻辑 — 调用方的事
- ❌ emit 事件 / 审计日志 — 调用方的事
- ❌ 生物识别 / 恢复密钥 — 非密码采集职责

---

## 2. 抽象接口

### 2.1 UnlockResult

```rust
pub enum UnlockResult {
    Confirmed(SecureString),
    Cancelled,
}
```

`Confirmed` 携带 `SecureString`，调用方通过 `expose_str()` 获取密码引用。`SecureString` 的 `Drop` 实现 zeroize 内部 `String` 的堆缓冲区，确保密码离开作用域后从内存中清除。

> `SecureString` 定义于 `vault_crypto::types`，已在项目中广泛使用（`decrypt_password`、`decrypt_notes` 等），drop 时 zeroize `String.as_bytes_mut()`。

### 2.2 UnlockDialog trait

```rust
pub trait UnlockDialog: Send + Sync {
    fn show(&self, on_result: Box<dyn FnOnce(UnlockResult) + Send>) -> Result<(), VaultError>;
}
```

变化：
- 回调参数从 `Result<bool, VaultError>` 改为 `UnlockResult`
- 去掉 `Err` 路径：窗口创建失败由 `show` 的 `Result` 返回；运行时用户取消是正常流程（`Cancelled`），不是错误
- `Confirmed(SecureString)` 携带密码，调用方负责使用后让 `SecureString` 自然 drop 以触发 zeroize

### 2.3 工厂函数

```rust
pub fn create_unlock_dialog() -> Result<Box<dyn UnlockDialog>, VaultError>;
```

不变。

---

## 3. Win32 实现规格

### 3.1 窗口属性

| 属性 | 值 | 说明 |
|------|---|------|
| 窗口类 | `AssistantUnlockDlg` | 已注册，不变 |
| 标题 | `Vault Unlock` | 不变 |
| 样式 | `WS_OVERLAPPED \| WS_CAPTION \| WS_SYSMENU \| WS_VISIBLE` | 标题栏 + 关闭按钮，不可调整大小 |
| 扩展样式 | `WS_EX_DLGMODALFRAME \| WS_EX_TOPMOST \| WS_EX_CONTROLPARENT` | `WS_EX_TOPMOST` 屏幕锁定后置顶可见；`WS_EX_CONTROLPARENT` 支持 Tab 导航 |
| 尺寸 | 380 × 150（96 DPI 基准） | DPI 缩放详见 §7 |
| 位置 | 屏幕居中 | `GetSystemMetrics(SM_CXSCREEN/SM_CYSCREEN)` 计算 |

### 3.2 控件布局

```
┌──────────────────────────────────────────────┐
│  Vault Unlock                          [×]   │
│                                              │
│  Enter master password:                      │
│  ┌────────────────────────────────────────┐  │
│  │ ••••••••••••••••••••••••••••••  👁 │  │
│  └────────────────────────────────────────┘  │
│                                              │
│              [ Unlock ]    [ Close ]         │
│                                              │
└──────────────────────────────────────────────┘
  👁 = 可见性图标，直接绘制在 EDIT 控件内部右端
      始终可见，点击切换掩码/明文（Toggle 模式）
```

| 控件 | ID | 样式 | 位置 | 说明 |
|------|----|------|------|------|
| 标题标签 | `ID_STATIC_TITLE` (301) | `WS_CHILD \| WS_VISIBLE \| WS_CLIPSIBLINGS` | (14, 10, 344, 18) | 宽度跟随输入框 |
| 密码输入框 | `ID_EDIT_PASSWORD` (101) | `WS_CHILD \| WS_VISIBLE \| WS_BORDER \| WS_TABSTOP \| ES_PASSWORD \| ES_AUTOHSCROLL` | (14, 36, 344, 24) | 右侧 26px 留给图标区域（`EM_SETMARGINS`） |
| Unlock 按钮 | `ID_BTN_UNLOCK` (201) | `WS_CHILD \| WS_VISIBLE \| WS_TABSTOP \| BS_DEFPUSHBUTTON` | (192, 72, 80, 26) | 默认按钮，响应 Enter |
| Close 按钮 | `ID_BTN_CLOSE` (202) | `WS_CHILD \| WS_VISIBLE \| WS_TABSTOP` | (280, 72, 80, 26) | 不变 |

布局计算：
- 输入框右边缘：14 + 344 = 358，距客户区右边缘（~372）14px，与左 margin 对称
- 图标区域：输入框右端 26px（`ICON_RIGHT_MARGIN = 26`），由 `EM_SETMARGINS(EC_RIGHTMARGIN)` 留出，文字不会覆盖图标
- 图标垂直居中：`DrawTextW` 使用 `DT_CENTER | DT_VCENTER | DT_SINGLELINE`
- 底部按钮右边缘：280 + 80 = 360，与输入框右边缘（358）近似对齐

移除控件：
- `ID_STATIC_ERR` (401) — 错误显示不是本窗口职责
- `ID_BTN_TOGGLE_VIS` (501) — 可见性图标不再使用独立 BUTTON 控件，改为直接绘制在 EDIT 内部

### 3.3 密码掩码

| 行为 | 实现 |
|------|------|
| 默认状态 | `ES_PASSWORD` 样式，系统默认掩码字符（`●` on Windows 10+） |
| 点击图标切换明文 | `EM_SETPASSWORDCHAR(0)` — 设置密码字符为 0，EDIT 显示明文 |
| 点击图标恢复掩码 | `EM_SETPASSWORDCHAR(0x25CF)` — 设置密码字符为 `●`，EDIT 恢复掩码 |
| 切换后刷新 | 清空文本 → 重设文本 → `InvalidateRect` 整个 EDIT 重绘 |

> **设计决策**：使用 `EM_SETPASSWORDCHAR` 消息而非 `SetWindowLongPtrW` 修改 `ES_PASSWORD` 样式位。原因：EDIT 控件内部缓存了密码字符，运行时修改样式位不会刷新缓存，必须通过消息通知控件。

> **交互模式**：Toggle 切换（点击切换，再次点击恢复），而非 press-and-hold。图标始终可见，用户可随时点击。

### 3.4 键盘交互

| 按键 | 行为 |
|------|------|
| Enter | 等同点击 Unlock 按钮（`BS_DEFPUSHBUTTON` + `IsDialogMessageW`） |
| Escape | 等同点击 Close 按钮（`WM_KEYDOWN` + `VK_ESCAPE` → `PostMessageW WM_CLOSE`） |
| Tab | 在控件间导航（`WS_EX_CONTROLPARENT` + `IsDialogMessageW`） |

### 3.5 窗口居中

```rust
let cx = GetSystemMetrics(SM_CXSCREEN);
let cy = GetSystemMetrics(SM_CYSCREEN);
let x = (cx - WINDOW_WIDTH) / 2;
let y = (cy - WINDOW_HEIGHT) / 2;
```

### 3.6 DialogState

```rust
struct DialogState {
    on_result: Option<Box<dyn FnOnce(UnlockResult) + Send>>,
    closed: bool,
    password_visible: bool,   // 当前密码是否为明文状态
}
```

字段说明：
- `password_visible`：`true` 时密码显示为明文，`false` 时显示为掩码。Toggle 切换时取反。

### 3.7 消息处理

| 消息 | 处理 |
|------|------|
| `WM_NCCREATE` | 从 `CREATESTRUCTW` 提取 `DialogState` 指针，存入 `GWLP_USERDATA` |
| `WM_CREATE` | 创建所有子控件，设置字体，子类化 EDIT 控件，设置 `EM_SETMARGINS` 右边距 |
| `WM_COMMAND` | 按钮点击：Unlock → 读取密码 → 回调 `Confirmed` → 销毁窗口；Close → 回调 `Cancelled` → 销毁窗口 |
| `WM_KEYDOWN` | `VK_ESCAPE` → `PostMessageW WM_CLOSE` |
| `WM_CLOSE` | 回调 `Cancelled` → 销毁窗口 |
| `WM_DESTROY` | 清空 EDIT 文本 + 删除图标字体 + `PostQuitMessage(0)` |

### 3.8 EDIT 子类化

**核心设计**：可见性图标直接绘制在 EDIT 控件内部，而非使用叠加的 BUTTON 子窗口。通过子类化 EDIT 控件拦截关键消息。

#### 3.8.1 子类化安装

```rust
static mut ORIGINAL_EDIT_PROC: Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT> = None;

// WM_CREATE 中，创建 EDIT 后：
let orig_proc = SetWindowLongPtrW(hedit, GWLP_WNDPROC, edit_subclass_proc as isize);
ORIGINAL_EDIT_PROC = Some(std::mem::transmute::<isize, _>(orig_proc));
```

#### 3.8.2 子类化消息处理

```rust
unsafe extern "system" fn edit_subclass_proc(
    hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let result = call_original_edit_proc(hwnd, msg, wparam, lparam);
            draw_visibility_icon(hwnd);  // 默认绘制完成后叠加图标
            result
        }
        WM_LBUTTONDOWN => {
            if is_in_icon_area(hwnd, get_x_lparam(lparam), get_y_lparam(lparam)) {
                let parent = get_parent_dialog(hwnd);
                toggle_password_visibility(parent);
                return LRESULT(0);  // 消费消息，不传递到 EDIT
            }
            call_original_edit_proc(hwnd, msg, wparam, lparam)
        }
        WM_SETCURSOR => {
            if is_cursor_in_icon_area(hwnd) {
                SetCursor(Some(LoadCursorW(None, IDC_HAND)));
                return LRESULT(1);  // 已处理，阻止默认光标
            }
            call_original_edit_proc(hwnd, msg, wparam, lparam)
        }
        _ => call_original_edit_proc(hwnd, msg, wparam, lparam),
    }
}
```

| 拦截消息 | 作用 |
|----------|------|
| `WM_PAINT` | 默认绘制完成后调用 `draw_visibility_icon` 叠加图标 |
| `WM_LBUTTONDOWN` | 点击位置在图标区域时触发 `toggle_password_visibility`，消费消息 |
| `WM_SETCURSOR` | 鼠标在图标区域时显示手型光标（`IDC_HAND`） |

#### 3.8.3 父对话框查找

EDIT 子类化 proc 中需要访问 `DialogState`，但 `GWLP_USERDATA` 存储在父对话框窗口上。`get_parent_dialog` 通过向上遍历窗口层级查找：

```rust
unsafe fn get_parent_dialog(hwnd: HWND) -> HWND {
    let mut parent = hwnd;
    for _ in 0..10 {
        // 检查是否有 DialogState（GWLP_USERDATA 非空）
        let owner = GetWindowLongPtrW(parent, GWLP_USERDATA) as DialogStatePtr;
        if !owner.is_null() { return parent; }
        // 检查是否有标题栏（对话框特征）
        let style = GetWindowLongPtrW(parent, GWL_STYLE) as u32;
        if (style & (WS_CAPTION.0 as u32)) != 0 { return parent; }
        // 继续向上
        let p = GetAncestor(parent, GA_PARENT);
        if p.is_invalid() || p == parent { break; }
        parent = p;
    }
    parent
}
```

### 3.9 可见性图标绘制

#### 3.9.1 图标区域计算

```rust
const ICON_RIGHT_MARGIN: i32 = 26;

unsafe fn get_icon_rect(hwnd: HWND) -> RECT {
    let mut rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut rect);
    RECT {
        left: rect.right - ICON_RIGHT_MARGIN,
        top: 0,
        right: rect.right - 2,
        bottom: rect.bottom,
    }
}
```

图标占据 EDIT 客户区右端 26px（左侧 2px 间距），垂直填满。

#### 3.9.2 绘制实现

```rust
unsafe fn draw_visibility_icon(hwnd: HWND) {
    let dc = GetDC(Some(hwnd));
    // 1. 选择 Segoe UI Emoji 字体
    let old_font = SelectObject(dc, ICON_FONT.into());
    // 2. 设置透明背景 + 灰色文字
    SetBkMode(dc, BACKGROUND_MODE(BKMODE_TRANSPARENT));
    SetTextColor(dc, COLORREF(GetSysColor(COLOR_GRAYTEXT)));
    // 3. 根据状态选择图标
    let icon_text = if password_visible { "🙈" } else { "👁" };
    // 4. DrawTextW 居中绘制
    DrawTextW(dc, &mut icon_text, &mut draw_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    // 5. 恢复旧字体
    SelectObject(dc, old_font);
    ReleaseDC(Some(hwnd), dc);
}
```

| 绘制参数 | 值 | 说明 |
|----------|---|------|
| 字体 | `Segoe UI Emoji`，`lfHeight = -14` | 支持 Emoji 渲染 |
| 背景模式 | `TRANSPARENT` | 不覆盖 EDIT 背景 |
| 文字颜色 | `GetSysColor(COLOR_GRAYTEXT)` | 系统禁用文字色，视觉上不抢眼 |
| 图标文本 | 密码掩码时 `👁` (U+1F441)，明文时 `🙈` (U+1F648) | 眼睛图标表示"可点击查看" |

#### 3.9.3 图标字体

```rust
static mut ICON_FONT: HFONT = HFONT(std::ptr::null_mut());

fn create_icon_font() -> HFONT {
    let mut lf = LOGFONTW::default();
    lf.lfHeight = -14;
    lf.lfWeight = 400;
    let face: Vec<u16> = "Segoe UI Emoji\0".encode_utf16().collect();
    lf.lfFaceName[..copy_len].copy_from_slice(&face[..copy_len]);
    unsafe { CreateFontIndirectW(&lf) }
}
```

字体在 `WM_CREATE` 中创建，在 `WM_DESTROY` 中 `DeleteObject` 释放。

### 3.10 密码可见性切换

```rust
unsafe fn toggle_password_visibility(hwnd: HWND) {
    // 1. 切换状态
    state.password_visible = !state.password_visible;

    // 2. 通过 EM_SETPASSWORDCHAR 消息切换
    if state.password_visible {
        SendMessageW(hedit, EM_SETPASSWORDCHAR, WPARAM(0), LPARAM(0));      // 明文
    } else {
        SendMessageW(hedit, EM_SETPASSWORDCHAR, WPARAM(0x25CF), LPARAM(0)); // 掩码 ●
    }

    // 3. 刷新显示：清空 → 重设文本 → 强制重绘
    let text = read_edit_text(hedit);
    SetWindowTextW(hedit, w!(""));
    SetWindowTextW(hedit, &text_wide);
    InvalidateRect(Some(hedit), None, true);
}
```

> **为什么用 `EM_SETPASSWORDCHAR` 而非 `SetWindowLongPtrW`**：`SetWindowLongPtrW` 修改 `ES_PASSWORD` 样式位后，EDIT 控件内部缓存的密码字符不会更新，显示不生效。`EM_SETPASSWORDCHAR` 是官方推荐的运行时切换方式。

### 3.11 密码安全读取与传递

密码从 Win32 EDIT 控件到调用方回调的完整安全流程：

```
┌─ Win32 Dialog Thread ──────────────────────────────────────────────┐
│                                                                    │
│  ① GetWindowTextW → Vec<u16> (UTF-16 缓冲区)                      │
│  ② String::from_utf16_lossy → String (UTF-8)                      │
│  ③ SecureString::new(string) → SecureString                       │
│  ④ buf.zeroize() — 清零 Vec<u16> 中间缓冲区                       │
│  ⑤ clear_edit_text() — 清空 EDIT 控件文本                         │
│  ⑥ EM_EMPTYUNDOBUFFER — 清空 EDIT 控件 undo 缓冲区               │
│  ⑦ on_result(UnlockResult::Confirmed(secure_string))              │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
  │
  ▼  回调在 dialog 线程上执行
┌─ Caller ───────────────────────────────────────────────────────────┐
│  ⑧ secure_string.expose_str() → &str  (使用密码)                   │
│  ⑨ vault_unlock(password) / 其他操作                               │
│  ⑩ SecureString drop → String.as_bytes_mut().zeroize()            │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
```

#### 实现代码

```rust
fn read_edit_text_secure(hwnd: HWND, id: u16) -> SecureString {
    let hedit = unsafe { GetDlgItem(hwnd, id as i32) };
    if hedit.is_invalid() {
        return SecureString::new(String::new());
    }
    let len = unsafe { GetWindowTextLengthW(hedit) } as usize;
    if len == 0 {
        return SecureString::new(String::new());
    }
    let mut buf = vec![0u16; len + 1];
    let copied = unsafe { GetWindowTextW(hedit, &mut buf) };
    if copied == 0 {
        buf.zeroize();
        return SecureString::new(String::new());
    }
    let password = String::from_utf16_lossy(&buf[..copied as usize]);
    buf.zeroize();
    SecureString::new(password)
}
```

#### 安全分析

| 风险点 | 缓解措施 |
|--------|----------|
| `Vec<u16>` 中间缓冲区 | 读取后显式 `buf.zeroize()`，drop 前已清零 |
| `String` 堆缓冲区 | `SecureString` 的 `Drop` 实现 `as_bytes_mut().zeroize()` |
| EDIT 控件内部缓冲区 | `clear_edit_text()` 清空文本 + `EM_EMPTYUNDOBUFFER` 清空 undo |
| EDIT 控件 IME 组合缓冲区 | Windows 在 `SetWindowTextW("")` 时清理，无额外处理 |
| `SecureString` 在回调中驻留 | `FnOnce` 语义保证只调用一次；调用方使用后让 `SecureString` 自然 drop |
| 密码跨线程传递 | `SecureString` 内含 `String`，`String` 是 `Send`，因此 `SecureString` 是 `Send`；回调 `FnOnce(UnlockResult) + Send` 允许跨线程 |

### 3.12 窗口销毁与资源回收

#### 销毁触发路径

```
路径 A: 用户点击 Unlock
  ID_BTN_UNLOCK → 读取密码 → cb(Confirmed) → closed=true → DestroyWindow → WM_DESTROY → PostQuitMessage

路径 B: 用户点击 Close / × / Escape
  ID_BTN_CLOSE / WM_CLOSE → cb(Cancelled) → closed=true → DestroyWindow → WM_DESTROY → PostQuitMessage

路径 C: 外部销毁（进程退出等）
  WM_DESTROY → PostQuitMessage → 消息循环退出 → 检查 !closed → cb(Cancelled)
```

#### WM_DESTROY 安全清理

```rust
WM_DESTROY => {
    clear_edit_text(hwnd, ID_EDIT_PASSWORD);
    if !ICON_FONT.0.is_null() {
        let _ = DeleteObject(ICON_FONT.into());
        ICON_FONT = HFONT(std::ptr::null_mut());
    }
    PostQuitMessage(0);
    LRESULT(0)
}
```

#### DialogState 生命周期

```
创建:  Box::into_raw(DialogState) → lpCreateParams → WM_NCCREATE → SetWindowLongPtrW(GWLP_USERDATA)
使用:  GetWindowLongPtrW(GWLP_USERDATA) → &mut *state_ptr → take(on_result)
回收:  消息循环退出后 → Box::from_raw(state_ptr) → drop
```

关键不变量：
- `on_result` 是 `Option<Box<dyn FnOnce>>`，`take()` 保证回调只执行一次
- `closed` 标志防止消息循环后的兜底路径重复回调
- `Box::from_raw` 保证 `DialogState` 内存被正确回收

### 3.13 消息循环

使用 `IsDialogMessageW` 处理对话框消息，确保 Tab 导航和默认按钮行为：

```rust
loop {
    let ret = GetMessageW(&mut msg, None, 0, 0).0;
    if ret == 0 || ret == -1 { break; }
    if !IsDialogMessageW(hwnd, &msg) {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}
```

主窗口需 `WS_EX_CONTROLPARENT` 扩展样式。

---

## 4. 调用方适配

### 4.1 lifecycle.rs

```rust
pub fn vault_show_unlock_window(_app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use assistant_security::unlock_dialog::{create_unlock_dialog, UnlockResult};

        let dialog = create_unlock_dialog().map_err(|e| e.to_string())?;
        dialog
            .show(Box::new(move |result| {
                match result {
                    UnlockResult::Confirmed(password) => {
                        log::info!("Native unlock dialog: user confirmed");
                        // 调用方在此使用 password.expose_str() 执行解锁
                        // password 离开作用域时自动 zeroize
                    }
                    UnlockResult::Cancelled => {
                        log::info!("Native unlock dialog: user cancelled");
                    }
                }
            }))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = _app;
        Err("Native unlock dialog not supported on this platform".into())
    }
}
```

---

## 5. 需要的 windows crate features

| API | 所属 feature | 已有 |
|-----|-------------|------|
| `GetSystemMetrics` | `Win32_UI_WindowsAndMessaging` | ✅ |
| `SM_CXSCREEN` / `SM_CYSCREEN` | `Win32_UI_WindowsAndMessaging` | ✅ |
| `IsDialogMessageW` | `Win32_UI_WindowsAndMessaging` | ✅ |
| `BS_DEFPUSHBUTTON` | `Win32_UI_WindowsAndMessaging` | ✅ |
| `WS_EX_CONTROLPARENT` | `Win32_UI_WindowsAndMessaging` | ✅ |
| `GetAncestor` | `Win32_UI_WindowsAndMessaging` | ✅ |
| `LoadCursorW` / `IDC_HAND` | `Win32_UI_WindowsAndMessaging` | ✅ |
| `InvalidateRect` | `Win32_Graphics_Gdi` | ✅ |
| `DrawTextW` | `Win32_Graphics_Gdi` | ✅ |
| `SetBkMode` / `SetTextColor` | `Win32_Graphics_Gdi` | ✅ |
| `GetSysColor` / `COLOR_GRAYTEXT` | `Win32_Graphics_Gdi` | ✅ |
| `CreateFontIndirectW` | `Win32_Graphics_Gdi` | ✅ |
| `SelectObject` / `DeleteObject` | `Win32_Graphics_Gdi` | ✅ |
| `SetWindowLongPtrW` / `GetWindowLongPtrW` | linked from user32 | ✅ |
| `GetDlgItem` | linked from user32 | ✅ |
| `ScreenToClient` / `GetCursorPos` | linked from user32 | ✅ |

DPI 适配新增（详见 §7）：

| API | 所属 feature | 状态 |
|-----|-------------|------|
| `GetDpiForSystem` | `Win32_UI_HiDpi` | ❌ **新增** |
| `AdjustWindowRectExForDpi` | `Win32_UI_HiDpi` | ❌ **新增** |
| `GetDpiForWindow` | `Win32_UI_HiDpi` | ❌ **新增**（`WM_DPICHANGED` 进阶用） |

Dark Mode 适配新增（详见 §9）：

| API | 所属 feature | 状态 |
|-----|-------------|------|
| `DwmSetWindowAttribute` | `Win32_Graphics_Dwm` | ✅ 已新增 |
| `DWMWA_USE_IMMERSIVE_DARK_MODE` | `Win32_Graphics_Dwm` | ✅ 已新增 |
| `TrackMouseEvent` / `TRACKMOUSEEVENT` / `TME_LEAVE` | `Win32_UI_Input_KeyboardAndMouse` | ✅ 已有 |
| `WM_MOUSELEAVE` | `Win32_UI_Controls` | ✅ 已有 |

---

## 6. FFI 声明

部分 API 在 windows crate 0.62 中未暴露或签名不便，通过 `#[link(name = "user32")]` 直接链接：

```rust
#[link(name = "user32")]
extern "system" {
    fn GetWindowLongPtrW(hwnd: HWND, nindex: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: HWND, nindex: i32, newlong: isize) -> isize;
    fn GetDlgItem(hDlg: HWND, nIDDlgItem: i32) -> HWND;
    fn ScreenToClient(hWnd: HWND, lpPoint: *mut POINT) -> i32;
    fn GetCursorPos(lpPoint: *mut POINT) -> i32;
}
```

---

## 7. DPI 适配（设计，待实现）

### 7.1 问题分析

当前实现所有尺寸均为硬编码像素值：

| 硬编码项 | 值 | 影响 |
|----------|---|------|
| 窗口尺寸 | `WINDOW_WIDTH = 380`, `WINDOW_HEIGHT = 150` | 高 DPI 下窗口过小 |
| 控件位置 | `(14, 10)`, `(14, 36)`, `(192, 72)` 等 | 高 DPI 下控件间距过窄 |
| 控件尺寸 | `(344, 18)`, `(344, 24)`, `(80, 26)` 等 | 高 DPI 下控件过小 |
| 字体高度 | `lfHeight = -12`（Segoe UI）, `lfHeight = -14`（Emoji） | 高 DPI 下文字过小 |
| 图标边距 | `ICON_RIGHT_MARGIN = 26` | 高 DPI 下图标区域过窄 |

**进程 DPI 上下文**：Tauri v2 + WebView2 在初始化时调用 `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)`，将进程设为 Per-Monitor DPI Aware v2。这意味着：

- Windows **不会**对窗口进行位图拉伸（不会模糊）
- 但系统**不会**自动缩放控件布局——所有 `CreateWindowExW` 的坐标仍以物理像素为单位
- 在 150% DPI 显示器上，380×150 的窗口在物理上仍是 380×150 像素，视觉上仅占 96 DPI 下面积的 44%

**结论**：必须手动查询目标显示器 DPI 并缩放所有布局值。

### 7.2 适配策略：Per-Monitor DPI 缩放

采用 **DPI 缩放因子** 方案：在创建窗口前查询目标显示器 DPI，将所有布局常量乘以 `dpi / 96`。

```
96 DPI (100%)  → 窗口 380×150，控件位置/尺寸 = 基准值
144 DPI (150%) → 窗口 570×225，控件位置/尺寸 = 基准值 × 1.5
192 DPI (200%) → 窗口 760×300，控件位置/尺寸 = 基准值 × 2.0
```

### 7.3 DPI 查询

```rust
fn get_dpi() -> u32 {
    unsafe { GetDpiForSystem() }
}

fn dpi_factor(dpi: u32) -> f64 {
    dpi as f64 / 96.0
}

fn scale(value: i32, factor: f64) -> i32 {
    (value as f64 * factor).round() as i32
}
```

### 7.4 窗口尺寸计算

使用 `AdjustWindowRectExForDpi` 根据窗口样式和 DPI 计算包含非客户区的窗口矩形：

```rust
fn calc_window_rect(dpi: u32) -> RECT {
    let factor = dpi_factor(dpi);
    let client_w = scale(WINDOW_WIDTH, factor);
    let client_h = scale(WINDOW_HEIGHT, factor);
    let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU;
    let ex_style = WS_EX_DLGMODALFRAME | WS_EX_TOPMOST | WS_EX_CONTROLPARENT;
    let mut rect = RECT { left: 0, top: 0, right: client_w, bottom: client_h };
    unsafe { let _ = AdjustWindowRectExForDpi(&mut rect, style, false, ex_style, dpi); }
    rect
}
```

### 7.5 控件布局缩放

引入 `LayoutMetrics` 结构体，将所有基准值集中管理，按 DPI 因子一次性缩放：

```rust
struct LayoutMetrics {
    margin: i32,           // 基准 14
    title_y: i32,          // 基准 10
    title_h: i32,          // 基准 18
    edit_y: i32,           // 基准 36
    edit_h: i32,           // 基准 24
    btn_y: i32,            // 基准 72
    btn_h: i32,            // 基准 26
    btn_w: i32,            // 基准 80
    icon_margin: i32,      // 基准 26 (ICON_RIGHT_MARGIN)
    content_w: i32,        // 基准 344
    unlock_btn_x: i32,     // 基准 192
    close_btn_x: i32,      // 基准 280
}

impl LayoutMetrics {
    fn new(dpi: u32) -> Self {
        let f = dpi_factor(dpi);
        Self {
            margin:       scale(14, f),
            title_y:      scale(10, f),
            title_h:      scale(18, f),
            edit_y:       scale(36, f),
            edit_h:       scale(24, f),
            btn_y:        scale(72, f),
            btn_h:        scale(26, f),
            btn_w:        scale(80, f),
            icon_margin:  scale(26, f),
            content_w:    scale(344, f),
            unlock_btn_x: scale(192, f),
            close_btn_x:  scale(280, f),
        }
    }
}
```

`WM_CREATE` 中使用 `LayoutMetrics` 替代硬编码值。`DialogState` 持有 `layout: LayoutMetrics` 和 `dpi: u32`。

### 7.6 字体缩放

```rust
fn create_default_font(dpi: u32) -> HFONT {
    let mut lf = LOGFONTW::default();
    lf.lfHeight = -scale(12, dpi_factor(dpi));
    lf.lfWeight = 400;
    // ... Segoe UI
    unsafe { CreateFontIndirectW(&lf) }
}

fn create_icon_font(dpi: u32) -> HFONT {
    let mut lf = LOGFONTW::default();
    lf.lfHeight = -scale(14, dpi_factor(dpi));
    lf.lfWeight = 400;
    // ... Segoe UI Emoji
    unsafe { CreateFontIndirectW(&lf) }
}
```

### 7.7 图标区域与鼠标检测缩放

`get_icon_rect` 从 `DialogState.layout.icon_margin` 读取缩放后的边距：

```rust
unsafe fn get_icon_rect(hwnd: HWND) -> RECT {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as DialogStatePtr;
    let margin = if !state_ptr.is_null() { (*state_ptr).layout.icon_margin } else { 26 };
    let mut rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut rect);
    RECT { left: rect.right - margin, top: 0, right: rect.right - 2, bottom: rect.bottom }
}
```

### 7.8 窗口居中改进

```rust
let dpi = get_dpi();
let win_rect = calc_window_rect(dpi);
let win_w = win_rect.right - win_rect.left;
let win_h = win_rect.bottom - win_rect.top;
let cx = GetSystemMetrics(SM_CXSCREEN);
let cy = GetSystemMetrics(SM_CYSCREEN);
let x = (cx - win_w) / 2 - win_rect.left;
let y = (cy - win_h) / 2 - win_rect.top;
```

### 7.9 WM_DPICHANGED 支持（可选进阶）

解锁对话框为短暂交互，跨显示器拖动概率极低。**首版不实现 `WM_DPICHANGED`**，仅使用创建时的系统 DPI。

### 7.10 需要的 windows crate features

在 workspace `Cargo.toml` 的 `windows` features 列表中新增 `Win32_UI_HiDpi`。

### 7.11 DPI 适配完整流程

```
std::thread::spawn
  │
  ├─ ① dpi = GetDpiForSystem()
  ├─ ② layout = LayoutMetrics::new(dpi)
  ├─ ③ win_rect = calc_window_rect(dpi)
  ├─ ④ (x, y) = 屏幕居中计算(win_rect)
  ├─ ⑤ CreateWindowExW(x, y, win_w, win_h, ...)
  │     └─ WM_CREATE:
  │         ├─ hfont = create_default_font(dpi)
  │         ├─ 创建子控件，使用 layout 中的缩放坐标
  │         └─ EM_SETMARGINS 使用 layout.icon_margin
  ├─ ⑥ 消息循环
  └─ ⑦ 线程退出
```

---

## 8. 验收标准

- [x] 密码输入框默认显示掩码字符（`●`），非明文
- [x] 输入框宽度占窗口近满宽（344px），左右 margin 对称（14px）
- [x] 可见性图标直接绘制在 EDIT 控件内部右端，始终可见
- [x] 点击图标 Toggle 切换掩码/明文（`EM_SETPASSWORDCHAR`）
- [x] 图标区域鼠标悬停显示手型光标（`IDC_HAND`）
- [x] 图标绘制使用透明背景 + 系统灰色文字（`SetBkMode(TRANSPARENT)` + `GetSysColor(COLOR_GRAYTEXT)`）
- [x] 图标字体为 `Segoe UI Emoji`，支持 Emoji 渲染
- [x] `EM_SETMARGINS(EC_RIGHTMARGIN)` 留出图标区域，文字不覆盖图标
- [x] Enter 键提交（`BS_DEFPUSHBUTTON` + `IsDialogMessageW`）
- [x] Escape 键取消（`WM_KEYDOWN` + `VK_ESCAPE`）
- [x] Tab 键在控件间正常导航（`WS_EX_CONTROLPARENT` + `IsDialogMessageW`）
- [x] 窗口居中显示于屏幕中央（`GetSystemMetrics`）
- [x] 窗口置顶（`WS_EX_TOPMOST`），屏幕锁定后可见
- [x] Unlock 按钮为默认按钮样式（粗边框，响应 Enter）
- [x] 点击 Unlock 后回调 `UnlockResult::Confirmed(SecureString)`
- [x] 点击 Close / × / Escape 后回调 `UnlockResult::Cancelled`
- [x] 回调类型为 `UnlockResult`
- [x] 移除未使用的 `ID_STATIC_ERR` 控件
- [x] 移除 `ID_BTN_TOGGLE_VIS` BUTTON 控件，图标改为 EDIT 内部绘制
- [x] `read_edit_text_secure` 返回 `SecureString`，`Vec<u16>` 中间缓冲区显式 zeroize
- [x] Unlock 后清空 EDIT 控件文本 + undo 缓冲区
- [x] `SecureString` 离开回调作用域后自动 zeroize
- [x] `WM_DESTROY` 中 `DeleteObject` 释放图标字体
- [ ] 96 DPI (100%) 下窗口尺寸为 380×150，控件布局与当前硬编码值一致
- [ ] 144 DPI (150%) 下窗口尺寸约为 570×225，控件等比缩放
- [ ] 192 DPI (200%) 下窗口尺寸约为 760×300，控件等比缩放
- [ ] 字体大小随 DPI 等比缩放（Segoe UI / Segoe UI Emoji）
- [ ] 密码可见性图标区域随 DPI 缩放，鼠标检测正确
- [ ] 窗口居中计算使用 `AdjustWindowRectExForDpi`，客户区精确居中
- [ ] `LayoutMetrics` 集中管理所有布局基准值，无硬编码像素值散落
- [ ] workspace `Cargo.toml` 新增 `Win32_UI_HiDpi` feature
- [x] 系统为 dark 模式时，对话框标题栏为暗色（`DWMWA_USE_IMMERSIVE_DARK_MODE`）
- [x] 系统为 dark 模式时，客户区背景为深灰、文字为白色
- [x] 系统为 dark 模式时，按钮使用 owner-draw 自绘，支持正常/悬停/按下/焦点四种状态
- [x] 系统为 dark 模式时，按钮悬停追踪通过子类化 + `TrackMouseEvent` 实现
- [x] 系统为 dark 模式时，可见性图标颜色为白色（`DARK_TEXT`）
- [x] 系统为 light 模式时，外观与当前完全一致
- [x] 窗口关闭后再打开，按钮悬停状态不残留（`WM_DESTROY` 重置 `BTN_HOVER_*`）
- [ ] `AppConf.theme = "system"` 时跟随系统；`"light"` / `"dark"` 时按用户指定
- [x] workspace `Cargo.toml` 新增 `Win32_Graphics_Dwm` feature
- [x] 系统 UI 语言为中文时，窗口标题/标签/按钮显示中文
- [x] 系统 UI 语言为非中文时，窗口标题/标签/按钮显示英文
- [x] 语言检测使用 `GetUserDefaultUILanguage()` 主语言 ID，无需额外框架

---

## 10. 国际化（已实现）

### 10.1 设计决策

原生对话框不依赖 Tauri i18next 运行时，采用**硬编码中英文映射**，通过 Win32 API `GetUserDefaultUILanguage()` 检测系统 UI 语言。理由：

1. 对话框生命周期极短（用户输入密码后立即关闭），无需运行时语言切换
2. 仅 4 个字符串，不值得引入 i18n 框架
3. Win32 API 零依赖、零开销

### 10.2 语言检测

```rust
#[link(name = "kernel32")]
extern "system" {
    fn GetUserDefaultUILanguage() -> u16;
}

fn is_chinese_ui() -> bool {
    let lang_id = unsafe { GetUserDefaultUILanguage() };
    let primary_lang = lang_id & 0x3F;  // 低 6 位为主语言 ID
    primary_lang == 0x04                // 0x04 = 中文系（zh-CN/zh-TW/zh-HK/zh-SG 等）
}
```

> `GetUserDefaultUILanguage()` 返回 `LANGID`（16 位），低 6 位为主语言 ID（Primary Language Identifier）。中文系主语言 ID 为 `0x04`，覆盖简体/繁体/新加坡等所有中文变体。

### 10.3 字符串映射

```rust
struct UiStrings {
    title: &'static str,   // 窗口标题
    label: &'static str,   // 密码输入标签
    unlock: &'static str,  // Unlock 按钮
    close: &'static str,   // Close 按钮
}

fn get_ui_strings() -> UiStrings {
    if is_chinese_ui() {
        UiStrings {
            title: "保险库解锁",
            label: "请输入主密码：",
            unlock: "解锁",
            close: "关闭",
        }
    } else {
        UiStrings {
            title: "Vault Unlock",
            label: "Enter master password:",
            unlock: "Unlock",
            close: "Close",
        }
    }
}
```

### 10.4 宽字符串转换

Win32 `CreateWindowExW` 需要 `PCWSTR`（null-terminated UTF-16），运行时字符串无法使用 `w!()` 宏（编译期），因此提供转换函数：

```rust
fn str_to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0u16)).collect()
}
```

### 10.5 DialogState 扩展

```rust
struct DialogState {
    // ... 原有字段 ...
    ui_strings: UiStrings,  // 新增
}
```

### 10.6 使用方式

#### 窗口标题（`show()` 中 `CreateWindowExW` 调用前）

```rust
let ui_strings = get_ui_strings();
// ... 存入 DialogState ...
let title_wide = str_to_wide(&(*state).ui_strings.title);
CreateWindowExW(..., PCWSTR(title_wide.as_ptr()), ...);
```

#### 控件文本（`WM_CREATE` 中）

```rust
let ui = &(*state_ptr).ui_strings;
let label_wide = str_to_wide(ui.label);
let unlock_wide = str_to_wide(ui.unlock);
let close_wide = str_to_wide(ui.close);

CreateWindowExW(..., PCWSTR(label_wide.as_ptr()), ...);    // STATIC
CreateWindowExW(..., PCWSTR(unlock_wide.as_ptr()), ...);   // Unlock BUTTON
CreateWindowExW(..., PCWSTR(close_wide.as_ptr()), ...);    // Close BUTTON
```

### 10.7 国际化完整流程

```
show()
  │
  ├─ ① ui_strings = get_ui_strings()
  │     └─ is_chinese_ui() → GetUserDefaultUILanguage() & 0x3F == 0x04
  ├─ ② DialogState { ..., ui_strings }
  ├─ ③ CreateWindowExW(PCWSTR(str_to_wide(ui_strings.title).as_ptr()))
  │     └─ WM_CREATE:
  │         ├─ str_to_wide(ui.label)    → STATIC 控件文本
  │         ├─ str_to_wide(ui.unlock)   → Unlock 按钮文本
  │         └─ str_to_wide(ui.close)    → Close 按钮文本
  └─ ④ 消息循环 / 用户交互 / 销毁
```

### 10.8 字符串对照表

| 控件 | 中文 | 英文 |
|------|------|------|
| 窗口标题 | 保险库解锁 | Vault Unlock |
| 密码标签 | 请输入主密码： | Enter master password: |
| Unlock 按钮 | 解锁 | Unlock |
| Close 按钮 | 关闭 | Close |

## 9. Dark Mode 适配（已实现）

### 9.1 问题分析

项目支持 `"system"` / `"light"` / `"dark"` 三种主题（`AppConf.theme`），Tauri Webview 窗口通过 Ant Design `darkAlgorithm` 跟随。原生对话框始终使用 Win32 默认浅色，dark 模式下视觉割裂。

Win32 原生控件不内置 dark mode（不同于 UWP/WinUI 3）。Windows 10 1903+ 提供已文档化的 `DwmSetWindowAttribute(DWMWA_USE_IMMERSIVE_DARK_MODE)` 可使标题栏变暗；客户区需手动着色。

### 9.2 主题检测

对话框创建时读取注册表一次，无需运行时追踪切换（对话框生命周期极短）：

```rust
fn is_system_dark_mode() -> bool {
    use windows::Win32::System::Registry::RegGetValueW;

    let mut buf: u32 = 0;
    let mut buf_size: u32 = std::mem::size_of::<u32>() as u32;

    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_DWORD,
            None,
            Some(&mut buf as *mut u32 as *mut c_void),
            Some(&mut buf_size),
        )
    };

    result.is_ok() && buf == 0
}
```

> 使用 `RegGetValueW` 而非 `winreg` crate，避免额外依赖。与 Tauri `window.theme()` 结果一致（两者底层均读取同一系统设置）。

### 9.3 标题栏暗色

```rust
if is_dark {
    let value: i32 = 1;
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_USE_IMMERSIVE_DARK_MODE,
        &value as *const i32 as *const c_void,
        std::mem::size_of::<i32>() as u32,
    );
}
```

> 使用 `DWMWA_USE_IMMERSIVE_DARK_MODE`（windows crate 中的命名，值为 20），Windows 10 1903+ 文档化 API。

### 9.4 客户区着色

使用 `WM_ERASEBKGND` + `FillRect` 代替 `SetClassLongPtrW(GCLP_HBRBACKGROUND)`，避免影响同类所有窗口实例：

```rust
WM_ERASEBKGND => {
    if is_dark {
        let mut rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut rect);
        let _ = FillRect(HDC(wparam.0 as *mut c_void), &rect, dark_brush);
        return LRESULT(1);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
```

### 9.5 控件文字/背景色

```rust
WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORBTN => {
    if is_dark {
        let dc = HDC(wparam.0 as *mut c_void);
        let _ = SetTextColor(dc, COLORREF(DARK_TEXT));
        let _ = SetBkColor(dc, COLORREF(DARK_BG));
        return LRESULT(dark_brush.0 as isize);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
```

| 控件 | 消息 | 文字色 | 背景色 |
|------|------|--------|--------|
| STATIC | `WM_CTLCOLORSTATIC` | `#FFFFFF` | `#1E1E1E` |
| EDIT | `WM_CTLCOLOREDIT` | `#FFFFFF` | `#1E1E1E` |
| BUTTON | `WM_CTLCOLORBTN` | `#FFFFFF` | `#1E1E1E` |

### 9.6 深色配色常量

```rust
const DARK_BG: u32 = 0x001E1E1E;          // 客户区背景 #1E1E1E
const DARK_TEXT: u32 = 0x00FFFFFF;         // 文字 #FFFFFF
const DARK_BTN_FACE: u32 = 0x002D2D2D;    // 按钮面 #2D2D2D
const DARK_BTN_BORDER: u32 = 0x004D4D4D;  // 按钮边框 #4D4D4D
const DARK_BTN_HOVER: u32 = 0x00504040;   // 按钮悬停 #504040（偏蓝调）
const DARK_BTN_PRESSED: u32 = 0x005C4848; // 按钮按下 #5C4848（偏蓝调）
```

> 悬停/按下色偏蓝调（R 分量较高），模拟 Windows 强调色暗色变体，与纯灰区分明显。

### 9.7 Owner-Draw 按钮

`WM_CTLCOLORBTN` 在 Visual Styles 启用时被忽略，按钮始终由主题管理器渲染为浅色。解决方案：深色模式下使用 `BS_OWNERDRAW` 自绘按钮。

#### 9.7.1 按钮样式切换

```rust
let btn_unlock_style = if is_dark {
    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON | BS_OWNERDRAW | BS_NOTIFY)
} else {
    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON)
};
```

> `BS_NOTIFY` 使按钮在状态变化时发送通知，但 `ODS_HOTLIGHT` 仅在 Visual Styles 启用时有效。实际悬停追踪通过按钮子类化实现（§9.8）。

#### 9.7.2 WM_DRAWITEM 处理

```rust
WM_DRAWITEM => {
    let dis = lparam.0 as *const DRAWITEMSTRUCT;
    if dis.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    unsafe { draw_dark_button(&*dis) };
    LRESULT(1)
}
```

#### 9.7.3 按钮绘制实现

```rust
unsafe fn draw_dark_button(dis: &DRAWITEMSTRUCT) {
    let dc = dis.hDC;
    let rect = dis.rcItem;

    let is_selected = (dis.itemState.0 & ODS_SELECTED.0) != 0;
    let btn_id = GetWindowLongPtrW(dis.hwndItem, GWLP_USERDATA) as u32;
    let is_hot = if btn_id == ID_BTN_UNLOCK as u32 {
        BTN_HOVER_UNLOCK
    } else if btn_id == ID_BTN_CLOSE as u32 {
        BTN_HOVER_CLOSE
    } else {
        false
    };
    let is_focused = (dis.itemState.0 & ODS_FOCUS.0) != 0;

    // 1. 填充背景
    let bg_color = if is_selected { COLORREF(DARK_BTN_PRESSED) }
                   else if is_hot { COLORREF(DARK_BTN_HOVER) }
                   else { COLORREF(DARK_BTN_FACE) };
    let bg_brush = CreateSolidBrush(bg_color);
    let _ = FillRect(dc, &rect, bg_brush);
    let _ = DeleteObject(bg_brush.into());

    // 2. 绘制边框
    let border_brush = CreateSolidBrush(COLORREF(DARK_BTN_BORDER));
    let _ = FrameRect(dc, &rect, border_brush);
    let _ = DeleteObject(border_brush.into());

    // 3. 绘制文字
    let _ = SetBkMode(dc, BACKGROUND_MODE(BKMODE_TRANSPARENT));
    let _ = SetTextColor(dc, COLORREF(DARK_TEXT));
    // ... DrawTextW 居中绘制

    // 4. 绘制焦点虚线框
    if is_focused {
        let mut focus_rect = rect;
        focus_rect.left += 2; focus_rect.top += 2;
        focus_rect.right -= 2; focus_rect.bottom -= 2;
        let _ = DrawFocusRect(dc, &focus_rect);
    }
}
```

| 状态 | 背景色 | 触发条件 |
|------|--------|---------|
| 正常 | `#2D2D2D` | 鼠标不在按钮上 |
| 悬停 | `#504040` | 鼠标在按钮上（`BTN_HOVER_*`） |
| 按下 | `#5C4848` | 鼠标按下（`ODS_SELECTED`） |
| 焦点 | 虚线框 | Tab 聚焦（`ODS_FOCUS`） |

### 9.8 按钮悬停追踪（子类化）

`ODS_HOTLIGHT` 仅在 Visual Styles（UxTheme）启用时有效。当前程序未初始化 Visual Styles，因此需要通过按钮子类化 + `TrackMouseEvent` 手动追踪鼠标悬停。

#### 9.8.1 全局悬停状态

```rust
static mut BTN_HOVER_UNLOCK: bool = false;
static mut BTN_HOVER_CLOSE: bool = false;
```

> 每个按钮独立追踪，避免一个按钮悬停影响另一个。`WM_DESTROY` 中重置为 `false`，防止窗口关闭后再打开时残留状态。

#### 9.8.2 子类化安装

```rust
// WM_CREATE 中，创建按钮后：
if is_dark {
    SetWindowLongPtrW(h, GWLP_USERDATA, ID_BTN_UNLOCK as isize);  // 存储按钮 ID
    let orig = SetWindowLongPtrW(h, GWLP_WNDPROC, btn_subclass_proc as *const () as isize);
    ORIGINAL_BTN_PROC = Some(std::mem::transmute::<isize, _>(orig));
}
```

> `GWLP_USERDATA` 存储按钮 ID，子类化 proc 中通过它区分 Unlock / Close 按钮。

#### 9.8.3 子类化消息处理

```rust
unsafe extern "system" fn btn_subclass_proc(
    hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    let btn_id = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as u32;

    match msg {
        WM_MOUSEMOVE => {
            let was_hover = if btn_id == ID_BTN_UNLOCK as u32 { BTN_HOVER_UNLOCK }
                            else { BTN_HOVER_CLOSE };
            if !was_hover {
                // 设置 hover 标志
                if btn_id == ID_BTN_UNLOCK as u32 { BTN_HOVER_UNLOCK = true; }
                else { BTN_HOVER_CLOSE = true; }
                // 开始追踪鼠标离开
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut tme);
                let _ = InvalidateRect(Some(hwnd), None, true);  // 触发重绘
            }
        }
        WM_MOUSELEAVE => {
            // 清除 hover 标志
            if btn_id == ID_BTN_UNLOCK as u32 { BTN_HOVER_UNLOCK = false; }
            else { BTN_HOVER_CLOSE = false; }
            let _ = InvalidateRect(Some(hwnd), None, true);  // 触发重绘
        }
        _ => {}
    }

    // 转发到原始 wndproc
    if let Some(orig) = ORIGINAL_BTN_PROC {
        orig(hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}
```

| 消息 | 行为 |
|------|------|
| `WM_MOUSEMOVE` | 首次进入时设置 `BTN_HOVER_* = true`，调用 `TrackMouseEvent(TME_LEAVE)` 开始追踪，`InvalidateRect` 触发重绘 |
| `WM_MOUSELEAVE` | 设置 `BTN_HOVER_* = false`，`InvalidateRect` 触发重绘恢复常态 |

### 9.9 可见性图标深色适配

深色模式下图标颜色使用 `DARK_TEXT`（白色）代替 `GetSysColor(COLOR_GRAYTEXT)`：

```rust
let icon_color = if is_dark {
    COLORREF(DARK_TEXT)
} else {
    COLORREF(GetSysColor(COLOR_GRAYTEXT))
};
let _ = SetTextColor(dc, icon_color);
```

### 9.10 DialogState 扩展

```rust
struct DialogState {
    on_result: Option<Box<dyn FnOnce(UnlockResult) + Send>>,
    closed: bool,
    password_visible: bool,
    layout: LayoutMetrics,
    is_dark: bool,          // 新增
    dark_brush: HBRUSH,     // 新增，WM_DESTROY 中 DeleteObject 释放
}
```

### 9.11 WM_DESTROY 清理

```rust
WM_DESTROY => {
    // ... 原有清理 ...
    BTN_HOVER_UNLOCK = false;   // 重置悬停状态
    BTN_HOVER_CLOSE = false;    // 防止再次打开时残留
    PostQuitMessage(0);
    LRESULT(0)
}
```

### 9.12 需要的 windows crate features

| API | 所属 feature | 状态 |
|-----|-------------|------|
| `DwmSetWindowAttribute` | `Win32_Graphics_Dwm` | ✅ 已新增 |
| `DWMWA_USE_IMMERSIVE_DARK_MODE` | `Win32_Graphics_Dwm` | ✅ 已新增 |
| `TrackMouseEvent` / `TRACKMOUSEEVENT` / `TME_LEAVE` | `Win32_UI_Input_KeyboardAndMouse` | ✅ 已有 |
| `WM_MOUSELEAVE` | `Win32_UI_Controls` | ✅ 已有 |

### 9.13 Dark Mode 适配完整流程

```
std::thread::spawn
  │
  ├─ ① is_dark = is_system_dark_mode()  // RegGetValueW 读注册表
  ├─ ② CreateWindowExW(...)
  │     └─ WM_CREATE:
  │         ├─ if is_dark:
  │         │   ├─ DwmSetWindowAttribute(DWMWA_USE_IMMERSIVE_DARK_MODE)  // 标题栏暗色
  │         │   ├─ 创建子控件（按钮使用 BS_OWNERDRAW | BS_NOTIFY）
  │         │   ├─ 子类化按钮（btn_subclass_proc），GWLP_USERDATA 存按钮 ID
  │         │   ├─ CreateSolidBrush(RGB(30,30,30)) → DialogState.dark_brush
  │         │   └─ DialogState.is_dark = true
  │         └─ else: 创建子控件（按钮使用标准样式）
  ├─ ③ 消息循环
  │     ├─ WM_ERASEBKGND: if is_dark → FillRect(dark_brush)
  │     ├─ WM_CTLCOLORSTATIC / WM_CTLCOLOREDIT / WM_CTLCOLORBTN:
  │     │   └─ if is_dark: SetTextColor(白) + SetBkColor(深灰) + return dark_brush
  │     ├─ WM_DRAWITEM: draw_dark_button()  // owner-draw 按钮绘制
  │     └─ 按钮子类化: WM_MOUSEMOVE → TrackMouseEvent → WM_MOUSELEAVE
  └─ ④ WM_DESTROY: DeleteObject(dark_brush) + BTN_HOVER_* = false
```