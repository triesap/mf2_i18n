use std::collections::BTreeMap;

use mf2_i18n_core::MessageId;
use sha2::{Digest, Sha256};

use crate::error::{RuntimeError, RuntimeResult};

#[derive(Debug, Clone)]
pub struct IdMap {
    entries: BTreeMap<String, MessageId>,
}

impl IdMap {
    pub fn from_json(contents: &str) -> RuntimeResult<Self> {
        let map: BTreeMap<String, u32> = serde_json::from_str(contents)?;
        let mut entries = BTreeMap::new();
        for (key, id) in map {
            entries.insert(key, MessageId::new(id));
        }
        Ok(Self { entries })
    }

    pub fn from_bytes(contents: &[u8]) -> RuntimeResult<Self> {
        let map: BTreeMap<String, u32> = serde_json::from_slice(contents)?;
        let mut entries = BTreeMap::new();
        for (key, id) in map {
            entries.insert(key, MessageId::new(id));
        }
        Ok(Self { entries })
    }

    pub fn get(&self, key: &str) -> Option<MessageId> {
        self.entries.get(key).copied()
    }

    pub fn entries(&self) -> impl Iterator<Item = (&str, MessageId)> {
        self.entries.iter().map(|(key, id)| (key.as_str(), *id))
    }

    pub fn hash(&self) -> RuntimeResult<[u8; 32]> {
        let mut hasher = Sha256::new();
        for (key, id) in &self.entries {
            let len: u32 = key
                .len()
                .try_into()
                .map_err(|_| RuntimeError::InvalidIdMap)?;
            hasher.update(len.to_le_bytes());
            hasher.update(key.as_bytes());
            hasher.update(u32::from(*id).to_le_bytes());
        }
        Ok(hasher.finalize().into())
    }
}

#[cfg(test)]
mod tests {
    use super::IdMap;
    use mf2_i18n_core::MessageId;

    #[test]
    fn parses_id_map_json() {
        let json = r#"{"home.title": 7}"#;
        let map = IdMap::from_json(json).expect("map");
        let id = map.get("home.title").expect("id");
        assert_eq!(u32::from(id), 7);
    }

    #[test]
    fn parses_id_map_bytes() {
        let bytes = br#"{"home.title": 9}"#;
        let map = IdMap::from_bytes(bytes).expect("map");
        let id = map.get("home.title").expect("id");
        assert_eq!(u32::from(id), 9);
    }

    #[test]
    fn iterates_entries() {
        let json = r#"{"home.title": 7, "home.subtitle": 11}"#;
        let map = IdMap::from_json(json).expect("map");
        let entries: Vec<(&str, MessageId)> = map.entries().collect();
        assert_eq!(entries.len(), 2);
    }
}
