use assistant_agent::kernel::{ProviderDelta, TurnEvent, TurnPhase, TurnStatus};
use std::path::{Path, PathBuf};

fn collect_kernel_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).expect("read kernel dir");
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_kernel_sources(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// I36b V1 / G1：本 crate 的依赖清单不得出现 `tauri`。
///
/// 源码扫描证明「内核代码不引用 tauri 类型」，本用例补「依赖层面也拿不到 tauri」；
/// 传递依赖由 §5.5 的 `cargo tree` 门禁兜底。
#[test]
fn manifest_declares_no_tauri_dependency() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).expect("read Cargo.toml");
    let offenders: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| {
            line.starts_with("tauri") || line.contains("tauri =") || line.contains("tauri}")
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "assistant-agent 不得直接依赖 tauri（G1）：{offenders:?}"
    );
}

#[test]
fn kernel_sources_have_no_tauri_types() {
    let kernel_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("kernel");
    let mut sources = Vec::new();
    collect_kernel_sources(&kernel_dir, &mut sources);
    assert!(!sources.is_empty(), "kernel sources not found");

    let forbidden = ["tauri::", "AppHandle", "WebviewWindow", "tauri::State"];
    for path in sources {
        let text = std::fs::read_to_string(&path).expect("read kernel source");
        for needle in forbidden {
            assert!(
                !text.contains(needle),
                "{} contains forbidden tauri token {}",
                path.display(),
                needle
            );
        }
    }
}

#[test]
fn turn_event_carries_turn_id_and_status_on_termination() {
    let events = [
        TurnEvent::StreamStart {
            turn_id: "conv:1".to_string(),
            phase: TurnPhase::Thinking,
        },
        TurnEvent::Delta {
            turn_id: "conv:1".to_string(),
            seq: 0,
            iteration: 1,
            delta: ProviderDelta::default(),
        },
        TurnEvent::Terminated {
            turn_id: "conv:1".to_string(),
            status: TurnStatus::Cancelled,
            last_seq: 0,
            error: None,
        },
    ];
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], TurnEvent::StreamStart { .. }));
    assert!(matches!(events[1], TurnEvent::Delta { seq: 0, .. }));
    assert!(matches!(
        events[2],
        TurnEvent::Terminated {
            status: TurnStatus::Cancelled,
            ..
        }
    ));
}
