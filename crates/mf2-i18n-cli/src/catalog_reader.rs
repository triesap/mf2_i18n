use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use thiserror::Error;

use crate::catalog::Catalog;
use crate::model::MessageSpec;

#[derive(Debug, Error)]
pub enum CatalogReadError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid id map hash")]
    InvalidHash,
}

#[derive(Debug)]
pub struct CatalogBundle {
    pub catalog: Catalog,
    pub message_specs: BTreeMap<String, MessageSpec>,
    pub id_map_hash: [u8; 32],
}

pub fn load_catalog(
    catalog_path: &Path,
    id_map_hash_path: &Path,
) -> Result<CatalogBundle, CatalogReadError> {
    let catalog_bytes = fs::read_to_string(catalog_path)?;
    let catalog: Catalog = serde_json::from_str(&catalog_bytes)?;
    let id_map_hash = read_id_map_hash(id_map_hash_path)?;

    let mut message_specs = BTreeMap::new();
    for message in &catalog.messages {
        message_specs.insert(
            message.key.clone(),
            MessageSpec {
                key: message.key.clone(),
                args: message.args.clone(),
            },
        );
    }

    Ok(CatalogBundle {
        catalog,
        message_specs,
        id_map_hash,
    })
}

fn read_id_map_hash(path: &Path) -> Result<[u8; 32], CatalogReadError> {
    let contents = fs::read_to_string(path)?;
    let value = contents.trim();
    let hex = value.strip_prefix("sha256:").unwrap_or(value);
    let bytes = hex::decode(hex).map_err(|_| CatalogReadError::InvalidHash)?;
    if bytes.len() != 32 {
        return Err(CatalogReadError::InvalidHash);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::load_catalog;
    use crate::catalog::{Catalog, CatalogFeatures, CatalogMessage};
    use crate::model::{ArgSpec, ArgType};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str, ext: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_{name}_{nanos}.{ext}"));
        path
    }

    #[test]
    fn loads_catalog_and_hash() {
        let catalog_path = temp_path("catalog", "json");
        let hash_path = temp_path("hash", "txt");
        let catalog = Catalog {
            schema: 1,
            project: "demo".to_string(),
            generated_at: "2026-02-01T00:00:00Z".to_string(),
            default_locale: "en".to_string(),
            messages: vec![CatalogMessage {
                key: "home.title".to_string(),
                id: 42,
                args: vec![ArgSpec {
                    name: "name".to_string(),
                    arg_type: ArgType::String,
                    required: true,
                }],
                features: CatalogFeatures::default(),
                source_refs: None,
            }],
        };
        fs::write(&catalog_path, serde_json::to_string(&catalog).unwrap()).unwrap();
        fs::write(&hash_path, "sha256:000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
            .unwrap();

        let bundle = load_catalog(&catalog_path, &hash_path).expect("load");
        assert_eq!(bundle.message_specs.len(), 1);

        fs::remove_file(&catalog_path).ok();
        fs::remove_file(&hash_path).ok();
    }
}
