// 参考: docs/store/feature-config-ownership-spec.md §6.2 — B1 / B2 结构断言

use std::fs;
use std::path::{Path, PathBuf};

const FEATURE_MANIFEST: &str = "../../../config/features.json";
const MECHANISM_FILES: [&str; 2] = ["conf_group.rs", "json_config.rs"];

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).expect("source directory is readable");
    for entry in entries {
        let path = entry.expect("source entry is readable").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn feature_ids() -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(FEATURE_MANIFEST);
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is readable: {}", path.display(), e));
    let value: serde_json::Value = serde_json::from_str(&raw).expect("features.json is valid JSON");
    let ids: Vec<String> = value
        .as_object()
        .expect("features.json is an object")
        .keys()
        .cloned()
        .collect();
    assert!(
        !ids.is_empty(),
        "config/features.json 未声明任何特性，扫描失去基准"
    );
    ids
}

#[test]
fn config_crate_has_no_feature_schema_module() {
    let mut files = Vec::new();
    collect_rs_files(&src_root(), &mut files);
    let offenders: Vec<String> = files
        .iter()
        .filter(|file| {
            file.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.ends_with("_conf.rs"))
                .unwrap_or(false)
        })
        .map(|file| {
            file.strip_prefix(src_root())
                .unwrap_or(file)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .filter(|name| {
            !matches!(
                name.as_str(),
                "app_conf.rs" | "orb_conf.rs" | "mascot_conf.rs"
            )
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "assistant-config 只允许保留三个核心配置组，特性 schema 必须归各自 crate: {:?}",
        offenders
    );
}

#[test]
fn config_crate_sources_do_not_mention_feature_ids() {
    let ids = feature_ids();
    let mut files = Vec::new();
    collect_rs_files(&src_root(), &mut files);

    let mut offenders = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).expect("source file is readable");
        let lowered = text.to_lowercase();
        for id in &ids {
            if lowered.contains(&id.to_lowercase()) {
                let relative = file
                    .strip_prefix(src_root())
                    .unwrap_or(&file)
                    .to_string_lossy()
                    .replace('\\', "/");
                offenders.push(format!("{} -> {}", relative, id));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "assistant-config 是特性无关的叶子 crate，不得出现特性 id: {:?}",
        offenders
    );
}

#[test]
fn mechanism_layer_has_no_feature_branching() {
    let ids = feature_ids();
    let mut offenders = Vec::new();

    for name in MECHANISM_FILES {
        let path = src_root().join(name);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is readable: {}", path.display(), e));
        for marker in ["cfg!(feature", "#[cfg(feature"] {
            if text.contains(marker) {
                offenders.push(format!("{} -> {}", name, marker));
            }
        }
        let lowered = text.to_lowercase();
        for id in &ids {
            if lowered.contains(&id.to_lowercase()) {
                offenders.push(format!("{} -> feature id {}", name, id));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "配置机制层不得按特性分叉: {:?}",
        offenders
    );
}
