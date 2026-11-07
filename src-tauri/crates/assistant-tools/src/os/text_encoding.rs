use chardetng::EncodingDetector;
use encoding_rs::{Encoding, IBM866, UTF_16BE, UTF_16LE, UTF_8, WINDOWS_1252};

/// 二进制探测窗口：只看前 8KB，避免为判定读入整个大文件。
pub const BINARY_PROBE_BYTES: usize = 8 * 1024;

/// 编码结论的来源。footer 的后缀由它决定（上级 §14.4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingSource {
    /// BOM 自述（优先级最高，覆盖显式 `encoding`）。
    Bom,
    /// 显式 `encoding` 参数生效。
    Explicit,
    /// 整文件通过 UTF-8 校验（默认路径，footer 静默）。
    Utf8,
    /// `chardetng` 猜测生效。
    Guess,
}

impl EncodingSource {
    /// footer 的来源后缀；`None` 表示不加后缀。
    pub fn suffix(self) -> Option<&'static str> {
        match self {
            Self::Bom => Some("bom"),
            Self::Guess => Some("guess"),
            Self::Utf8 | Self::Explicit => None,
        }
    }
}

/// 无损解码结果。`encoding` 是实际生效的编码，`name()` 即 footer 的 `<label>`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedText {
    pub text: String,
    pub encoding: &'static Encoding,
    pub has_bom: bool,
    pub source: EncodingSource,
}

impl DecodedText {
    pub fn encoding_name(&self) -> &'static str {
        self.encoding.name()
    }

    pub fn info(&self) -> DecodedInfo {
        DecodedInfo {
            encoding: self.encoding,
            has_bom: self.has_bom,
            source: self.source,
        }
    }
}

/// 流式读取的解码结论：只带 footer / 回执需要的字段，不含正文。
///
/// [`DecodedText`] 要求整文件驻留内存，`read` 改为分块解码后只能拿到这份元信息。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedInfo {
    pub encoding: &'static Encoding,
    pub has_bom: bool,
    pub source: EncodingSource,
}

impl DecodedInfo {
    pub fn encoding_name(&self) -> &'static str {
        self.encoding.name()
    }
}

/// 解码失败。三种情形都对应 `Business`，且都不允许写回（上级 §14.3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// 原始字节前 8KB 含 `0x00`：真二进制或无 BOM 的 UTF-16。
    Binary,
    /// 该编码无法无损表示全部字节。
    Lossy { encoding_name: &'static str },
    /// `chardetng` 置信度不足。
    Undetectable,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Binary => f.write_str(
                "file looks binary (NUL bytes in the first 8KB); if it is UTF-16 without a BOM, \
                 pass `encoding` explicitly",
            ),
            Self::Lossy { encoding_name } => write!(
                f,
                "content cannot be decoded as {encoding_name} without loss; pass the correct \
                 `encoding`"
            ),
            Self::Undetectable => f.write_str(
                "could not detect the encoding with confidence; pass `encoding` explicitly",
            ),
        }
    }
}

impl std::error::Error for DecodeError {}

/// 编码失败：文本含目标编码无法表示的字符（禁止 `?` 替换式写回，上级 §14.6 G2）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodeError {
    pub encoding_name: &'static str,
}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "content contains characters that cannot be encoded as {}; remove them or choose \
             another encoding",
            self.encoding_name
        )
    }
}

impl std::error::Error for EncodeError {}

/// 把模型 / 用户传入的 label 解析为规范编码；未知 label 返回 `None`。
pub fn resolve_label(label: &str) -> Option<&'static Encoding> {
    Encoding::for_label(label.trim().as_bytes())
}

/// 从工具入参对象里解析可选的 `encoding` 字段。
///
/// `read` / `write` / `grep` 共用同一套 label 校验，避免三个工具给出不同的别名表。
pub fn parse_optional_encoding(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<&'static Encoding>, String> {
    match object.get("encoding") {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(label)) => resolve_label(label).map(Some).ok_or_else(|| {
            format!(
                "unknown 'encoding' label '{}'; pass a WHATWG encoding label such as gbk, big5 \
                 or shift_jis",
                label
            )
        }),
        Some(_) => Err("'encoding' must be a string".to_string()),
    }
}

/// 写回时是否真的会写入 BOM：UTF-16 恒写；UTF-8 取决于 `has_bom`；其余编码没有 BOM。
///
/// 回执里的 `has_bom` 必须是这个结果，而不是调用方要求的 `has_bom`——
/// 否则「要求给 GBK 文件加 BOM」会写出一条与实际字节不符的回执。
pub fn writes_bom(encoding: &'static Encoding, has_bom: bool) -> bool {
    match bom_bytes(encoding) {
        Some(_) => has_bom || encoding == UTF_16LE || encoding == UTF_16BE,
        None => false,
    }
}

