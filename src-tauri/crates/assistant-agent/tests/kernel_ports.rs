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
