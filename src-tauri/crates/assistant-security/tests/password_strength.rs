use assistant_security::password_strength::{estimate_strength, StrengthWarning};

#[test]
fn weak_password_low_score_and_warnings() {
    let result = estimate_strength("123");
    assert!(result.score < 2);
    assert!(result.warnings.contains(&StrengthWarning::TooShort));
    assert!(result.warnings.contains(&StrengthWarning::NoUppercase));
    assert!(result.warnings.contains(&StrengthWarning::NoSpecialChars));
}

#[test]
fn strong_password_high_score_no_critical_warnings() {
    let result = estimate_strength("Kx9#mP2$vL5@nQ8!");
    assert!(result.score >= 3);
    assert!(!result.warnings.contains(&StrengthWarning::TooShort));
    assert!(!result.warnings.contains(&StrengthWarning::NoUppercase));
    assert!(!result.warnings.contains(&StrengthWarning::NoLowercase));
    assert!(!result.warnings.contains(&StrengthWarning::NoDigits));
    assert!(!result.warnings.contains(&StrengthWarning::NoSpecialChars));
}

#[test]
fn common_pattern_detected() {
    let result = estimate_strength("mypassword123");
    assert!(result.warnings.contains(&StrengthWarning::CommonPattern));
}

#[test]
fn repeated_chars_detected() {
    let result = estimate_strength("aaabbbccc");
    assert!(result.warnings.contains(&StrengthWarning::RepeatedChars));
}

#[test]
fn sequential_chars_detected() {
    let result = estimate_strength("abcdefg");
    assert!(result.warnings.contains(&StrengthWarning::SequentialChars));
}

#[test]
fn dictionary_word_detected() {
    let result = estimate_strength("password");
    assert!(result.warnings.contains(&StrengthWarning::DictionaryWord));
}

#[test]
fn missing_uppercase_warning() {
    let result = estimate_strength("lowercase123!");
    assert!(result.warnings.contains(&StrengthWarning::NoUppercase));
}

#[test]
fn missing_lowercase_warning() {
    let result = estimate_strength("UPPERCASE123!");
    assert!(result.warnings.contains(&StrengthWarning::NoLowercase));
}

#[test]
fn missing_digits_warning() {
    let result = estimate_strength("NoDigitsHere!");
    assert!(result.warnings.contains(&StrengthWarning::NoDigits));
}

#[test]
fn missing_special_chars_warning() {
    let result = estimate_strength("NoSpecialChars123");
    assert!(result.warnings.contains(&StrengthWarning::NoSpecialChars));
}

#[test]
fn crack_time_display_not_empty() {
    let result = estimate_strength("moderatePw1!");
    assert!(!result.crack_time_display.is_empty());
}

#[test]
fn entropy_increases_with_complexity() {
    let weak = estimate_strength("abc");
    let moderate = estimate_strength("Moderate1!");
    let strong = estimate_strength("Kx9#mP2$vL5@nQ8!wR4&");
    assert!(weak.entropy_bits < moderate.entropy_bits);
    assert!(moderate.entropy_bits < strong.entropy_bits);
}
