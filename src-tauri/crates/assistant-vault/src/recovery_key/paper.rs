use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::types::Dek;
use crc::{Crc, CRC_32_ISCSI};
use zeroize::Zeroize;

const CRC32C: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);

pub const PAPER_CHARS: usize = 40;
pub const PAPER_GROUPS: usize = 5;
pub const PAPER_GROUP_LEN: usize = 8;
pub const PAPER_VERSION_BYTE: u8 = 0x02;
pub const CROCKFORD_ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

const PAPER_BYTES: usize = 25;
const DEK_BYTES: usize = 20;
const CRC_BYTES: usize = 4;
const CHECKSUM_RANGE: usize = 1 + DEK_BYTES;

pub fn encode_paper(dek: &Dek) -> String {
    let mut bytes = [0u8; PAPER_BYTES];
    bytes[0] = PAPER_VERSION_BYTE;
    bytes[1..CHECKSUM_RANGE].copy_from_slice(dek.expose());
    let crc = CRC32C.checksum(&bytes[..CHECKSUM_RANGE]);
    bytes[CHECKSUM_RANGE..].copy_from_slice(&crc.to_be_bytes());
    let encoded = base32_encode(&bytes);
    bytes.zeroize();
    encoded
}

pub fn format_groups(key: &str) -> Vec<String> {
    key.as_bytes()
        .chunks(PAPER_GROUP_LEN)
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect()
}

pub fn normalize_paper_input(raw: &str) -> String {
    let mut out = String::with_capacity(PAPER_CHARS);
    for ch in raw.chars() {
        let mapped = match ch {
            '\u{FF10}'..='\u{FF19}' => char::from_u32(ch as u32 - 0xFF10 + '0' as u32),
            '\u{FF21}'..='\u{FF3A}' => char::from_u32(ch as u32 - 0xFF21 + 'A' as u32),
            other => Some(other),
        };
        let Some(ch) = mapped else { continue };
        let upper = ch.to_ascii_uppercase();
        if matches!(upper, '-' | ' ' | '\t' | '\r' | '\n' | '\u{3000}') {
            continue;
        }
        let final_ch = match upper {
            'O' => '0',
            'I' | 'L' => '1',
            'U' => 'V',
            other => other,
        };
        out.push(final_ch);
    }
    out
}

pub fn decode_paper(raw: &str) -> Result<Dek, VaultError> {
    let normalized = normalize_paper_input(raw);
    if normalized.len() != PAPER_CHARS {
        return Err(VaultError::InvalidRecoveryKeyFormat);
    }
    let mut bytes = base32_decode(&normalized).ok_or(VaultError::InvalidRecoveryKeyFormat)?;
    if bytes.len() != PAPER_BYTES {
        bytes.zeroize();
        return Err(VaultError::InvalidRecoveryKeyFormat);
    }
    let version_ok = bytes[0] == PAPER_VERSION_BYTE;
    let checksum_ok = crc32c_matches(&bytes);
    if !version_ok {
        bytes.zeroize();
        return Err(VaultError::InvalidRecoveryKeyFormat);
    }
    if !checksum_ok {
        bytes.zeroize();
        return Err(VaultError::InvalidRecoveryKeyChecksum);
    }
    let mut dek_bytes = [0u8; DEK_BYTES];
    dek_bytes.copy_from_slice(&bytes[1..CHECKSUM_RANGE]);
    bytes.zeroize();
    let dek = Dek::new(dek_bytes);
    dek_bytes.zeroize();
    Ok(dek)
}

pub fn locate_checksum_error(raw: &str) -> Option<(u8, u8)> {
    let normalized = normalize_paper_input(raw);
    if normalized.len() != PAPER_CHARS {
        return None;
    }
    let mut hits: Vec<(u8, u8)> = Vec::new();
    let chars: Vec<u8> = normalized.bytes().collect();
    for index in 0..PAPER_CHARS {
        let original = chars[index];
        for candidate in CROCKFORD_ALPHABET.iter().copied() {
            if candidate == original {
                continue;
            }
            let mut attempt = chars.clone();
            attempt[index] = candidate;
            let Ok(text) = String::from_utf8(attempt) else {
                continue;
            };
            let mut bytes = match base32_decode(&text) {
                Some(bytes) => bytes,
                None => continue,
            };
            let ok = bytes[0] == PAPER_VERSION_BYTE && crc32c_matches(&bytes);
            bytes.zeroize();
            if ok {
                let group = u8::try_from(index / PAPER_GROUP_LEN + 1).ok()?;
                let position = u8::try_from(index % PAPER_GROUP_LEN + 1).ok()?;
                hits.push((group, position));
            }
        }
    }
    if hits.len() == 1 {
        hits.pop()
    } else {
        None
    }
}

fn crc32c_matches(bytes: &[u8]) -> bool {
    if bytes.len() != PAPER_BYTES {
        return false;
    }
    let expected = CRC32C.checksum(&bytes[..CHECKSUM_RANGE]);
    expected
        == u32::from_be_bytes([
            bytes[CHECKSUM_RANGE],
            bytes[CHECKSUM_RANGE + 1],
            bytes[CHECKSUM_RANGE + 2],
            bytes[CHECKSUM_RANGE + CRC_BYTES - 1],
        ])
}