/// 按固定顺序检测并解码：BOM → 显式 `encoding` → 二进制判定 → UTF-8 校验 → 猜测。
///
/// BOM 是文件自述，优先级高于显式参数；猜测仅在「置信」且「整文件无损」时接受。
/// 二进制判定放在 UTF-8 校验之前：纯 ASCII 的 UTF-16（`h\0i\0`）本身是合法 UTF-8，
/// 顺序反过来会让它被当成「带 NUL 的文本」静默读入。
pub fn detect_and_decode(
    bytes: &[u8],
    explicit: Option<&'static Encoding>,
) -> Result<DecodedText, DecodeError> {
    if let Some((bom_encoding, bom_len)) = Encoding::for_bom(bytes) {
        let (text, had_errors) = decode_without_bom(bom_encoding, &bytes[bom_len..]);
        if had_errors {
            return Err(DecodeError::Lossy {
                encoding_name: bom_encoding.name(),
            });
        }
        return Ok(DecodedText {
            text,
            encoding: bom_encoding,
            has_bom: true,
            source: EncodingSource::Bom,
        });
    }

    if let Some(encoding) = explicit {
        let (text, had_errors) = decode_without_bom(encoding, bytes);
        if had_errors {
            return Err(DecodeError::Lossy {
                encoding_name: encoding.name(),
            });
        }
        return Ok(DecodedText {
            text,
            encoding,
            has_bom: false,
            source: EncodingSource::Explicit,
        });
    }

    if bytes[..bytes.len().min(BINARY_PROBE_BYTES)].contains(&0) {
        return Err(DecodeError::Binary);
    }

    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok(DecodedText {
            text: text.to_owned(),
            encoding: UTF_8,
            has_bom: false,
            source: EncodingSource::Utf8,
        });
    }

    let (encoding, is_confident) = guess_encoding(bytes);
    if !is_confident {
        return Err(DecodeError::Undetectable);
    }
    let (text, had_errors) = decode_without_bom(encoding, bytes);
    if had_errors {
        return Err(DecodeError::Lossy {
            encoding_name: encoding.name(),
        });
    }
    Ok(DecodedText {
        text,
        encoding,
        has_bom: false,
        source: EncodingSource::Guess,
    })
}

/// 按目标编码编码全文，有损即报错；BOM 按编码规范补写（UTF-16 恒补，UTF-8 由 `has_bom` 决定）。
pub fn encode_text(
    text: &str,
    encoding: &'static Encoding,
    has_bom: bool,
) -> Result<Vec<u8>, EncodeError> {
    if encoding == UTF_16LE || encoding == UTF_16BE {
        return Ok(encode_utf16(text, encoding));
    }

    let (encoded, used, had_errors) = encoding.encode(text);
    if had_errors {
        return Err(EncodeError {
            encoding_name: used.name(),
        });
    }

    let body = match Encoding::for_bom(&encoded) {
        Some((bom_encoding, bom_len)) if bom_encoding == used => &encoded[bom_len..],
        _ => &encoded[..],
    };

    let bom = bom_bytes(used).filter(|_| writes_bom(used, has_bom));
    let mut out = Vec::with_capacity(body.len() + bom.map_or(0, |bytes| bytes.len()));
    if let Some(bom) = bom {
        out.extend_from_slice(bom);
    }
    out.extend_from_slice(body);
    Ok(out)
}

/// `encoding_rs` 的 `encode` 对 UTF-16 走 Web 语义（输出 UTF-8），因此 UTF-16 写回自己拼。
///
/// UTF-16 能表示全部 Unicode 标量值，不存在不可编码字符，恒带 BOM（上级 §14.6 G5）。
fn encode_utf16(text: &str, encoding: &'static Encoding) -> Vec<u8> {
    let little_endian = encoding == UTF_16LE;
    let mut out = Vec::with_capacity(2 * (text.len() + 1));
    if let Some(bom) = bom_bytes(encoding) {
        out.extend_from_slice(bom);
    }
    for unit in text.encode_utf16() {
        let bytes = if little_endian {
            unit.to_le_bytes()
        } else {
            unit.to_be_bytes()
        };
        out.extend_from_slice(&bytes);
    }
    out
}

