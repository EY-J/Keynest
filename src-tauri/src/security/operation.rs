use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone, Default)]
pub(crate) struct SecurityOperationGate {
    inner: Arc<Mutex<()>>,
}

pub(crate) struct SecurityOperationGuard<'a> {
    gate: &'a SecurityOperationGate,
    _guard: MutexGuard<'a, ()>,
}

impl SecurityOperationGate {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn lock(&self) -> SecurityOperationGuard<'_> {
        SecurityOperationGuard {
            gate: self,
            _guard: self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        }
    }

    pub(crate) fn owns(&self, guard: &SecurityOperationGuard<'_>) -> bool {
        Arc::ptr_eq(&self.inner, &guard.gate.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::SecurityOperationGate;

    #[test]
    fn operation_gate_exposes_an_identity_checked_guard() {
        let first = SecurityOperationGate::new();
        let second = SecurityOperationGate::new();
        let guard = first.lock();

        assert!(first.owns(&guard));
        assert!(!second.owns(&guard));
    }
}
