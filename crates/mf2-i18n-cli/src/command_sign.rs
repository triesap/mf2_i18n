use std::fs;
use std::path::{Path, PathBuf};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use thiserror::Error;

use crate::manifest::{Manifest, ManifestSigning};

#[derive(Debug, Error)]
pub enum SignCommandError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid signing key")]
    InvalidKey,
    #[error("invalid key length {0}")]
    InvalidKeyLength(usize),
    #[error("signature verification failed")]
    SignatureFailed,
}

#[derive(Debug, Clone)]
pub struct SignOptions {
    pub manifest_path: PathBuf,
    pub key_path: PathBuf,
    pub key_id: String,
    pub out_path: Option<PathBuf>,
}

pub fn run_sign(options: &SignOptions) -> Result<(), SignCommandError> {
    let manifest_contents = fs::read_to_string(&options.manifest_path)?;
    let mut manifest: Manifest = serde_json::from_str(&manifest_contents)?;
    let signing_key = load_signing_key(&options.key_path)?;

    let signature = sign_manifest(&manifest, &signing_key, &options.key_id);
    manifest.signing = Some(signature);

    let out_path = options
        .out_path
        .as_ref()
        .unwrap_or(&options.manifest_path);
    let json = serde_json::to_string_pretty(&manifest)?;
    fs::write(out_path, json)?;
    Ok(())
}

fn sign_manifest(manifest: &Manifest, key: &SigningKey, key_id: &str) -> ManifestSigning {
    let bytes = manifest.to_signing_bytes();
    let signature = key.sign(&bytes);
    ManifestSigning {
        sig_alg: "ed25519".to_string(),
        key_id: key_id.to_string(),
        manifest_sig: format!("hex:{}", hex::encode(signature.to_bytes())),
    }
}

fn load_signing_key(path: &Path) -> Result<SigningKey, SignCommandError> {
    let contents = fs::read_to_string(path)?;
    let trimmed = contents.trim();
    let hex_text = trimmed.strip_prefix("hex:").unwrap_or(trimmed);
    let bytes = hex::decode(hex_text).map_err(|_| SignCommandError::InvalidKey)?;
    if bytes.len() != 32 {
        return Err(SignCommandError::InvalidKeyLength(bytes.len()));
    }
    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&bytes);
    Ok(SigningKey::from_bytes(&key_bytes))
}

fn verify_manifest_signature(
    manifest: &Manifest,
    signature: &Signature,
    verifying_key: &VerifyingKey,
) -> Result<(), SignCommandError> {
    let bytes = manifest.to_signing_bytes();
    verifying_key
        .verify_strict(&bytes, signature)
        .map_err(|_| SignCommandError::SignatureFailed)
}

#[cfg(test)]
mod tests {
    use super::{load_signing_key, sign_manifest, verify_manifest_signature, SignOptions};
    use crate::command_sign::run_sign;
    use crate::manifest::{Manifest, PackEntry};
    use ed25519_dalek::SigningKey;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_{name}_{nanos}.json"));
        path
    }

    fn sample_manifest() -> Manifest {
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
        Manifest {
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
        }
    }

    #[test]
    fn loads_signing_key_from_hex() {
        let path = temp_path("key");
        fs::write(&path, "hex:000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
            .expect("write");
        let key = load_signing_key(&path).expect("key");
        let bytes = key.to_bytes();
        assert_eq!(bytes[0], 0);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn signs_and_verifies_manifest() {
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let verifying_key = signing_key.verifying_key();
        let manifest = sample_manifest();
        let signing = sign_manifest(&manifest, &signing_key, "demo");
        let signature_bytes = hex::decode(signing.manifest_sig.trim_start_matches("hex:")).expect("hex");
        let signature = ed25519_dalek::Signature::from_slice(&signature_bytes).expect("sig");
        verify_manifest_signature(&manifest, &signature, &verifying_key).expect("verify");
    }

    #[test]
    fn run_sign_writes_signature() {
        let manifest_path = temp_path("manifest");
        let key_path = temp_path("signing_key");
        let out_path = temp_path("manifest_out");

        let manifest = sample_manifest();
        fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).expect("json"))
            .expect("write");
        fs::write(&key_path, hex::encode([3u8; 32])).expect("write");

        let options = SignOptions {
            manifest_path: manifest_path.clone(),
            key_path,
            key_id: "key-1".to_string(),
            out_path: Some(out_path.clone()),
        };
        run_sign(&options).expect("sign");
        let signed_contents = fs::read_to_string(&out_path).expect("read");
        assert!(signed_contents.contains("\"signing\""));

        fs::remove_file(&manifest_path).ok();
        fs::remove_file(&out_path).ok();
    }
}
