// Orb 窗口快照与隐藏/恢复序列：几何写入、可见性校验、失败聚合。
// 参考: Kiri ADR 0107 — 可见性校验 + ShowWindow 兜底 + DwmFlush

use log::warn;
use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::screenshot::{CANCEL_FAILED, CAPTURE_FAILED};

use super::menu_fade;
use super::platform;
use super::OrbWindowSnapshot;

/// 窗口操作的最小抽象：生产实现包住 `WebviewWindow`，单测注入假实现断言隐藏序列。
pub(crate) trait WindowOps {
    fn outer_position(&self) -> Result<PhysicalPosition<i32>, String>;
    fn outer_size(&self) -> Result<PhysicalSize<u32>, String>;
    fn is_visible(&self) -> Result<bool, String>;
    fn set_visible(&mut self, visible: bool) -> Result<(), String>;
    fn force_hide(&mut self) -> Result<(), String>;
    fn flush(&mut self) -> Result<(), String>;
    fn is_always_on_top(&self) -> Result<bool, String>;
    fn set_always_on_top(&mut self, always_on_top: bool) -> Result<(), String>;
    fn set_size(&mut self, size: PhysicalSize<u32>) -> Result<(), String>;
    fn set_position(&mut self, position: PhysicalPosition<i32>) -> Result<(), String>;
}

pub(super) struct WebviewOps<'a>(pub(super) &'a WebviewWindow);

impl WindowOps for WebviewOps<'_> {
    fn outer_position(&self) -> Result<PhysicalPosition<i32>, String> {
        self.0.outer_position().map_err(|e| e.to_string())
    }

    fn outer_size(&self) -> Result<PhysicalSize<u32>, String> {
        self.0.outer_size().map_err(|e| e.to_string())
    }

    fn is_visible(&self) -> Result<bool, String> {
        self.0.is_visible().map_err(|e| e.to_string())
    }

    fn set_visible(&mut self, visible: bool) -> Result<(), String> {
        if visible {
            self.0.show()
        } else {
            self.0.hide()
        }
        .map_err(|e| e.to_string())
    }

    fn force_hide(&mut self) -> Result<(), String> {
        platform::force_hide(self.0)
    }

    fn flush(&mut self) -> Result<(), String> {
        platform::flush()
    }

    fn is_always_on_top(&self) -> Result<bool, String> {
        self.0.is_always_on_top().map_err(|e| e.to_string())
    }

    fn set_always_on_top(&mut self, always_on_top: bool) -> Result<(), String> {
        self.0
            .set_always_on_top(always_on_top)
            .map_err(|e| e.to_string())
    }

    fn set_size(&mut self, size: PhysicalSize<u32>) -> Result<(), String> {
        self.0.set_size(size).map_err(|e| e.to_string())
    }

    fn set_position(&mut self, position: PhysicalPosition<i32>) -> Result<(), String> {
        self.0.set_position(position).map_err(|e| e.to_string())
    }
}

pub(super) fn snapshot_of(window: &WebviewWindow) -> Result<OrbWindowSnapshot, String> {
    snapshot_with(&WebviewOps(window), crate::cmd::orb::orb_click_through())
}

pub(super) fn snapshot_with(
    ops: &dyn WindowOps,
    click_through: bool,
) -> Result<OrbWindowSnapshot, String> {
    Ok(OrbWindowSnapshot {
        position: ops.outer_position()?,
        size: ops.outer_size()?,
        visible: ops.is_visible()?,
        always_on_top: ops.is_always_on_top()?,
        click_through,
    })
}

/// 几何写入：Tauri 2.12 的 `WebviewWindow` 没有 `set_bounds`，按规格 §8.5.1 的回退方案
/// 先写尺寸再写位置，恢复路径同样写回快照值而不是重算。
pub(super) fn apply_geometry(
    ops: &mut dyn WindowOps,
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
) -> Result<(), String> {
    ops.set_size(size)?;
    ops.set_position(position)?;
    Ok(())
}

/// 隐藏序列：`set_visible(false)` → 可见性仍为真时补平台兜底 → `flush`。
fn hide_sequence(ops: &mut dyn WindowOps) -> Result<(), String> {
    ops.set_visible(false)?;

    // `is_visible` 本身走主线程队列往返，返回的就是 hide 落地后的状态，不需要轮询等待。
    if ops.is_visible()? {
        ops.force_hide()?;
        if ops.is_visible()? {
            return Err(CAPTURE_FAILED.to_string());
        }
    }

    ops.flush()
}

/// 隐藏序列：先清掉本进程残留的弹出菜单，再在禁用 DWM 过渡的前提下隐藏 Orb，
/// 最后排空菜单关闭动画——抓屏读的是当前合成结果，动画未结束就会被冻进底图。
pub(super) fn hide_and_verify(window: &WebviewWindow) -> Result<(), String> {
    menu_fade::hide_leftover_menus()?;
    platform::suppress_transitions(window, || hide_sequence(&mut WebviewOps(window)))?;
    menu_fade::settle_menu_fade()
}

