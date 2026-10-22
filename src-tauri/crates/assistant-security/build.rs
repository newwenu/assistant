fn main() {
    if let Ok(dir) = std::env::var("OPENSSL_DIR") {
        let lib_dir = std::path::Path::new(&dir).join("lib");
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        if std::env::var("OPENSSL_STATIC").is_ok() {
            println!("cargo:rustc-link-lib=static=libcrypto");
            println!("cargo:rustc-link-lib=static=libssl");
        } else {
            println!("cargo:rustc-link-lib=dylib=libcrypto");
            println!("cargo:rustc-link-lib=dylib=libssl");
        }
    }

    if cfg!(target_os = "windows") {
        println!("cargo:rustc-link-lib=dylib=crypt32");
        println!("cargo:rustc-link-lib=dylib=ws2_32");
        println!("cargo:rustc-link-lib=dylib=advapi32");
        println!("cargo:rustc-link-lib=dylib=user32");
    }
}
