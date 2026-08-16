use alloc::collections::BTreeSet;

use crate::core::context::Qpid;

#[derive(Debug, Default)]
pub struct IdempotencyGuard {
    seen: BTreeSet<Qpid>,
    max_entries: usize,
}

impl IdempotencyGuard {
    pub fn new(max_entries: usize) -> Self {
        Self {
            seen: BTreeSet::new(),
            max_entries,
        }
    }

    pub fn check_qpid(&mut self, qpid: Qpid) -> bool {
        if self.seen.contains(&qpid) {
            return false;
        }

        self.seen.insert(qpid);
        if self.seen.len() > self.max_entries {
            // Deterministic bounded memory: drop smallest qpid.
            if let Some(oldest) = self.seen.iter().next().copied() {
                self.seen.remove(&oldest);
            }
        }
        true
    }
}
