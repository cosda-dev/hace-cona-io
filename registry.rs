use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
};

use spin::RwLock;

use crate::strategy::IoStrategy;

#[derive(Default)]
pub struct StrategyRegistry {
    entries: RwLock<BTreeMap<String, Arc<dyn IoStrategy>>>,
    fallback: RwLock<Option<Arc<dyn IoStrategy>>>,
}

impl StrategyRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, domain: &str, strategy: Arc<dyn IoStrategy>) {
        self.entries.write().insert(domain.to_string(), strategy);
    }

    pub fn unregister(&self, domain: &str) -> Option<Arc<dyn IoStrategy>> {
        self.entries.write().remove(domain)
    }

    pub fn clear(&self) {
        self.entries.write().clear();
    }

    pub fn set_fallback(&self, strategy: Arc<dyn IoStrategy>) {
        *self.fallback.write() = Some(strategy);
    }

    pub fn clear_fallback(&self) {
        *self.fallback.write() = None;
    }

    pub fn resolve(&self, domain: &str) -> Option<Arc<dyn IoStrategy>> {
        self.entries
            .read()
            .get(domain)
            .cloned()
            .or_else(|| self.fallback.read().clone())
    }
}
