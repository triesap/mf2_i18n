use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RichTextPart {
    Text(String),
    Slot { name: String },
}

impl RichTextPart {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    pub fn slot(name: impl Into<String>) -> Self {
        Self::Slot { name: name.into() }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            Self::Slot { .. } => None,
        }
    }

    pub fn slot_name(&self) -> Option<&str> {
        match self {
            Self::Text(_) => None,
            Self::Slot { name } => Some(name),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RichTextMessage {
    parts: Vec<RichTextPart>,
}

impl RichTextMessage {
    pub fn new(parts: Vec<RichTextPart>) -> Self {
        Self { parts }
    }

    pub fn plain(value: impl Into<String>) -> Self {
        Self {
            parts: vec![RichTextPart::Text(value.into())],
        }
    }

    pub fn parts(&self) -> &[RichTextPart] {
        &self.parts
    }

    pub fn into_parts(self) -> Vec<RichTextPart> {
        self.parts
    }

    pub fn push_text(&mut self, value: impl Into<String>) {
        self.parts.push(RichTextPart::Text(value.into()));
    }

    pub fn push_slot(&mut self, name: impl Into<String>) {
        self.parts.push(RichTextPart::Slot { name: name.into() });
    }

    pub fn is_plain_text(&self) -> bool {
        self.parts
            .iter()
            .all(|part| matches!(part, RichTextPart::Text(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::{RichTextMessage, RichTextPart};
    use alloc::vec;

    #[test]
    fn preserves_localized_text_and_slot_order() {
        let message = RichTextMessage::new(vec![
            RichTextPart::text("Read "),
            RichTextPart::slot("terms"),
            RichTextPart::text(" before "),
            RichTextPart::slot("signup"),
            RichTextPart::text("."),
        ]);

        assert_eq!(message.parts()[0].as_text(), Some("Read "));
        assert_eq!(message.parts()[1].slot_name(), Some("terms"));
        assert_eq!(message.parts()[3].slot_name(), Some("signup"));
        assert!(!message.is_plain_text());
    }

    #[test]
    fn identifies_plain_text_messages() {
        let message = RichTextMessage::plain("radroots");

        assert!(message.is_plain_text());
        assert_eq!(message.into_parts(), vec![RichTextPart::text("radroots")]);
    }
}
