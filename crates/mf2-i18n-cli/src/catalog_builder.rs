use thiserror::Error;

use crate::catalog::{Catalog, CatalogFeatures, CatalogMessage};
use crate::extract::ExtractedMessage;
use crate::id_map::{build_id_map, IdMap, IdMapError};

#[derive(Debug, Error)]
pub enum CatalogBuildError {
    #[error(transparent)]
    IdMap(#[from] IdMapError),
    #[error("missing message id for key {0}")]
    MissingKey(String),
}

#[derive(Debug)]
pub struct BuildOutput {
    pub catalog: Catalog,
    pub id_map: IdMap,
    pub id_map_hash: [u8; 32],
}

pub fn build_catalog(
    messages: &[ExtractedMessage],
    project: &str,
    default_locale: &str,
    generated_at: &str,
    salt: &[u8],
) -> Result<BuildOutput, CatalogBuildError> {
    let keys: Vec<String> = messages.iter().map(|message| message.key.clone()).collect();
    let id_map = build_id_map(keys, salt)?;
    let id_map_hash = id_map.hash()?;

    let mut catalog_messages = Vec::with_capacity(messages.len());
    for message in messages {
        let id = id_map
            .get(&message.key)
            .ok_or_else(|| CatalogBuildError::MissingKey(message.key.clone()))?;
        catalog_messages.push(CatalogMessage {
            key: message.key.clone(),
            id: u32::from(id),
            args: message.args.clone(),
            features: CatalogFeatures::default(),
            source_refs: None,
        });
    }

    let catalog = Catalog {
        schema: 1,
        project: project.to_string(),
        generated_at: generated_at.to_string(),
        default_locale: default_locale.to_string(),
        messages: catalog_messages,
    };

    Ok(BuildOutput {
        catalog,
        id_map,
        id_map_hash,
    })
}

#[cfg(test)]
mod tests {
    use crate::extract::ExtractedMessage;
    use crate::id_map::derive_message_id;
    use crate::lexer::Span;
    use crate::model::{ArgSpec, ArgType};

    use super::build_catalog;

    fn span() -> Span {
        Span {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        }
    }

    #[test]
    fn builds_catalog_with_ids() {
        let messages = vec![ExtractedMessage {
            key: "home.title".to_string(),
            args: vec![ArgSpec {
                name: "name".to_string(),
                arg_type: ArgType::String,
                required: true,
            }],
            span: span(),
        }];
        let salt = b"project-salt";
        let output = build_catalog(
            &messages,
            "demo",
            "en",
            "2026-02-01T00:00:00Z",
            salt,
        )
        .expect("build");

        let expected = derive_message_id("home.title", salt);
        assert_eq!(output.catalog.messages[0].id, u32::from(expected));
    }
}
