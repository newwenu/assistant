// 屏幕录制授权：只有 macOS 需要 TCC 前置校验，其它平台直接放行。
// 平台分派：每个平台一个实现模块，调用方统一走 `permission_granted`。

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::permission_granted;

#[cfg(not(target_os = "macos"))]
mod fallback;

#[cfg(not(target_os = "macos"))]
pub use fallback::permission_granted;
