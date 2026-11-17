// 平台分派：每个平台一个实现模块，共享入口只做 `pub(crate) use`，调用方不感知平台。

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub(crate) use windows::{flush, force_hide, suppress_transitions};

#[cfg(not(target_os = "windows"))]
mod fallback;

#[cfg(not(target_os = "windows"))]
pub(crate) use fallback::{flush, force_hide, suppress_transitions};