pub(super) fn restore_with(
    ops: &mut dyn WindowOps,
    snapshot: OrbWindowSnapshot,
    click_through: impl FnOnce(bool) -> Result<(), String>,
) -> Result<(), String> {
    let mut issues: Vec<String> = Vec::new();

    // 必须在不可见时改几何：此刻窗口是可见的全屏覆盖层、位置在显示器原点，
    // `set_size` 与 `set_position` 是两次独立的主线程消息，中间会合成一帧——
    // 那一帧里窗口已缩回小球尺寸却仍在原点，于是左上角闪出一个可见的小球。
    if let Err(error) = ops.set_visible(false) {
        issues.push(format!("hide: {error}"));
    }

    if let Err(error) = ops.set_always_on_top(snapshot.always_on_top) {
        issues.push(format!("always_on_top: {error}"));
    }
    if let Err(error) = click_through(snapshot.click_through) {
        issues.push(format!("click_through: {error}"));
    }
    if let Err(error) = apply_geometry(ops, snapshot.position, snapshot.size) {
        issues.push(format!("bounds: {error}"));
    }

    // 只还原可见性，不抢焦点：用户当前前台窗口应保持焦点。
    if snapshot.visible {
        if let Err(error) = ops.set_visible(true) {
            issues.push(format!("visible: {error}"));
        }
    }

    if issues.is_empty() {
        Ok(())
    } else {
        warn!("screenshot: orb restore incomplete: {}", issues.join("; "));
        Err(CANCEL_FAILED.to_string())
    }
}

