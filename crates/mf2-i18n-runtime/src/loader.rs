use std::fs;
use std::path::Path;

use crate::error::{RuntimeError, RuntimeResult};
use crate::id_map::IdMap;
use crate::manifest::Manifest;

pub fn load_manifest(path: &Path) -> RuntimeResult<Manifest> {
    let contents = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&contents)?)
}

pub fn load_id_map(path: &Path) -> RuntimeResult<IdMap> {
    let contents = fs::read_to_string(path)?;
    IdMap::from_json(&contents)
}

pub fn parse_sha256(value: &str) -> RuntimeResult<[u8; 32]> {
    let trimmed = value.trim();
    let hex = trimmed.strip_prefix("sha256:").unwrap_or(trimmed);
    let bytes = hex::decode(hex).map_err(|_| RuntimeError::InvalidHash)?;
    if bytes.len() != 32 {
        return Err(RuntimeError::InvalidHash);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::parse_sha256;

    #[test]
    fn parses_prefixed_hash() {
        let bytes =
            parse_sha256("sha256:000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
                .expect("hash");
        assert_eq!(bytes[0], 0);
    }
}
