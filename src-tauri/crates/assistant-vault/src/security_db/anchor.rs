use super::meta::Anchor;

pub struct AnchorExpectation {
    pub meta_hash: String,
    pub dek_version: u32,
}

impl AnchorExpectation {
    pub fn new(meta_hash: impl Into<String>, dek_version: u32) -> Self {
        Self {
            meta_hash: meta_hash.into(),
            dek_version,
        }
    }

    pub fn from_anchor(anchor: &Anchor) -> Self {
        Self {
            meta_hash: anchor.meta_hash.clone(),
            dek_version: anchor.dek_version,
        }
    }
}
