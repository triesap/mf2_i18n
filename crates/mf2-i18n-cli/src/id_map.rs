use std::collections::BTreeMap;

use blake3::Hasher;
use mf2_i18n_core::MessageId;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IdMapError {
    #[error("message id collision for {id} between {existing} and {incoming}")]
    Collision {
        id: MessageId,
        existing: String,
        incoming: String,
    },
    #[error("key length exceeds u32 range: {len}")]
    KeyTooLong { len: usize },
}

#[derive(Debug, Clone)]
pub struct IdMap {
    entries: BTreeMap<String, MessageId>,
    reverse: BTreeMap<MessageId, String>,
}

impl IdMap {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            reverse: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, key: String, id: MessageId) -> Result<(), IdMapError> {
        if let Some(existing) = self.reverse.get(&id) {
            if existing != &key {
                return Err(IdMapError::Collision {
                    id,
                    existing: existing.clone(),
                    incoming: key,
                });
            }
        }
        self.entries.insert(key.clone(), id);
        self.reverse.insert(id, key);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<MessageId> {
        self.entries.get(key).copied()
    }

    pub fn entries(&self) -> impl Iterator<Item = (&str, MessageId)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), *v))
    }

    pub fn hash(&self) -> Result<[u8; 32], IdMapError> {
        let mut hasher = Sha256::new();
        for (key, id) in &self.entries {
            let len: u32 = key.len().try_into().map_err(|_| IdMapError::KeyTooLong {
                len: key.len(),
            })?;
            hasher.update(len.to_le_bytes());
            hasher.update(key.as_bytes());
            hasher.update(u32::from(*id).to_le_bytes());
        }
        Ok(hasher.finalize().into())
    }
}

impl Default for IdMap {
    fn default() -> Self {
        Self::new()
    }
}

pub fn derive_message_id(key: &str, salt: &[u8]) -> MessageId {
    let mut hasher = Hasher::new();
    hasher.update(salt);
    hasher.update(key.as_bytes());
    let hash = hasher.finalize();
    let bytes = hash.as_bytes();
    MessageId::new(u32::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3],
    ]))
}

pub fn build_id_map<I>(keys: I, salt: &[u8]) -> Result<IdMap, IdMapError>
where
    I: IntoIterator<Item = String>,
{
    let mut map = IdMap::new();
    for key in keys {
        let id = derive_message_id(&key, salt);
        map.insert(key, id)?;
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::{build_id_map, derive_message_id, IdMap, IdMapError};
    use mf2_i18n_core::MessageId;

    #[test]
    fn derives_message_id_deterministically() {
        let salt = b"project-salt";
        let id_a = derive_message_id("home.title", salt);
        let id_b = derive_message_id("home.title", salt);
        assert_eq!(id_a, id_b);
    }

    #[test]
    fn builds_id_map_and_hashes_stably() {
        let salt = b"project-salt";
        let keys = vec!["b".to_string(), "a".to_string()];
        let map = build_id_map(keys, salt).expect("map");
        assert!(map.get("a").is_some());
        let hash_a = map.hash().expect("hash");
        let hash_b = map.hash().expect("hash");
        assert_eq!(hash_a, hash_b);
    }

    #[test]
    fn detects_message_id_collisions() {
        let mut map = IdMap::new();
        map.insert("home.title".to_string(), MessageId::new(7))
            .expect("insert");
        let err = map
            .insert("home.subtitle".to_string(), MessageId::new(7))
            .expect_err("collision");
        assert!(matches!(err, IdMapError::Collision { .. }));
    }
}
