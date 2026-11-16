use std::path::Path;

/// 按字节计行数，与 `str::lines()` 的口径一致（空文件 0 行，末尾换行不多算一行）。
pub(super) fn count_lines(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        return 0;
    }
    let newlines = bytes.iter().filter(|byte| **byte == b'\n').count();
    if bytes.ends_with(b"\n") {
        newlines
    } else {
        newlines + 1
    }
}

/// 流式统计行数：逐块读到换行为止，不保留内容。
pub(super) fn count_lines_in_file(path: &Path) -> Option<usize> {
    use std::io::BufRead;

    let file = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(file);
    let mut buffer = Vec::new();
    let mut lines = 0usize;
    loop {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer) {
            Ok(0) => return Some(lines),
            Ok(_) => lines += 1,
            Err(_) => return None,
        }
    }
}