/// 复位到收起态：先隐藏再改几何，与 `restore_with` 同一不变式；失败聚合后由调用方记日志。
pub(super) fn reset_collapsed(
    ops: &mut dyn WindowOps,
    size: PhysicalSize<u32>,
    position: Option<PhysicalPosition<i32>>,
) -> Result<(), String> {
    let mut issues: Vec<String> = Vec::new();

    if let Err(error) = ops.set_visible(false) {
        issues.push(format!("hide: {error}"));
    }
    if let Err(error) = ops.set_size(size) {
        issues.push(format!("size: {error}"));
    }
    if let Some(position) = position {
        if let Err(error) = ops.set_position(position) {
            issues.push(format!("position: {error}"));
        }
    }
    if let Err(error) = ops.set_visible(true) {
        issues.push(format!("visible: {error}"));
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct FakeOps {
        visible: bool,
        always_on_top: bool,
        position: PhysicalPosition<i32>,
        size: PhysicalSize<u32>,
        hide_takes_effect: bool,
        force_hide_takes_effect: bool,
        fail_always_on_top: bool,
        calls: RefCell<Vec<&'static str>>,
    }

    impl FakeOps {
        fn new() -> Self {
            Self {
                visible: true,
                always_on_top: false,
                position: PhysicalPosition::new(100, 200),
                size: PhysicalSize::new(88, 88),
                hide_takes_effect: true,
                force_hide_takes_effect: true,
                fail_always_on_top: false,
                calls: RefCell::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<&'static str> {
            self.calls.borrow().clone()
        }
    }

    impl WindowOps for FakeOps {
        fn outer_position(&self) -> Result<PhysicalPosition<i32>, String> {
            Ok(self.position)
        }

        fn outer_size(&self) -> Result<PhysicalSize<u32>, String> {
            Ok(self.size)
        }

        fn is_visible(&self) -> Result<bool, String> {
            Ok(self.visible)
        }

        fn set_visible(&mut self, visible: bool) -> Result<(), String> {
            self.calls
                .borrow_mut()
                .push(if visible { "show" } else { "hide" });
            if visible || self.hide_takes_effect {
                self.visible = visible;
            }
            Ok(())
        }

        fn force_hide(&mut self) -> Result<(), String> {
            self.calls.borrow_mut().push("force_hide");
            if self.force_hide_takes_effect {
                self.visible = false;
            }
            Ok(())
        }

        fn flush(&mut self) -> Result<(), String> {
            self.calls.borrow_mut().push("flush");
            Ok(())
        }

        fn is_always_on_top(&self) -> Result<bool, String> {
            Ok(self.always_on_top)
        }

        fn set_always_on_top(&mut self, always_on_top: bool) -> Result<(), String> {
            self.calls.borrow_mut().push("always_on_top");
            if self.fail_always_on_top {
                return Err("boom".to_string());
            }
            self.always_on_top = always_on_top;
            Ok(())
        }

        fn set_size(&mut self, size: PhysicalSize<u32>) -> Result<(), String> {
            self.calls.borrow_mut().push("size");
            self.size = size;
            Ok(())
        }

        fn set_position(&mut self, position: PhysicalPosition<i32>) -> Result<(), String> {
            self.calls.borrow_mut().push("position");
            self.position = position;
            Ok(())
        }
    }

    fn snapshot(visible: bool) -> OrbWindowSnapshot {
        OrbWindowSnapshot {
            position: PhysicalPosition::new(10, 20),
            size: PhysicalSize::new(88, 88),
            visible,
            always_on_top: true,
            click_through: true,
        }
    }

    #[test]
    fn hide_sequence_hides_then_flushes() {
        let mut ops = FakeOps::new();
        hide_sequence(&mut ops).expect("hide");
        assert_eq!(ops.calls(), vec!["hide", "flush"]);
        assert!(!ops.visible);
    }

    #[test]
    fn hide_sequence_falls_back_to_force_hide() {
        let mut ops = FakeOps::new();
        ops.hide_takes_effect = false;
        hide_sequence(&mut ops).expect("force hide fallback");
        assert_eq!(ops.calls(), vec!["hide", "force_hide", "flush"]);
        assert!(!ops.visible);
    }

    #[test]
    fn hide_sequence_reports_capture_failed() {
        let mut ops = FakeOps::new();
        ops.hide_takes_effect = false;
        ops.force_hide_takes_effect = false;
        assert_eq!(
            hide_sequence(&mut ops).unwrap_err(),
            crate::screenshot::CAPTURE_FAILED
        );
        assert_eq!(ops.calls(), vec!["hide", "force_hide"]);
    }

    #[test]
    fn snapshot_reads_all_window_fields() {
        let ops = FakeOps::new();
        let snap = snapshot_with(&ops, true).expect("snapshot");
        assert_eq!(snap.position, PhysicalPosition::new(100, 200));
        assert_eq!(snap.size, PhysicalSize::new(88, 88));
        assert!(snap.visible);
        assert!(!snap.always_on_top);
        assert!(snap.click_through);
    }

    #[test]
    fn restore_writes_snapshot_and_click_through() {
        let mut ops = FakeOps::new();
        let mut restored_click_through = None;
        restore_with(&mut ops, snapshot(false), |enabled| {
            restored_click_through = Some(enabled);
            Result::<(), String>::Ok(())
        })
        .expect("restore");

        assert_eq!(restored_click_through, Some(true));
        assert!(ops.always_on_top);
        assert_eq!(ops.position, PhysicalPosition::new(10, 20));
        assert_eq!(ops.size, PhysicalSize::new(88, 88));
        assert!(!ops.visible);
        // 快照本就不可见时不再补一次 show。
        assert_eq!(
            ops.calls(),
            vec!["hide", "always_on_top", "size", "position"]
        );
    }

    /// 可见的全屏覆盖层缩回小球时，几何必须已在隐藏状态下改完，否则中间帧会把
    /// 小球画在原点（左上角）。这里直接断言「hide 早于 size 与 position」。
    #[test]
    fn restore_hides_before_geometry_change() {
        let mut ops = FakeOps::new();
        ops.size = PhysicalSize::new(2520, 1680);
        ops.position = PhysicalPosition::new(0, 0);
        restore_with(&mut ops, snapshot(true), |_| Ok(())).expect("restore");

        let calls = ops.calls();
        let hide_at = calls.iter().position(|c| *c == "hide").expect("hide");
        let size_at = calls.iter().position(|c| *c == "size").expect("size");
        let position_at = calls
            .iter()
            .position(|c| *c == "position")
            .expect("position");
        assert!(hide_at < size_at, "hide must precede size: {calls:?}");
        assert!(
            hide_at < position_at,
            "hide must precede position: {calls:?}"
        );
        assert_eq!(calls.last(), Some(&"show"));
        assert_eq!(ops.size, PhysicalSize::new(88, 88));
        assert_eq!(ops.position, PhysicalPosition::new(10, 20));
        assert!(ops.visible);
    }

    #[test]
    fn reset_collapsed_hides_before_geometry_change() {
        let mut ops = FakeOps::new();
        ops.size = PhysicalSize::new(2520, 1680);
        ops.position = PhysicalPosition::new(0, 0);
        reset_collapsed(
            &mut ops,
            PhysicalSize::new(88, 88),
            Some(PhysicalPosition::new(10, 20)),
        )
        .expect("reset");

        let calls = ops.calls();
        let hide_at = calls.iter().position(|c| *c == "hide").expect("hide");
        let size_at = calls.iter().position(|c| *c == "size").expect("size");
        assert!(hide_at < size_at, "hide must precede size: {calls:?}");
        assert_eq!(calls.last(), Some(&"show"));
        assert_eq!(ops.size, PhysicalSize::new(88, 88));
        assert_eq!(ops.position, PhysicalPosition::new(10, 20));
    }

    #[test]
    fn restore_keeps_going_and_reports_cancel_failed() {
        let mut ops = FakeOps::new();
        ops.fail_always_on_top = true;
        assert_eq!(
            restore_with(&mut ops, snapshot(true), |_| Ok(())).unwrap_err(),
            crate::screenshot::CANCEL_FAILED
        );
        assert_eq!(
            ops.calls(),
            vec!["hide", "always_on_top", "size", "position", "show"]
        );
        assert!(ops.visible);
    }
}
