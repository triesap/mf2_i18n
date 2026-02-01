use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::catalog::Catalog;
use crate::error::CliError;
use crate::id_map::IdMap;

pub fn write_catalog(path: &Path, catalog: &Catalog) -> Result<(), CliError> {
    let file = fs::File::create(path)?;
    serde_json::to_writer_pretty(file, catalog)?;
    Ok(())
}

pub fn write_id_map(path: &Path, id_map: &IdMap) -> Result<(), CliError> {
    let mut entries: BTreeMap<String, u32> = BTreeMap::new();
    for (key, id) in id_map.entries() {
        entries.insert(key.to_string(), u32::from(id));
    }
    let file = fs::File::create(path)?;
    serde_json::to_writer_pretty(file, &entries)?;
    Ok(())
}

pub fn write_id_map_hash(path: &Path, hash: [u8; 32]) -> Result<(), CliError> {
    let mut file = fs::File::create(path)?;
    writeln!(file, "sha256:{}", hex_encode(hash))?;
    Ok(())
}

fn hex_encode(bytes: [u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{write_catalog, write_id_map, write_id_map_hash};
    use crate::catalog::{Catalog, CatalogFeatures, CatalogMessage};
    use crate::id_map::{build_id_map, derive_message_id};
    use crate::model::{ArgSpec, ArgType};
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

    #[test]
    fn writes_catalog_json() {
        let path = temp_path("catalog");
        let catalog = Catalog {
            schema: 1,
            project: "demo".to_string(),
            generated_at: "2026-02-01T00:00:00Z".to_string(),
            default_locale: "en".to_string(),
            messages: vec![CatalogMessage {
                key: "home.title".to_string(),
                id: 7,
                args: vec![ArgSpec {
                    name: "name".to_string(),
                    arg_type: ArgType::String,
                    required: true,
                }],
                features: CatalogFeatures::default(),
                source_refs: None,
            }],
        };
        write_catalog(&path, &catalog).expect("write catalog");
        let contents = fs::read_to_string(&path).expect("read");
        assert!(contents.contains("\"schema\""));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn writes_id_map_and_hash() {
        let salt = b"project-salt";
        let map = build_id_map(vec!["home.title".to_string()], salt).expect("map");
        let hash = map.hash().expect("hash");
        let id_path = temp_path("id_map");
        let hash_path = temp_path("id_map_hash");
        write_id_map(&id_path, &map).expect("write id map");
        write_id_map_hash(&hash_path, hash).expect("write hash");
        let contents = fs::read_to_string(&hash_path).expect("read");
        let expected = derive_message_id("home.title", salt);
        assert!(contents.starts_with("sha256:"));
        assert!(fs::read_to_string(&id_path).unwrap().contains(&u32::from(expected).to_string()));
        fs::remove_file(&id_path).ok();
        fs::remove_file(&hash_path).ok();
    }
}
