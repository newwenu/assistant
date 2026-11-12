use rand::Rng;
use std::fmt;
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum CipherAlgorithm {
    #[default]
    Aes256Gcm = 0,
    ChaCha20Poly1305 = 1,
}

impl CipherAlgorithm {
    pub fn discriminant(&self) -> u8 {
        *self as u8
    }

    pub fn from_discriminant(byte: u8) -> Result<Self, String> {
        match byte {
            0 => Ok(Self::Aes256Gcm),
            1 => Ok(Self::ChaCha20Poly1305),
            _ => Err(format!("Unknown cipher algorithm discriminant: {}", byte)),
        }
    }
}

pub struct VaultPlaintext {
    inner: Vec<u8>,
}

impl VaultPlaintext {
    pub fn expose(&self) -> &[u8] {
        &self.inner
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn new_str(s: &str) -> Self {
        Self::from(s)
    }

    pub fn to_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.expose())
    }
}

impl From<&str> for VaultPlaintext {
    fn from(s: &str) -> Self {
        Self {
            inner: s.as_bytes().to_vec(),
        }
    }
}

impl From<Vec<u8>> for VaultPlaintext {
    fn from(other: Vec<u8>) -> Self {
        Self { inner: other }
    }
}

impl Drop for VaultPlaintext {
    fn drop(&mut self) {
        self.inner.zeroize();
    }
}

impl fmt::Debug for VaultPlaintext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VaultPlaintext([REDACTED; {} bytes])", self.inner.len())
    }
}

pub struct SecureString {
    inner: String,
}

impl SecureString {
    pub fn new(s: String) -> Self {
        Self { inner: s }
    }

    pub fn expose_str(&self) -> &str {
        &self.inner
    }

    /// 转为 `Zeroizing<String>`：离开 `SecureString` 后仍由持有方负责清零。
    pub fn into_zeroizing(mut s: Self) -> Zeroizing<String> {
        let inner = std::mem::take(&mut s.inner);
        std::mem::forget(s);
        Zeroizing::new(inner)
    }

    /// 仅供「立即消费并移交到不可清零边界」的场景使用（如序列化到 wire DTO）。
    #[deprecated(note = "明文会脱离清零保护；优先用 into_zeroizing，确需移交时在调用点显式说明")]
    pub fn into_inner(mut s: Self) -> String {
        let inner = std::mem::take(&mut s.inner);
        std::mem::forget(s);
        inner
    }
}

impl Drop for SecureString {
    fn drop(&mut self) {
        let bytes = unsafe { self.inner.as_bytes_mut() };
        bytes.zeroize();
    }
}

impl fmt::Debug for SecureString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecureString([REDACTED; {} chars])", self.inner.len())
    }
}

pub struct PerEntryKey([u8; 32]);

impl PerEntryKey {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn ct_eq(&self, other: &PerEntryKey) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl From<[u8; 32]> for PerEntryKey {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl Zeroize for PerEntryKey {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

impl Drop for PerEntryKey {
    fn drop(&mut self) {
        self.zeroize();
    }
}

pub struct Dek([u8; 20]);

impl Dek {
    pub fn new(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }

    pub fn generate() -> Self {
        Self(rand::thread_rng().gen::<[u8; 20]>())
    }

    pub fn expose(&self) -> &[u8; 20] {
        &self.0
    }

    pub fn to_bytes(&self) -> [u8; 20] {
        self.0
    }

    pub fn ct_eq(&self, other: &Dek) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl Zeroize for Dek {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

impl Drop for Dek {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl fmt::Debug for Dek {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Dek([REDACTED; {} bytes])", self.0.len())
    }
}

macro_rules! define_32_byte_key {
    ($name:ident) => {
        pub struct $name([u8; 32]);

        impl $name {
            pub fn new(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            pub fn expose(&self) -> &[u8; 32] {
                &self.0
            }

            pub fn to_bytes(&self) -> [u8; 32] {
                self.0
            }

            pub fn ct_eq(&self, other: &$name) -> bool {
                self.0.ct_eq(&other.0).into()
            }
        }

        impl From<[u8; 32]> for $name {
            fn from(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }
        }

        impl Zeroize for $name {
            fn zeroize(&mut self) {
                self.0.zeroize();
            }
        }

        impl Drop for $name {
            fn drop(&mut self) {
                self.zeroize();
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(
                    f,
                    concat!(stringify!($name), "([REDACTED; {} bytes])"),
                    self.0.len()
                )
            }
        }
    };
}

define_32_byte_key!(Kek);
define_32_byte_key!(DbKey);
define_32_byte_key!(MetaKey);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dek_keeps_exactly_twenty_bytes() {
        let dek = Dek::new([7u8; 20]);
        assert_eq!(dek.expose().len(), 20);
        assert_eq!(dek.to_bytes().len(), 20);
    }

    #[test]
    fn dek_generate_is_not_all_zero_and_differs() {
        let a = Dek::generate();
        let b = Dek::generate();
        assert_ne!(a.to_bytes(), [0u8; 20], "随机 dek 不得为全零");
        assert!(!a.ct_eq(&b), "两次生成的 dek 必须不同");
        assert!(a.ct_eq(&Dek::new(a.to_bytes())));
    }

    #[test]
    fn dek_debug_is_redacted() {
        let dek = Dek::new([0xAA; 20]);
        let rendered = format!("{:?}", dek);
        assert!(rendered.contains("REDACTED"), "{rendered}");
        assert!(!rendered.contains("170"), "Debug 不得泄露字节: {rendered}");
    }

    #[test]
    fn derived_keys_keep_exactly_thirty_two_bytes() {
        assert_eq!(Kek::new([1u8; 32]).expose().len(), 32);
        assert_eq!(DbKey::new([2u8; 32]).to_bytes().len(), 32);
        assert_eq!(MetaKey::new([3u8; 32]).expose().len(), 32);
    }

    #[test]
    fn derived_keys_debug_is_redacted() {
        let kek = format!("{:?}", Kek::new([0xBB; 32]));
        let db_key = format!("{:?}", DbKey::new([0xBB; 32]));
        let meta_key = format!("{:?}", MetaKey::new([0xBB; 32]));

        for (name, rendered) in [
            ("Kek", kek.as_str()),
            ("DbKey", db_key.as_str()),
            ("MetaKey", meta_key.as_str()),
        ] {
            assert!(rendered.starts_with(name), "{rendered}");
            assert!(rendered.contains("REDACTED"), "{rendered}");
        }
    }

    #[test]
    fn derived_keys_compare_in_constant_time_api() {
        assert!(Kek::new([9u8; 32]).ct_eq(&Kek::new([9u8; 32])));
        assert!(!Kek::new([9u8; 32]).ct_eq(&Kek::new([8u8; 32])));
    }
}
