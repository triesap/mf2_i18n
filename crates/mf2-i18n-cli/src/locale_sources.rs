use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::mf2_source::parse_mf2_source;

#[derive(Debug, Clone)]
pub struct LocaleMessage {
    pub key: String,
    pub value: String,
    pub file: String,
    pub line: u32,
}

#[derive(Debug, Clone)]
pub struct LocaleBundle {
    pub locale: String,
    pub messages: BTreeMap<String, LocaleMessage>,
}

#[derive(Debug, Error)]
pub enum LocaleSourceError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("source parse error: {0}")]
    Parse(String),
    #[error("duplicate key {0} in locale {1}")]
    DuplicateKey(String, String),
    #[error("no locales found")]
    NoLocales,
}

pub fn load_locales(roots: &[PathBuf]) -> Result<Vec<LocaleBundle>, LocaleSourceError> {
    let mut bundles = Vec::new();
    for root in roots {
        let entries = fs::read_dir(root)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let locale = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_string();
            let messages = load_locale_dir(&path, &locale)?;
            bundles.push(LocaleBundle { locale, messages });
        }
    }
    if bundles.is_empty() {
        return Err(LocaleSourceError::NoLocales);
    }
    Ok(bundles)
}

fn load_locale_dir(path: &Path, locale: &str) -> Result<BTreeMap<String, LocaleMessage>, LocaleSourceError> {
    let mut messages = BTreeMap::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_path = entry.path();
        if file_path.extension().and_then(|ext| ext.to_str()) != Some("mf2") {
            continue;
        }
        let contents = fs::read_to_string(&file_path)?;
        let entries = parse_mf2_source(&contents).map_err(|err| {
            LocaleSourceError::Parse(format!("{}:{} {}", file_path.display(), err.line, err.message))
        })?;
        for entry in entries {
            if messages.contains_key(&entry.key) {
                return Err(LocaleSourceError::DuplicateKey(entry.key, locale.to_string()));
            }
            messages.insert(
                entry.key.clone(),
                LocaleMessage {
                    key: entry.key,
                    value: entry.value,
                    file: file_path.display().to_string(),
                    line: entry.line,
                },
            );
        }
    }
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::load_locales;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_locales_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    #[test]
    fn loads_locales_from_root() {
        let dir = temp_dir();
        let locale_dir = dir.join("en");
        fs::create_dir_all(&locale_dir).expect("locale");
        fs::write(locale_dir.join("messages.mf2"), "home.title = Hi").expect("write");

        let locales = load_locales(&[dir.clone()]).expect("load");
        assert_eq!(locales.len(), 1);
        assert!(locales[0].messages.contains_key("home.title"));

        fs::remove_dir_all(&dir).ok();
    }
}
