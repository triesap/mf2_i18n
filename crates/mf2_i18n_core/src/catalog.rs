use alloc::vec::Vec;

use crate::{BytecodeProgram, MessageId};

pub trait Catalog {
    fn lookup(&self, id: MessageId) -> Option<&BytecodeProgram>;
}

pub struct CatalogChain<'a> {
    catalogs: Vec<&'a dyn Catalog>,
}

impl<'a> CatalogChain<'a> {
    pub fn new(catalogs: Vec<&'a dyn Catalog>) -> Self {
        Self { catalogs }
    }

    pub fn lookup(&self, id: MessageId) -> Option<&'a BytecodeProgram> {
        for catalog in &self.catalogs {
            if let Some(message) = catalog.lookup(id) {
                return Some(message);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;
    use alloc::vec;
    use alloc::vec::Vec;

    use super::{Catalog, CatalogChain};
    use crate::{BytecodeProgram, MessageId, Opcode};

    struct TestCatalog {
        messages: BTreeMap<MessageId, BytecodeProgram>,
    }

    impl TestCatalog {
        fn new(pairs: Vec<(MessageId, BytecodeProgram)>) -> Self {
            let mut messages = BTreeMap::new();
            for (id, program) in pairs {
                messages.insert(id, program);
            }
            Self { messages }
        }
    }

    impl Catalog for TestCatalog {
        fn lookup(&self, id: MessageId) -> Option<&BytecodeProgram> {
            self.messages.get(&id)
        }
    }

    #[test]
    fn chain_prefers_first_catalog() {
        let mut primary = BytecodeProgram::new();
        primary.opcodes.push(Opcode::End);
        let mut secondary = BytecodeProgram::new();
        secondary.opcodes.push(Opcode::End);
        let id = MessageId::new(1);

        let primary_catalog = TestCatalog::new(vec![(id, primary)]);
        let secondary_catalog = TestCatalog::new(vec![(id, secondary)]);
        let chain = CatalogChain::new(vec![&primary_catalog, &secondary_catalog]);

        assert!(chain.lookup(id).is_some());
    }

    #[test]
    fn chain_falls_back_to_next_catalog() {
        let mut secondary = BytecodeProgram::new();
        secondary.opcodes.push(Opcode::End);
        let id = MessageId::new(2);

        let primary_catalog = TestCatalog::new(vec![]);
        let secondary_catalog = TestCatalog::new(vec![(id, secondary)]);
        let chain = CatalogChain::new(vec![&primary_catalog, &secondary_catalog]);

        assert!(chain.lookup(id).is_some());
    }
}
