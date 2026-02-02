use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::config::load_config_or_default;
use crate::error::CliError;
use crate::locale_sources::{load_locales, LocaleSourceError};

#[derive(Debug, Error)]
pub enum PseudoCommandError {
    #[error("config error: {0}")]
    Config(#[from] CliError),
    #[error(transparent)]
    Sources(#[from] LocaleSourceError),
    #[error("unknown locale {0}")]
    UnknownLocale(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct PseudoOptions {
    pub locale: String,
    pub target: String,
    pub out_dir: PathBuf,
    pub config_path: PathBuf,
}

pub fn run_pseudo(options: &PseudoOptions) -> Result<(), PseudoCommandError> {
    let config = load_config_or_default(&options.config_path)?;
    let base_dir = options
        .config_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let roots: Vec<PathBuf> = config
        .source_dirs
        .iter()
        .map(|dir| base_dir.join(dir))
        .collect();
    let locales = load_locales(&roots)?;
    let source = locales
        .into_iter()
        .find(|bundle| bundle.locale == options.locale)
        .ok_or_else(|| PseudoCommandError::UnknownLocale(options.locale.clone()))?;

    let output_dir = options.out_dir.join(&options.target);
    fs::create_dir_all(&output_dir)?;

    let mut entries = BTreeMap::new();
    for (key, message) in source.messages {
        entries.insert(key, pseudolocalize_message(&message.value));
    }

    let out_path = output_dir.join("messages.mf2");
    let contents = serialize_entries(&entries);
    fs::write(out_path, contents)?;
    Ok(())
}

fn serialize_entries(entries: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    for (idx, (key, value)) in entries.iter().enumerate() {
        if idx > 0 {
            out.push_str("\n\n");
        }
        let mut lines = value.lines();
        if let Some(first) = lines.next() {
            out.push_str(key);
            out.push_str(" = ");
            out.push_str(first);
        } else {
            out.push_str(key);
            out.push_str(" = ");
        }
        for line in lines {
            out.push('\n');
            out.push_str(line);
        }
    }
    out
}

fn pseudolocalize_message(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }
    let mut output = String::from("[[");
    let mut depth = 0u32;
    for ch in input.chars() {
        match ch {
            '{' => {
                depth += 1;
                output.push(ch);
            }
            '}' => {
                if depth > 0 {
                    depth -= 1;
                }
                output.push(ch);
            }
            _ => {
                if depth > 0 {
                    output.push(ch);
                } else {
                    output.push_str(&pseudo_char(ch));
                }
            }
        }
    }
    output.push_str("]]");
    output
}

fn pseudo_char(ch: char) -> String {
    if !ch.is_ascii_alphabetic() {
        return ch.to_string();
    }
    let lower = ch.to_ascii_lowercase();
    let doubled = match lower {
        'a' | 'e' | 'i' | 'o' | 'u' => true,
        _ => false,
    };
    if doubled {
        let mut out = String::new();
        out.push(ch);
        out.push(ch);
        out
    } else {
        let mut out = String::new();
        out.push(ch);
        out.push('~');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::{pseudolocalize_message, run_pseudo, PseudoOptions};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_{name}_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    #[test]
    fn pseudo_preserves_expressions() {
        let input = "Hello { $name }";
        let out = pseudolocalize_message(input);
        assert!(out.contains("{ $name }"));
        assert!(out.starts_with("[["));
    }

    #[test]
    fn pseudo_command_writes_locale_file() {
        let root = temp_dir("pseudo_root");
        let locale_dir = root.join("en");
        fs::create_dir_all(&locale_dir).expect("locale");
        fs::write(locale_dir.join("messages.mf2"), "home.title = Hello").expect("write");

        let config_path = root.join("mf2-i18n.toml");
        fs::write(
            &config_path,
            "default_locale = \"en\"\nsource_dirs = [\".\"]\nmicro_locales_registry = \"micro-locales.toml\"\nproject_salt_path = \"tools/id_salt.txt\"\n",
        )
        .expect("write config");

        let out_dir = temp_dir("pseudo_out");
        let options = PseudoOptions {
            locale: "en".to_string(),
            target: "en-xa".to_string(),
            out_dir: out_dir.clone(),
            config_path,
        };
        run_pseudo(&options).expect("run");

        let output_file = out_dir.join("en-xa").join("messages.mf2");
        let contents = fs::read_to_string(&output_file).expect("read");
        assert!(contents.contains("home.title"));

        fs::remove_dir_all(&root).ok();
        fs::remove_dir_all(&out_dir).ok();
    }
}
