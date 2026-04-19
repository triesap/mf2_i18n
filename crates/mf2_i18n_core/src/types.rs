use alloc::string::String;
use core::fmt;

use crate::{CoreError, CoreResult};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key(String);

impl Key {
    pub fn new(value: impl Into<String>) -> CoreResult<Self> {
        let value = value.into();
        if value.is_empty() {
            return Err(CoreError::InvalidInput("key is empty"));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for Key {
    type Error = CoreError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Key::new(value)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MessageId(u32);

impl MessageId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u32> for MessageId {
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

impl From<MessageId> for u32 {
    fn from(value: MessageId) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::{Key, MessageId};
    use alloc::string::ToString;

    #[test]
    fn key_rejects_empty() {
        let err = Key::new("").expect_err("empty key should fail");
        assert_eq!(err, crate::CoreError::InvalidInput("key is empty"));
    }

    #[test]
    fn key_accepts_non_empty() {
        let key = Key::new("home.title").expect("valid key");
        assert_eq!(key.as_str(), "home.title");
        assert_eq!(key.to_string(), "home.title");
    }

    #[test]
    fn message_id_round_trips() {
        let id = MessageId::new(42);
        assert_eq!(id.get(), 42);
        let raw: u32 = id.into();
        assert_eq!(raw, 42);
        let id = MessageId::from(7);
        assert_eq!(id.get(), 7);
    }
}
