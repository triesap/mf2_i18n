use std::collections::BTreeMap;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedMessageKey {
    pub id: String,
    pub variant: String,
}

#[derive(Debug, Error)]
pub enum GeneratedKeyError {
    #[error("message key {key:?} cannot be converted to a Rust variant: {reason}")]
    InvalidMessageKey { key: String, reason: &'static str },
    #[error(
        "message keys {first_key:?} and {duplicate_key:?} both generate Rust variant {variant}"
    )]
    VariantCollision {
        variant: String,
        first_key: String,
        duplicate_key: String,
    },
}

pub fn build_generated_message_keys(
    keys: &[String],
) -> Result<Vec<GeneratedMessageKey>, GeneratedKeyError> {
    let mut variants = BTreeMap::<String, String>::new();
    let mut generated = Vec::with_capacity(keys.len());
    for key in keys {
        let variant = message_key_variant(key)?;
        if let Some(first_key) = variants.insert(variant.clone(), key.clone()) {
            return Err(GeneratedKeyError::VariantCollision {
                variant,
                first_key,
                duplicate_key: key.clone(),
            });
        }
        generated.push(GeneratedMessageKey {
            id: key.clone(),
            variant,
        });
    }
    Ok(generated)
}

pub fn message_key_variant(key: &str) -> Result<String, GeneratedKeyError> {
    let tokens = message_key_tokens(key)?;
    let mut variant = String::new();
    for token in tokens {
        push_pascal_token(&mut variant, &token);
    }
    if variant
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_digit())
    {
        variant.insert_str(0, "Key");
    }
    if is_reserved_variant(&variant) {
        variant.push_str("Message");
    }
    Ok(variant)
}

fn message_key_tokens(key: &str) -> Result<Vec<String>, GeneratedKeyError> {
    if key.trim().is_empty() {
        return Err(invalid_key(key, "message key must not be empty"));
    }

    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in key.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch);
        } else if matches!(ch, '.' | '-' | '_') {
            push_token(&mut tokens, &mut current);
        } else {
            return Err(invalid_key(
                key,
                "message key must contain only ASCII letters, digits, dots, hyphens, and underscores",
            ));
        }
    }
    push_token(&mut tokens, &mut current);

    if tokens.is_empty() {
        return Err(invalid_key(
            key,
            "message key must contain at least one ASCII letter or digit",
        ));
    }
    Ok(tokens)
}

fn push_token(tokens: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        tokens.push(std::mem::take(current));
    }
}

fn push_pascal_token(output: &mut String, token: &str) {
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return;
    };
    output.push(first.to_ascii_uppercase());
    for ch in chars {
        output.push(ch.to_ascii_lowercase());
    }
}

fn is_reserved_variant(value: &str) -> bool {
    matches!(value, "Self" | "Super" | "Crate")
}

fn invalid_key(key: &str, reason: &'static str) -> GeneratedKeyError {
    GeneratedKeyError::InvalidMessageKey {
        key: key.to_owned(),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::{GeneratedKeyError, build_generated_message_keys, message_key_variant};

    #[test]
    fn generated_variants_use_pascal_case_for_common_separators() {
        assert_eq!(
            message_key_variant("common.home.title").unwrap(),
            "CommonHomeTitle"
        );
        assert_eq!(
            message_key_variant("common-home_title").unwrap(),
            "CommonHomeTitle"
        );
        assert_eq!(
            message_key_variant("oauth.2fa.status").unwrap(),
            "Oauth2faStatus"
        );
    }

    #[test]
    fn generated_variants_prefix_leading_numbers() {
        assert_eq!(message_key_variant("404.title").unwrap(), "Key404Title");
    }

    #[test]
    fn generated_variants_avoid_reserved_rust_paths() {
        assert_eq!(message_key_variant("self").unwrap(), "SelfMessage");
        assert_eq!(message_key_variant("super").unwrap(), "SuperMessage");
        assert_eq!(message_key_variant("crate").unwrap(), "CrateMessage");
    }

    #[test]
    fn generated_variants_reject_invalid_keys() {
        assert!(matches!(
            message_key_variant(""),
            Err(GeneratedKeyError::InvalidMessageKey { key, .. }) if key.is_empty()
        ));
        assert!(matches!(
            message_key_variant("common/home"),
            Err(GeneratedKeyError::InvalidMessageKey { key, .. }) if key == "common/home"
        ));
        assert!(matches!(
            message_key_variant("..."),
            Err(GeneratedKeyError::InvalidMessageKey { key, .. }) if key == "..."
        ));
    }

    #[test]
    fn generated_variants_reject_collisions() {
        let err = build_generated_message_keys(&[
            "common.home-title".to_string(),
            "common.home_title".to_string(),
        ])
        .expect_err("collision");

        assert!(matches!(
            err,
            GeneratedKeyError::VariantCollision { variant, first_key, duplicate_key }
                if variant == "CommonHomeTitle"
                    && first_key == "common.home-title"
                    && duplicate_key == "common.home_title"
        ));
    }
}