fn base32_encode(bytes: &[u8; PAPER_BYTES]) -> String {
    let mut out = String::with_capacity(PAPER_CHARS);
    for index in 0..PAPER_CHARS {
        let bit_offset = index * 5;
        let byte_index = bit_offset / 8;
        let shift = bit_offset % 8;
        let mut value = (bytes[byte_index] as u16) << 8;
        if byte_index + 1 < PAPER_BYTES {
            value |= bytes[byte_index + 1] as u16;
        }
        let symbol = ((value >> (11 - shift)) & 0x1F) as usize;
        out.push(CROCKFORD_ALPHABET[symbol] as char);
    }
    out
}

fn base32_decode(text: &str) -> Option<[u8; PAPER_BYTES]> {
    let mut bytes = [0u8; PAPER_BYTES];
    for (index, ch) in text.bytes().enumerate() {
        let symbol = u16::try_from(CROCKFORD_ALPHABET.iter().position(|c| *c == ch)?).ok()?;
        let bit_offset = index * 5;
        let byte_index = bit_offset / 8;
        let shift = bit_offset % 8;
        let shifted = symbol << (11 - shift);
        bytes[byte_index] |= (shifted >> 8) as u8;
        if byte_index + 1 < PAPER_BYTES {
            bytes[byte_index + 1] |= (shifted & 0xFF) as u8;
        } else if (shifted & 0xFF) != 0 {
            bytes.zeroize();
            return None;
        }
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_dek() -> Dek {
        Dek::new([0x11; DEK_BYTES])
    }

    #[test]
    fn encode_paper_is_40_chars() {
        let key = encode_paper(&sample_dek());
        assert_eq!(key.len(), PAPER_CHARS);
        assert!(key.bytes().all(|b| CROCKFORD_ALPHABET.contains(&b)));
    }

    #[test]
    fn encode_decode_roundtrip() {
        for _ in 0..1000 {
            let dek = Dek::generate();
            let key = encode_paper(&dek);
            let decoded = decode_paper(&key).unwrap();
            assert!(dek.ct_eq(&decoded));
        }
    }

    #[test]
    fn format_groups_splits_into_five() {
        let groups = format_groups(&encode_paper(&sample_dek()));
        assert_eq!(groups.len(), PAPER_GROUPS);
        assert!(groups.iter().all(|g| g.len() == PAPER_GROUP_LEN));
    }

    #[test]
    fn normalization_accepts_mixed_input() {
        let key = encode_paper(&sample_dek());
        let messy: String = key
            .to_lowercase()
            .chars()
            .map(|c| match c {
                'o' => '０',
                '0' => 'O',
                '1' => 'l',
                'v' => 'U',
                other => other,
            })
            .collect();
        let spaced = format_groups(&messy).join(" - ");
        assert!(sample_dek().ct_eq(&decode_paper(&spaced).unwrap()));
    }

    #[test]
    fn single_char_swap_is_checksum_error() {
        let key = encode_paper(&sample_dek());
        let chars: Vec<char> = key.chars().collect();
        for index in 0..PAPER_CHARS {
            let original = chars[index];
            for candidate in CROCKFORD_ALPHABET {
                let symbol = *candidate as char;
                if symbol == original {
                    continue;
                }
                let mut attempt = chars.clone();
                attempt[index] = symbol;
                let text: String = attempt.into_iter().collect();
                let result = decode_paper(&text);
                assert!(
                    matches!(
                        result,
                        Err(VaultError::InvalidRecoveryKeyFormat)
                            | Err(VaultError::InvalidRecoveryKeyChecksum)
                    ),
                    "index {index} swap to {symbol} got {result:?}"
                );
                if index == 0 {
                    assert!(matches!(result, Err(VaultError::InvalidRecoveryKeyFormat)));
                } else if index > 1 {
                    assert!(matches!(
                        result,
                        Err(VaultError::InvalidRecoveryKeyChecksum)
                    ));
                }
            }
        }
    }

    #[test]
    fn truncated_or_illegal_input_is_format_error() {
        let key = encode_paper(&sample_dek());
        let truncated = &key[..PAPER_CHARS - 1];
        assert!(matches!(
            decode_paper(truncated),
            Err(VaultError::InvalidRecoveryKeyFormat)
        ));
        let illegal = format!("{key}!");
        assert!(matches!(
            decode_paper(&illegal),
            Err(VaultError::InvalidRecoveryKeyFormat)
        ));
        assert!(matches!(
            decode_paper(""),
            Err(VaultError::InvalidRecoveryKeyFormat)
        ));
    }

    #[test]
    fn locate_checksum_error_finds_exact_position() {
        let key = encode_paper(&sample_dek());
        let chars: Vec<char> = key.chars().collect();
        for index in [0usize, 7, 13, 24, 39] {
            let original = chars[index];
            let replacement = CROCKFORD_ALPHABET
                .iter()
                .copied()
                .find(|c| *c as char != original)
                .unwrap();
            let mut attempt = chars.clone();
            attempt[index] = replacement as char;
            let text: String = attempt.into_iter().collect();
            let located = locate_checksum_error(&text).unwrap();
            let index = u8::try_from(index).unwrap();
            assert_eq!(located, (index / 8 + 1, index % 8 + 1));
        }
    }

    #[test]
    fn locate_checksum_error_ignores_valid_key() {
        let key = encode_paper(&sample_dek());
        assert_eq!(locate_checksum_error(&key), None);
    }
}
