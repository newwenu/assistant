// T2 漂移测试：`tauri.conf.json` 的 identifier 与 asset 作用域必须与 `app_dirs` 一致。
//
// 该文件是手写配置，无法引用 Rust 常量；这里用相等断言把两边钉住，
// 任一侧改动而另一侧未跟进都会失败。
//
// 同文件还承载 T16：与前端共读 `fixtures/verbatim-form-vectors.json`，逐条断言 `strip_verbatim_prefix`。

use std::fs;
use std::path::Path;

use assistant_app::app_dirs::{strip_verbatim_prefix, AppDirs};

#[test]
fn identifier_and_asset_scope_match_app_dirs() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let raw =
        fs::read_to_string(manifest.join("tauri.conf.json")).expect("tauri.conf.json 应可读取");
    let conf: serde_json::Value =
        serde_json::from_str(&raw).expect("tauri.conf.json 应为合法 JSON");

    let identifier = conf["identifier"].as_str().expect("identifier 应为字符串");
    assert_eq!(identifier, assistant_config::constant::APP_IDENTIFIER);

    let allow = conf["app"]["security"]["assetProtocol"]["scope"]["allow"]
        .as_array()
        .expect("assetProtocol.scope.allow 应为数组");
    let mut actual: Vec<String> = allow
        .iter()
        .map(|item| item.as_str().expect("scope 条目应为字符串").to_string())
        .collect();
    let mut expected = AppDirs::asset_scopes();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected, "asset 作用域与 asset_scopes() 不一致");
}

#[test]
fn verbatim_vectors_match_frontend_contract() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let raw = fs::read_to_string(manifest.join("tests/fixtures/verbatim-form-vectors.json"))
        .expect("向量文件应可读取");
    let doc: serde_json::Value = serde_json::from_str(&raw).expect("向量文件应为合法 JSON");

    let vectors = doc["vectors"].as_array().expect("vectors 应为数组");
    assert!(!vectors.is_empty(), "向量文件不得为空");

    for entry in vectors {
        let input = entry["input"].as_str().expect("input 应为字符串");
        let expected = entry["expected"].as_str().expect("expected 应为字符串");
        let note = entry["note"].as_str().unwrap_or("");
        assert_eq!(
            strip_verbatim_prefix(input),
            expected,
            "向量不一致（{note}）：input={input:?}"
        );
    }
}
