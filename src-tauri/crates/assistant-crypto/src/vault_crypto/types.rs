use std::fmt;
use zeroize::Zeroize;

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

pub struct MasterKey([u8; 32]);

impl MasterKey {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<[u8; 32]> for MasterKey {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl Zeroize for MasterKey {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

impl Drop for MasterKey {
    fn drop(&mut self) {
        self.zeroize();
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
