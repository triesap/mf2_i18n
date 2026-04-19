use ed25519_dalek::{Signature, VerifyingKey};

use crate::error::{RuntimeError, RuntimeResult};
use crate::manifest::Manifest;

pub fn verify_manifest_signature(
    manifest: &Manifest,
    key_id: &str,
    verifying_key: &VerifyingKey,
) -> RuntimeResult<()> {
    let signing = match &manifest.signing {
        Some(signing) => signing,
        None => return Ok(()),
    };
    if signing.key_id != key_id {
        return Err(RuntimeError::InvalidManifest("key id mismatch".to_string()));
    }
    if signing.sig_alg != "ed25519" {
        return Err(RuntimeError::InvalidManifest(
            "unsupported signature".to_string(),
        ));
    }
    let signature = parse_signature(&signing.manifest_sig)?;
    let bytes = manifest.to_signing_bytes()?;
    verifying_key
        .verify_strict(&bytes, &signature)
        .map_err(|_| RuntimeError::SignatureFailed)
}

fn parse_signature(value: &str) -> RuntimeResult<Signature> {
    let trimmed = value.trim();
    let hex = trimmed.strip_prefix("hex:").unwrap_or(trimmed);
    let bytes = hex::decode(hex)
        .map_err(|_| RuntimeError::InvalidManifest("invalid signature".to_string()))?;
    Signature::from_slice(&bytes)
        .map_err(|_| RuntimeError::InvalidManifest("invalid signature".to_string()))
}

#[cfg(test)]
mod tests {
    use super::verify_manifest_signature;
    use crate::manifest::{Manifest, ManifestSigning, PackEntry};
    use ed25519_dalek::{Signer, SigningKey};
    use std::collections::BTreeMap;

    #[test]
    fn verifies_manifest_signature() {
        let signing_key = SigningKey::from_bytes(&[9u8; 32]);
        let verifying_key = signing_key.verifying_key();
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
        let mut manifest = Manifest {
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

        let signature = signing_key.sign(&manifest.to_signing_bytes().expect("bytes"));
        manifest.signing = Some(ManifestSigning {
            sig_alg: "ed25519".to_string(),
            key_id: "key-1".to_string(),
            manifest_sig: format!("hex:{}", hex::encode(signature.to_bytes())),
        });

        verify_manifest_signature(&manifest, "key-1", &verifying_key).expect("verify");
    }
}
