use std::{fs, path::Path};

const CATALOGS: [&str; 2] = ["locales/zh-CN.json", "locales/en-US.json"];

fn main() {
    let mut fingerprint = 0xcbf29ce484222325_u64;
    for catalog in CATALOGS {
        println!("cargo:rerun-if-changed={catalog}");
        let bytes = fs::read(Path::new(catalog)).expect("read localization catalog");
        for byte in bytes {
            fingerprint ^= u64::from(byte);
            fingerprint = fingerprint.wrapping_mul(0x100000001b3);
        }
    }
    println!("cargo:rustc-env=ASSISTANT_I18N_CATALOG_REVISION={fingerprint:016x}");
}
