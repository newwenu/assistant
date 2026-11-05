#![allow(dead_code)]

use std::path::PathBuf;

pub fn conf_path(name: &str) -> PathBuf {
    tempfile::tempdir().unwrap().path().join(name)
}
