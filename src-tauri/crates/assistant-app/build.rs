fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let config_path = std::path::Path::new(&manifest_dir)
        .parent().unwrap()
        .parent().unwrap()
        .join("tauri.conf.json");

    tauri_build::try_build(
        tauri_build::Attributes::new()
            .config_path(&config_path),
    )
    .expect("failed to run tauri-build");
}