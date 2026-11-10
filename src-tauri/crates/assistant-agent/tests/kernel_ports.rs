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

// 原 `turn_event_carries_turn_id_and_status_on_termination` 已删除：它在测试内构造三个
// 枚举值再断言其匹配自身（含 events.len() == 3），无行为信息量。TurnEvent 的
// turn_id / status 由非 Option 字段在编译期强制，真实路由由 kernel_emit.rs 与
// loop_executor.rs 驱动 TurnEmitter 覆盖。