/// 进程输出的宽松解码：UTF-8 直通，否则猜一个编码解出文本，**不**拒绝、**不**处理 BOM。
///
/// 与 [`detect_and_decode`] 的差别只在「拒绝与否」：进程输出没有写回风险，
/// 交给模型一段可读文本比报错更有用。
pub fn decode_process_output(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => {
            let (encoding, _is_confident) = guess_encoding(bytes);
            decode_without_bom(encoding, bytes).0
        }
    }
}

fn decode_without_bom(encoding: &'static Encoding, bytes: &[u8]) -> (String, bool) {
    let mut decoder = encoding.new_decoder_without_bom_handling();
    let capacity = decoder
        .max_utf8_buffer_length(bytes.len())
        .unwrap_or(bytes.len() * 3);
    let mut text = String::with_capacity(capacity);
    let (_, read, had_errors) = decoder.decode_to_string(bytes, &mut text, true);
    // 尾部不完整的序列（如奇数字节的 UTF-16）不一定被消费：没读完就按有损处理，
    // 避免 debug 构建断言 panic、release 构建静默丢字节这两种行为分叉。
    (text, had_errors || read < bytes.len())
}

fn bom_bytes(encoding: &'static Encoding) -> Option<&'static [u8]> {
    if encoding == UTF_8 {
        Some(b"\xEF\xBB\xBF")
    } else if encoding == UTF_16LE {
        Some(b"\xFF\xFE")
    } else if encoding == UTF_16BE {
        Some(b"\xFE\xFF")
    } else {
        None
    }
}

/// 用 `chardetng` 猜一个编码，并附带置信标记；`IBM866 → WINDOWS_1252` 修正一并生效。
///
/// 调用方负责决定采样窗口：整文件读取传全部字节，流式读取传头部样本（见
/// [`crate::os::agent_read`] 的分块扫描）。
pub fn guess_encoding(bytes: &[u8]) -> (&'static Encoding, bool) {
    let mut detector = EncodingDetector::new();
    detector.feed(bytes, true);
    let (encoding, is_confident) = detector.guess_assess(None, true);

    if encoding == IBM866 && looks_like_windows_1252_punctuation(bytes) {
        return (WINDOWS_1252, is_confident);
    }

    (encoding, is_confident)
}

const WINDOWS_1252_PUNCT_BYTES: [u8; 8] = [0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x99];

fn looks_like_windows_1252_punctuation(bytes: &[u8]) -> bool {
    let mut saw_extended_punctuation = false;
    let mut saw_ascii_word = false;

    for &byte in bytes {
        if byte >= 0xA0 {
            return false;
        }
        if (0x80..=0x9F).contains(&byte) {
            if !WINDOWS_1252_PUNCT_BYTES.contains(&byte) {
                return false;
            }
            saw_extended_punctuation = true;
        }
        if byte.is_ascii_alphabetic() {
            saw_ascii_word = true;
        }
    }

    saw_extended_punctuation && saw_ascii_word
}

#[cfg(test)]
mod tests {
    use super::{detect_and_decode, encode_text, resolve_label, DecodeError, EncodingSource};
    use encoding_rs::{BIG5, GBK, SHIFT_JIS, UTF_16LE, UTF_8, WINDOWS_1252};

    #[test]
    fn utf8_plain_uses_fast_path() {
        let decoded = detect_and_decode("ASCII / 中文 / end".as_bytes(), None).unwrap();
        assert_eq!(decoded.text, "ASCII / 中文 / end");
        assert_eq!(decoded.encoding_name(), "UTF-8");
        assert!(!decoded.has_bom);
        assert_eq!(decoded.source, EncodingSource::Utf8);
    }

    #[test]
    fn utf8_with_bom_strips_bom_and_marks_source() {
        let bytes = b"\xEF\xBB\xBFhello";
        let decoded = detect_and_decode(bytes, None).unwrap();
        assert_eq!(decoded.text, "hello");
        assert_eq!(decoded.encoding_name(), "UTF-8");
        assert!(decoded.has_bom);
        assert_eq!(decoded.source, EncodingSource::Bom);
    }

    #[test]
    fn utf16le_with_bom_is_not_rejected_as_binary() {
        let mut bytes = Vec::from(b"\xFF\xFE");
        for unit in "中文".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }

        let decoded = detect_and_decode(&bytes, None).unwrap();
        assert_eq!(decoded.text, "中文");
        assert_eq!(decoded.encoding_name(), "UTF-16LE");
        assert!(decoded.has_bom);
        assert_eq!(decoded.source, EncodingSource::Bom);
    }

    #[test]
    fn ascii_utf16_without_bom_is_rejected_before_utf8_check() {
        let mut bytes = Vec::new();
        for unit in "hi".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(detect_and_decode(&bytes, None), Err(DecodeError::Binary));

        let decoded = detect_and_decode(&bytes, Some(UTF_16LE)).unwrap();
        assert_eq!(decoded.text, "hi");
        assert_eq!(decoded.source, EncodingSource::Explicit);
    }

    #[test]
    fn gbk_is_guessed() {
        let (bytes, _, had_errors) = GBK.encode("系统找不到指定的路径。");
        assert!(!had_errors);

        let decoded = detect_and_decode(&bytes, None).unwrap();
        assert_eq!(decoded.text, "系统找不到指定的路径。");
        assert_eq!(decoded.encoding_name(), "GBK");
        assert!(!decoded.has_bom);
        assert_eq!(decoded.source, EncodingSource::Guess);
    }

    #[test]
    fn truncated_utf16_is_reported_as_lossy() {
        let bytes = [0xFF, 0xFE, 0x41];
        assert_eq!(
            detect_and_decode(&bytes, None),
            Err(DecodeError::Lossy {
                encoding_name: "UTF-16LE"
            })
        );
    }

    #[test]
    fn big5_explicit() {
        let (bytes, _, had_errors) = BIG5.encode("繁體中文測試");
        assert!(!had_errors);

        let decoded = detect_and_decode(&bytes, Some(BIG5)).unwrap();
        assert_eq!(decoded.text, "繁體中文測試");
        assert_eq!(decoded.encoding_name(), "Big5");
        assert_eq!(decoded.source, EncodingSource::Explicit);
    }

    #[test]
    fn shift_jis_explicit() {
        let (bytes, _, had_errors) = SHIFT_JIS.encode("日本語のテスト");
        assert!(!had_errors);

        let decoded = detect_and_decode(&bytes, Some(SHIFT_JIS)).unwrap();
        assert_eq!(decoded.text, "日本語のテスト");
        assert_eq!(decoded.encoding_name(), "Shift_JIS");
        assert_eq!(decoded.source, EncodingSource::Explicit);
    }

    #[test]
    fn windows_1252_explicit() {
        let text = "café “quoted” — dash";
        let (bytes, _, had_errors) = WINDOWS_1252.encode(text);
        assert!(!had_errors);

        let decoded = detect_and_decode(&bytes, Some(WINDOWS_1252)).unwrap();
        assert_eq!(decoded.text, text);
        assert_eq!(decoded.encoding_name(), "windows-1252");
        assert_eq!(decoded.source, EncodingSource::Explicit);
    }

    #[test]
    fn binary_bytes_are_rejected() {
        let bytes = [0x00, 0x01, 0x02, b'a'];
        assert_eq!(detect_and_decode(&bytes, None), Err(DecodeError::Binary));
    }

    #[test]
    fn emoji_cannot_be_encoded_as_gbk() {
        let error = encode_text("emoji 😀", GBK, false).unwrap_err();
        assert_eq!(error.encoding_name, "GBK");
    }

    #[test]
    fn gbk_roundtrip_has_no_bom() {
        let bytes = encode_text("中文内容", GBK, false).unwrap();
        assert!(!bytes.starts_with(b"\xEF\xBB\xBF"));
        let decoded = detect_and_decode(&bytes, Some(GBK)).unwrap();
        assert_eq!(decoded.text, "中文内容");
    }

    #[test]
    fn utf8_bom_is_added_only_on_request() {
        let with_bom = encode_text("hello", UTF_8, true).unwrap();
        assert!(with_bom.starts_with(b"\xEF\xBB\xBF"));

        let without_bom = encode_text("hello", UTF_8, false).unwrap();
        assert!(!without_bom.starts_with(b"\xEF\xBB\xBF"));
    }

    #[test]
    fn utf16_always_gets_bom() {
        let bytes = encode_text("hi", UTF_16LE, false).unwrap();
        assert!(bytes.starts_with(b"\xFF\xFE"));
        let decoded = detect_and_decode(&bytes, None).unwrap();
        assert_eq!(decoded.text, "hi");
    }

    #[test]
    fn label_aliases_resolve_to_canonical_names() {
        assert_eq!(
            resolve_label("gb2312").map(|encoding| encoding.name()),
            Some("GBK")
        );
        assert_eq!(
            resolve_label(" GBK ").map(|encoding| encoding.name()),
            Some("GBK")
        );
        assert_eq!(resolve_label("bogus"), None);
    }
}
