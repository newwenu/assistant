use std::time::Duration;

use serde::{Deserialize, Serialize};
use zxcvbn::zxcvbn;

const MIN_LENGTH: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrengthResult {
    pub score: u8,
    pub entropy_bits: f64,
    pub crack_time_seconds: u64,
    pub crack_time_display: String,
    pub warnings: Vec<StrengthWarning>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StrengthWarning {
    TooShort,
    NoUppercase,
    NoLowercase,
    NoDigits,
    NoSpecialChars,
    CommonPattern,
    RepeatedChars,
    SequentialChars,
    DictionaryWord,
}

pub fn estimate_strength(password: &str) -> StrengthResult {
    let mut warnings = Vec::new();

    if password.len() < MIN_LENGTH {
        warnings.push(StrengthWarning::TooShort);
    }

    let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
    let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password.chars().any(|c| !c.is_ascii_alphanumeric());

    if !has_upper {
        warnings.push(StrengthWarning::NoUppercase);
    }
    if !has_lower {
        warnings.push(StrengthWarning::NoLowercase);
    }
    if !has_digit {
        warnings.push(StrengthWarning::NoDigits);
    }
    if !has_special {
        warnings.push(StrengthWarning::NoSpecialChars);
    }

    if has_repeated_chars(password) {
        warnings.push(StrengthWarning::RepeatedChars);
    }
    if has_sequential_chars(password) {
        warnings.push(StrengthWarning::SequentialChars);
    }
    if is_common_pattern(password) {
        warnings.push(StrengthWarning::CommonPattern);
    }

    let entropy_result = zxcvbn(password, &[]);

    if entropy_result
        .feedback()
        .is_some_and(|fb| fb.warning().is_some())
    {
        warnings.push(StrengthWarning::DictionaryWord);
    }

    let score: u8 = entropy_result.score().into();
    let entropy_bits = entropy_result.guesses_log10() * 10.0_f64.log2();
    let crack_time_seconds: u64 = Duration::from(
        entropy_result
            .crack_times()
            .offline_slow_hashing_1e4_per_second(),
    )
    .as_secs();

    let crack_time_display = format_crack_time(crack_time_seconds);

    StrengthResult {
        score,
        entropy_bits,
        crack_time_seconds,
        crack_time_display,
        warnings,
    }
}

fn has_repeated_chars(password: &str) -> bool {
    let chars: Vec<char> = password.chars().collect();
    for i in 0..chars.len().saturating_sub(2) {
        if chars[i] == chars[i + 1] && chars[i + 1] == chars[i + 2] {
            return true;
        }
    }
    false
}

fn has_sequential_chars(password: &str) -> bool {
    let chars: Vec<char> = password.chars().collect();
    for i in 0..chars.len().saturating_sub(2) {
        let a = chars[i] as i32;
        let b = chars[i + 1] as i32;
        let c = chars[i + 2] as i32;
        if b - a == 1 && c - b == 1 {
            return true;
        }
        if a - b == 1 && b - c == 1 {
            return true;
        }
    }
    false
}

fn is_common_pattern(password: &str) -> bool {
    let lower = password.to_lowercase();
    const COMMON: &[&str] = &[
        "password", "123456", "qwerty", "abc123", "letmein", "admin", "welcome", "monkey",
        "dragon", "master", "login", "passw0rd",
    ];
    COMMON.iter().any(|&p| lower.contains(p))
}

fn format_crack_time(seconds: u64) -> String {
    if seconds == 0 {
        return "瞬间".to_string();
    }
    if seconds < 1 {
        return "不到 1 秒".to_string();
    }

    let units = [
        ("千年", 31536000000u64),
        ("世纪", 3153600000),
        ("十年", 315360000),
        ("年", 31536000),
        ("月", 2592000),
        ("周", 604800),
        ("天", 86400),
        ("小时", 3600),
        ("分钟", 60),
        ("秒", 1),
    ];

    let mut parts = Vec::new();
    let mut remaining = seconds;
    for (unit, s) in &units {
        let qty = remaining / s;
        if qty > 0 {
            parts.push(format!("{} {}", qty, unit));
            remaining %= s;
        }
        if parts.len() >= 2 {
            break;
        }
    }

    if parts.is_empty() {
        "不到 1 秒".to_string()
    } else {
        parts.join(" ")
    }
}
