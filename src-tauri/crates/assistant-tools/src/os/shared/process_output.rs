pub use super::text_encoding::decode_process_output;

#[cfg(test)]
mod tests {
    use super::decode_process_output;
    use encoding_rs::GBK;

    #[test]
    fn utf8_passthrough() {
        let input = "ASCII / 中文 / end";
        assert_eq!(decode_process_output(input.as_bytes()), input);
    }

    #[test]
    fn gbk_decoded() {
        let (gbk_bytes, _, _) = GBK.encode("系统找不到指定的路径。");
        let decoded = decode_process_output(&gbk_bytes);
        assert_eq!(decoded, "系统找不到指定的路径。");
    }

    #[test]
    fn empty_input() {
        assert_eq!(decode_process_output(&[]), "");
    }
}
