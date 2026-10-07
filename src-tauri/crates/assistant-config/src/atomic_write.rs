use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

pub fn atomic_write(
    path: &Path,
    data: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("tmp");
    {
        let mut file = File::create(&temp)?;
        file.write_all(data)?;
        file.sync_all()?;
    }
    fs::rename(&temp, path)?;
    Ok(())
}

pub fn recover_temp(path: &Path) -> Option<Vec<u8>> {
    let temp = path.with_extension("tmp");
    if !temp.exists() {
        return None;
    }
    match fs::read(&temp) {
        Ok(bytes) => {
            let _ = fs::remove_file(&temp);
            Some(bytes)
        }
        Err(_) => {
            let _ = fs::remove_file(&temp);
            None
        }
    }
}