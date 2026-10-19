use chardetng::EncodingDetector;
use encoding_rs::{Decoder, Encoding, IBM866, WINDOWS_1252};

pub fn decode_process_output(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => {
            let encoding = detect_encoding(bytes);
            let mut decoder = encoding.new_decoder_without_bom_handling();
            decode_with_decoder(&mut decoder, bytes, true)
        }
    }
}

fn decode_with_decoder(decoder: &mut Decoder, bytes: &[u8], last: bool) -> String {
    let capacity = decoder
        .max_utf8_buffer_length(bytes.len())
        .unwrap_or(bytes.len() * 3);
    let mut text = String::with_capacity(capacity);
    let (_, read, _) = decoder.decode_to_string(bytes, &mut text, last);
    debug_assert_eq!(read, bytes.len());
    text
}

fn detect_encoding(bytes: &[u8]) -> &'static Encoding {
    let mut detector = EncodingDetector::new();
    detector.feed(bytes, true);
    let (encoding, _is_confident) = detector.guess_assess(None, true);

    if encoding == IBM866 && looks_like_windows_1252_punctuation(bytes) {
        return WINDOWS_1252;
    }

    encoding
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
