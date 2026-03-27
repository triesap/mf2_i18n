use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub release_id: String,
    pub generated_at: String,
    pub default_locale: String,
    pub supported_locales: Vec<String>,
    pub id_map_hash: String,
    pub mf2_packs: BTreeMap<String, PackEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icu_packs: Option<BTreeMap<String, PackEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub micro_locales: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budgets: Option<BTreeMap<String, u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing: Option<ManifestSigning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackEntry {
    pub kind: String,
    pub url: String,
    pub hash: String,
    pub size: u64,
    pub content_encoding: String,
    pub pack_schema: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestSigning {
    pub sig_alg: String,
    pub key_id: String,
    pub manifest_sig: String,
}

impl Manifest {
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    pub fn to_signing_bytes(&self) -> Vec<u8> {
        let mut clone = self.clone();
        clone.signing = None;
        serde_json::to_vec(&clone).unwrap_or_default()
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(sha256_raw(bytes)))
}

pub fn sha256_raw(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::{Manifest, PackEntry, sha256_hex};
    use std::collections::BTreeMap;

    #[test]
    fn canonical_json_is_stable() {
        let mut mf2_packs = BTreeMap::new();
        mf2_packs.insert(
            "en".to_string(),
            PackEntry {
                kind: "base".to_string(),
                url: "packs/en.mf2pack".to_string(),
                hash: "sha256:abc".to_string(),
                size: 12,
                content_encoding: "br".to_string(),
                pack_schema: 0,
                parent: None,
            },
        );
        let manifest = Manifest {
            schema: 1,
            release_id: "r1".to_string(),
            generated_at: "2026-02-01T00:00:00Z".to_string(),
            default_locale: "en".to_string(),
            supported_locales: vec!["en".to_string()],
            id_map_hash: "sha256:dead".to_string(),
            mf2_packs,
            icu_packs: None,
            micro_locales: None,
            budgets: None,
            signing: None,
        };
        let bytes_a = manifest.to_canonical_bytes();
        let bytes_b = manifest.to_canonical_bytes();
        assert_eq!(bytes_a, bytes_b);
    }

    #[test]
    fn hashes_bytes_to_prefixed_hex() {
        let hash = sha256_hex(b"hello");
        assert!(hash.starts_with("sha256:"));
    }
}
