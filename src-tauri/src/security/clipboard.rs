use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use thiserror::Error;
use zeroize::Zeroizing;

const ALLOWED_TIMEOUTS: [Duration; 3] = [
    Duration::from_secs(10),
    Duration::from_secs(30),
    Duration::from_secs(60),
];

pub(crate) trait ClipboardPort: Send + Sync {
    fn write_text(&self, value: &str) -> Result<(), ClipboardError>;
    fn read_text(&self) -> Result<String, ClipboardError>;
    fn clear(&self) -> Result<(), ClipboardError>;
}

#[derive(Clone)]
pub(crate) struct ClipboardService {
    inner: Arc<Mutex<ClipboardState>>,
    port: Arc<dyn ClipboardPort>,
    port_gate: Arc<Mutex<()>>,
}

struct ClipboardState {
    next_generation: u64,
    owned: Option<OwnedClipboard>,
    timeout: Duration,
}

struct OwnedClipboard {
    generation: u64,
    value: Zeroizing<String>,
}

struct CopyPlan {
    generation: u64,
    timeout: Duration,
}

impl ClipboardService {
    pub(crate) fn new(port: Arc<dyn ClipboardPort>, timeout: Duration) -> Self {
        debug_assert!(ALLOWED_TIMEOUTS.contains(&timeout));
        Self {
            inner: Arc::new(Mutex::new(ClipboardState {
                next_generation: 1,
                owned: None,
                timeout,
            })),
            port,
            port_gate: Arc::new(Mutex::new(())),
        }
    }

    pub(crate) fn copy_secret(&self, value: &str) -> Result<(), ClipboardError> {
        let plan = self.claim_secret(value)?;
        let service = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(plan.timeout);
            // Expiry errors are deliberately ignored here: they contain no clipboard data,
            // and a failed read must never turn into a blind clear.
            let _ = service.expire_generation(plan.generation);
        });
        Ok(())
    }

    pub(crate) fn set_timeout(&self, timeout: Duration) -> Result<(), ClipboardError> {
        if !ALLOWED_TIMEOUTS.contains(&timeout) {
            return Err(ClipboardError::InvalidTimeout);
        }
        self.lock_state().timeout = timeout;
        Ok(())
    }

    pub(crate) fn clear_if_owned(&self) -> Result<(), ClipboardError> {
        let _port_guard = self.lock_port_gate();
        let owned = self.lock_state().owned.take();
        self.clear_owned_value(owned)
    }

    pub(crate) fn clear_on_process_exit_best_effort(&self) {
        let service = self.clone();
        let _ = std::thread::spawn(move || service.clear_if_owned()).join();
    }

    fn claim_secret(&self, value: &str) -> Result<CopyPlan, ClipboardError> {
        let _port_guard = self.lock_port_gate();
        // Clipboard callbacks may block or re-enter the service, so no service lock is held here.
        self.port.write_text(value)?;

        let mut state = self.lock_state();
        let generation = state.next_generation;
        state.next_generation = match generation.checked_add(1) {
            Some(next) => next,
            None => {
                state.owned = None;
                drop(state);
                self.discard_unowned_write(value);
                return Err(ClipboardError::GenerationExhausted);
            }
        };
        let timeout = state.timeout;
        state.owned = Some(OwnedClipboard {
            generation,
            value: Zeroizing::new(value.to_owned()),
        });
        Ok(CopyPlan {
            generation,
            timeout,
        })
    }

    fn discard_unowned_write(&self, value: &str) {
        if self
            .port
            .read_text()
            .map(Zeroizing::new)
            .is_ok_and(|current| current.as_str() == value)
        {
            let _ = self.port.clear();
        }
    }

    fn expire_generation(&self, generation: u64) -> Result<(), ClipboardError> {
        let _port_guard = self.lock_port_gate();
        let owned = {
            let mut state = self.lock_state();
            match state.owned.as_ref() {
                Some(owned) if owned.generation == generation => state.owned.take(),
                _ => None,
            }
        };
        self.clear_owned_value(owned)
    }

    fn clear_owned_value(&self, owned: Option<OwnedClipboard>) -> Result<(), ClipboardError> {
        let Some(owned) = owned else {
            return Ok(());
        };

        let current = Zeroizing::new(self.port.read_text()?);
        if current.as_str() == owned.value.as_str() {
            self.port.clear()?;
        }
        Ok(())
    }

    fn lock_state(&self) -> MutexGuard<'_, ClipboardState> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_port_gate(&self) -> MutexGuard<'_, ()> {
        self.port_gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[cfg(test)]
    fn copy_plan_for_test(&self, value: &str) -> Result<CopyPlan, ClipboardError> {
        self.claim_secret(value)
    }

    #[cfg(test)]
    fn copy_secret_for_test(&self, value: &str) -> Result<u64, ClipboardError> {
        self.claim_secret(value).map(|plan| plan.generation)
    }

    #[cfg(test)]
    fn expire_generation_for_test(&self, generation: u64) -> Result<(), ClipboardError> {
        self.expire_generation(generation)
    }

    #[cfg(test)]
    fn has_owned_value_for_test(&self) -> bool {
        self.lock_state().owned.is_some()
    }
}

