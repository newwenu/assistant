// 路径形态转换：IO 形态（canonical 原样，Windows 带 `\\?\`）与展示形态互转。
//
// 展示形态只用于 UI / wire / 日志文本；比较、持久化、DB 存储、日志键、缓存键一律用绑定形态（原样字符串）。
// 本函数只做前缀处理：不做大小写归一、不做分隔符替换，后两者会破坏比较语义。
//
// 与前端 `src/chat/core/workspaceDisplay.ts` 的 `stripVerbatimPrefix` 属受控重复（I44 §10.2），
// 两侧共用同一组向量 `src-tauri/tests/fixtures/verbatim-form-vectors.json`。

/// 剥离 Windows verbatim 前缀（`\\?\` / `\\?\UNC\`）。
///
/// 输入：任意路径字符串（可能是 canonical 原样值）。
/// 返回：去掉前缀后的可读形式；无前缀时原样返回。
pub fn strip_verbatim_prefix(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{}", rest);
    }
    if let Some(rest) = path.strip_prefix(r"\\?\") {
        return rest.to_string();
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_drive_prefix() {
        assert_eq!(strip_verbatim_prefix(r"\\?\G:\code\app"), r"G:\code\app");
    }

    #[test]
    fn converts_unc_prefix() {
        assert_eq!(
            strip_verbatim_prefix(r"\\?\UNC\server\share\dir"),
            r"\\server\share\dir"
        );
    }

    #[test]
    fn keeps_unprefixed_input_untouched() {
        assert_eq!(strip_verbatim_prefix(r"G:\code\app"), r"G:\code\app");
        assert_eq!(strip_verbatim_prefix("/home/user/app"), "/home/user/app");
        assert_eq!(strip_verbatim_prefix(""), "");
    }

    #[test]
    fn does_not_normalize_case_or_separators() {
        assert_eq!(strip_verbatim_prefix(r"\\?\g:/CODE/app"), "g:/CODE/app");
    }
}
