use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MicroLocaleError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml error: {0}")]
    Toml(#[from] toml::de::Error),
}

#[derive(Debug, Deserialize)]
struct MicroLocaleFile {
    #[serde(default)]
    locale: Vec<MicroLocaleEntry>,
}

#[derive(Debug, Deserialize)]
struct MicroLocaleEntry {
    tag: String,
    parent: String,
}

pub fn load_micro_locales(path: &Path) -> Result<BTreeMap<String, String>, MicroLocaleError> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let contents = fs::read_to_string(path)?;
    let parsed: MicroLocaleFile = toml::from_str(&contents)?;
    let mut map = BTreeMap::new();
    for entry in parsed.locale {
        map.insert(entry.tag, entry.parent);
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::load_micro_locales;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_micro_{nanos}.toml"));
        path
    }

    #[test]
    fn loads_micro_locale_map() {
        let path = temp_path();
        fs::write(&path, "[[locale]]\ntag = \"en-x-test\"\nparent = \"en\"\n").expect("write");
        let map = load_micro_locales(&path).expect("load");
        assert_eq!(map.get("en-x-test"), Some(&"en".to_string()));
        fs::remove_file(&path).ok();
    }
}