#[derive(Debug, Error)]
pub(crate) enum ClipboardError {
    #[error("clipboard write failed")]
    WriteFailed,
    #[error("clipboard read failed")]
    ReadFailed,
    #[error("clipboard clear failed")]
    ClearFailed,
    #[error("clipboard clear duration is not supported")]
    InvalidTimeout,
    #[error("clipboard ownership generation is exhausted")]
    GenerationExhausted,
}

pub(crate) struct TauriClipboardPort {
    app: AppHandle,
}

impl TauriClipboardPort {
    pub(crate) fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl ClipboardPort for TauriClipboardPort {
    fn write_text(&self, value: &str) -> Result<(), ClipboardError> {
        self.app
            .clipboard()
            .write_text(value)
            .map_err(|_| ClipboardError::WriteFailed)
    }

    fn read_text(&self) -> Result<String, ClipboardError> {
        self.app
            .clipboard()
            .read_text()
            .map_err(|_| ClipboardError::ReadFailed)
    }

    fn clear(&self) -> Result<(), ClipboardError> {
        self.app
            .clipboard()
            .clear()
            .map_err(|_| ClipboardError::ClearFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Failure {
        Write,
        Read,
        Clear,
    }

    #[derive(Default)]
    struct FakeClipboard {
        text: Mutex<String>,
        failure: Mutex<Option<Failure>>,
        callback: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
        clears: Mutex<usize>,
    }

    impl FakeClipboard {
        fn text(&self) -> String {
            self.text.lock().unwrap().clone()
        }

        fn set_text(&self, value: &str) {
            *self.text.lock().unwrap() = value.to_owned();
        }

        fn fail_with(&self, failure: Failure) {
            *self.failure.lock().unwrap() = Some(failure);
        }

        fn clear_count(&self) -> usize {
            *self.clears.lock().unwrap()
        }

        fn set_callback(&self, callback: impl Fn() + Send + Sync + 'static) {
            *self.callback.lock().unwrap() = Some(Arc::new(callback));
        }

        fn invoke_callback(&self) {
            let callback = self.callback.lock().unwrap().clone();
            if let Some(callback) = callback {
                callback();
            }
        }
    }

    impl ClipboardPort for FakeClipboard {
        fn write_text(&self, value: &str) -> Result<(), ClipboardError> {
            self.invoke_callback();
            if *self.failure.lock().unwrap() == Some(Failure::Write) {
                return Err(ClipboardError::WriteFailed);
            }
            self.set_text(value);
            Ok(())
        }

        fn read_text(&self) -> Result<String, ClipboardError> {
            self.invoke_callback();
            if *self.failure.lock().unwrap() == Some(Failure::Read) {
                return Err(ClipboardError::ReadFailed);
            }
            Ok(self.text())
        }

        fn clear(&self) -> Result<(), ClipboardError> {
            self.invoke_callback();
            if *self.failure.lock().unwrap() == Some(Failure::Clear) {
                return Err(ClipboardError::ClearFailed);
            }
            *self.clears.lock().unwrap() += 1;
            self.set_text("");
            Ok(())
        }
    }

    fn service(port: Arc<FakeClipboard>) -> ClipboardService {
        ClipboardService::new(port, Duration::from_secs(30))
    }

    #[test]
    fn matching_owned_content_clears_on_expiration() {
        let port = Arc::new(FakeClipboard::default());
        let service = service(port.clone());
        let generation = service.copy_secret_for_test("secret-one").unwrap();

        service.expire_generation_for_test(generation).unwrap();

        assert_eq!(port.text(), "");
        assert_eq!(port.clear_count(), 1);
    }

    #[test]
    fn user_replaced_content_is_preserved_on_expiration() {
        let port = Arc::new(FakeClipboard::default());
        let service = service(port.clone());
        let generation = service.copy_secret_for_test("secret-one").unwrap();
        port.set_text("newer user value");

        service.expire_generation_for_test(generation).unwrap();

        assert_eq!(port.text(), "newer user value");
        assert_eq!(port.clear_count(), 0);
        assert!(!service.has_owned_value_for_test());
    }

    #[test]
    fn stale_generation_cannot_clear_a_newer_copy() {
        let port = Arc::new(FakeClipboard::default());
        let service = service(port.clone());
        let first = service.copy_secret_for_test("first secret").unwrap();
        let second = service.copy_secret_for_test("second secret").unwrap();

        service.expire_generation_for_test(first).unwrap();
        assert_eq!(port.text(), "second secret");
        assert!(service.has_owned_value_for_test());

        service.expire_generation_for_test(second).unwrap();
        assert_eq!(port.text(), "");
    }

    #[test]
    fn immediate_clear_clears_matching_content() {
        let port = Arc::new(FakeClipboard::default());
        let service = service(port.clone());
        service.copy_secret_for_test("secret").unwrap();

        service.clear_if_owned().unwrap();

        assert_eq!(port.text(), "");
        assert!(!service.has_owned_value_for_test());
    }

    #[test]
    fn immediate_clear_preserves_changed_user_content_and_is_idempotent() {
        let port = Arc::new(FakeClipboard::default());
        let service = service(port.clone());
        service.copy_secret_for_test("secret").unwrap();
        port.set_text("user content");

        service.clear_if_owned().unwrap();
        service.clear_if_owned().unwrap();

        assert_eq!(port.text(), "user content");
        assert_eq!(port.clear_count(), 0);
    }

    #[test]
    fn process_exit_best_effort_path_uses_ownership_safe_clearing() {
        let port = Arc::new(FakeClipboard::default());
        let service = service(port.clone());
        service.copy_secret_for_test("secret").unwrap();

        service.clear_on_process_exit_best_effort();
        assert_eq!(port.text(), "");

        service.copy_secret_for_test("another secret").unwrap();
        port.set_text("user content");
        service.clear_on_process_exit_best_effort();
        assert_eq!(port.text(), "user content");
    }

    #[test]
    fn timeout_replacement_is_captured_only_by_subsequent_copies() {
        let port = Arc::new(FakeClipboard::default());
        let service = service(port);
        let first = service.copy_plan_for_test("first").unwrap();

        service.set_timeout(Duration::from_secs(60)).unwrap();
        let second = service.copy_plan_for_test("second").unwrap();

        assert_eq!(first.timeout, Duration::from_secs(30));
        assert_eq!(second.timeout, Duration::from_secs(60));
        assert!(second.generation > first.generation);
        assert!(matches!(
            service.set_timeout(Duration::from_secs(31)),
            Err(ClipboardError::InvalidTimeout)
        ));
    }

    #[test]
    fn failed_write_does_not_claim_ownership() {
        let port = Arc::new(FakeClipboard::default());
        port.fail_with(Failure::Write);
        let service = service(port);

        assert!(matches!(
            service.copy_secret_for_test("secret"),
            Err(ClipboardError::WriteFailed)
        ));
        assert!(!service.has_owned_value_for_test());
    }

    #[test]
    fn failed_read_never_blindly_clears_and_discards_ownership() {
        let port = Arc::new(FakeClipboard::default());
        let service = service(port.clone());
        let generation = service.copy_secret_for_test("secret").unwrap();
        port.fail_with(Failure::Read);

        assert!(matches!(
            service.expire_generation_for_test(generation),
            Err(ClipboardError::ReadFailed)
        ));
        assert_eq!(port.clear_count(), 0);
        assert!(!service.has_owned_value_for_test());
    }

    #[test]
    fn failed_clear_returns_safe_error_and_discards_ownership() {
        let port = Arc::new(FakeClipboard::default());
        let service = service(port.clone());
        service.copy_secret_for_test("secret").unwrap();
        port.fail_with(Failure::Clear);

        assert!(matches!(
            service.clear_if_owned(),
            Err(ClipboardError::ClearFailed)
        ));
        assert!(!service.has_owned_value_for_test());
    }

    #[test]
    fn displayed_and_debug_errors_never_contain_secret_or_clipboard_content() {
        let secret = "secret-value-that-must-not-escape";
        let port = Arc::new(FakeClipboard::default());
        port.fail_with(Failure::Write);
        let service = service(port);

        let error = service.copy_secret_for_test(secret).unwrap_err();
        assert!(!error.to_string().contains(secret));
        assert!(!format!("{error:?}").contains(secret));
    }

    #[test]
    fn generation_exhaustion_safely_discards_the_unowned_write() {
        let port = Arc::new(FakeClipboard::default());
        let service = service(port.clone());
        service.copy_secret_for_test("prior secret").unwrap();
        service.lock_state().next_generation = u64::MAX;

        assert!(matches!(
            service.copy_secret_for_test("final secret"),
            Err(ClipboardError::GenerationExhausted)
        ));
        assert_eq!(port.text(), "");
        assert!(!service.has_owned_value_for_test());
    }

    #[test]
    fn clipboard_port_callbacks_never_run_while_service_mutex_is_locked() {
        let port = Arc::new(FakeClipboard::default());
        let service = service(port.clone());
        let inner = service.inner.clone();
        port.set_callback(move || {
            assert!(
                inner.try_lock().is_ok(),
                "clipboard I/O ran under service mutex"
            );
        });

        service.copy_secret_for_test("secret").unwrap();
        service.clear_if_owned().unwrap();
    }
}
