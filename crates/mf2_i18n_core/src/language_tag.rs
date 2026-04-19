use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::{CoreError, CoreResult};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LanguageTag {
    original: String,
    normalized: String,
    match_subtags: Vec<String>,
}

impl LanguageTag {
    pub fn parse(input: &str) -> CoreResult<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(CoreError::InvalidInput("language tag is empty"));
        }

        let subtags: Vec<&str> = trimmed.split('-').collect();
        if subtags.iter().any(|part| part.is_empty()) {
            return Err(CoreError::InvalidInput("language tag has empty subtag"));
        }

        let mut normalized_parts = Vec::with_capacity(subtags.len());
        let mut match_parts = Vec::with_capacity(subtags.len());
        let mut script_seen = false;
        let mut region_seen = false;
        let mut stop_for_match = false;

        for (idx, part) in subtags.iter().enumerate() {
            let part = part.trim();
            if idx == 0 {
                if !is_alpha(part) || !(2..=8).contains(&part.len()) {
                    return Err(CoreError::InvalidInput("invalid language subtag"));
                }
                let lower = part.to_ascii_lowercase();
                normalized_parts.push(lower.clone());
                match_parts.push(lower);
                continue;
            }

            if part.len() == 1 {
                stop_for_match = true;
                normalized_parts.push(part.to_ascii_lowercase());
                continue;
            }

            let normalized = if !script_seen && part.len() == 4 && is_alpha(part) {
                script_seen = true;
                titlecase(part)
            } else if !region_seen && is_region(part) {
                region_seen = true;
                part.to_ascii_uppercase()
            } else {
                part.to_ascii_lowercase()
            };

            normalized_parts.push(normalized.clone());
            if !stop_for_match {
                match_parts.push(normalized);
            }
        }

        let normalized = normalized_parts.join("-");

        Ok(Self {
            original: trimmed.to_string(),
            normalized,
            match_subtags: match_parts,
        })
    }

    pub fn original(&self) -> &str {
        &self.original
    }

    pub fn normalized(&self) -> &str {
        &self.normalized
    }

    pub fn match_subtags(&self) -> &[String] {
        &self.match_subtags
    }
}

fn is_alpha(value: &str) -> bool {
    value.chars().all(|ch| ch.is_ascii_alphabetic())
}

fn is_region(value: &str) -> bool {
    (value.len() == 2 && is_alpha(value))
        || (value.len() == 3 && value.chars().all(|ch| ch.is_ascii_digit()))
}

fn titlecase(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut output = String::with_capacity(value.len());
    output.push(first.to_ascii_uppercase());
    for ch in chars {
        output.push(ch.to_ascii_lowercase());
    }
    output
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use super::LanguageTag;

    #[test]
    fn normalize_language_script_region() {
        let tag = LanguageTag::parse("zh-hant-tw").expect("valid tag");
        assert_eq!(tag.normalized(), "zh-Hant-TW");
        assert_eq!(
            tag.match_subtags(),
            &[String::from("zh"), String::from("Hant"), String::from("TW")]
        );
    }

    #[test]
    fn stops_matching_on_extensions() {
        let tag = LanguageTag::parse("de-DE-u-co-phonebk").expect("valid tag");
        assert_eq!(tag.normalized(), "de-DE-u-co-phonebk");
        assert_eq!(
            tag.match_subtags(),
            &[String::from("de"), String::from("DE")]
        );
    }

    #[test]
    fn stops_matching_on_private_use() {
        let tag = LanguageTag::parse("es-PE-x-northperu").expect("valid tag");
        assert_eq!(tag.normalized(), "es-PE-x-northperu");
        assert_eq!(
            tag.match_subtags(),
            &[String::from("es"), String::from("PE")]
        );
    }

    #[test]
    fn rejects_empty_tag() {
        let err = LanguageTag::parse(" ").expect_err("empty tag should fail");
        assert_eq!(err, crate::CoreError::InvalidInput("language tag is empty"));
    }

    #[test]
    fn rejects_empty_subtag() {
        let err = LanguageTag::parse("en--US").expect_err("empty subtag should fail");
        assert_eq!(
            err,
            crate::CoreError::InvalidInput("language tag has empty subtag")
        );
    }
}
