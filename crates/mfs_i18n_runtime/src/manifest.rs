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
    pub fn to_signing_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut clone = self.clone();
        clone.signing = None;
        serde_json::to_vec(&clone)
    }
}

#[cfg(test)]
mod tests {
    use super::{Manifest, PackEntry};
    use std::collections::BTreeMap;

    #[test]
    fn signing_bytes_are_stable() {
        let mut mf2_packs = BTreeMap::new();
        mf2_packs.insert(
            "en".to_string(),
            PackEntry {
                kind: "base".to_string(),
                url: "packs/en.mf2pack".to_string(),
                hash: "sha256:abc".to_string(),
                size: 12,
                content_encoding: "identity".to_string(),
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
        let a = manifest.to_signing_bytes().expect("bytes");
        let b = manifest.to_signing_bytes().expect("bytes");
        assert_eq!(a, b);
    }
}
