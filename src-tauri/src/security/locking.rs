use std::sync::Arc;

use tauri::{AppHandle, Emitter};
use thiserror::Error;

use super::{AuthService, AuthStatus, ClipboardService};

const LOCKED_EVENT: &str = "keynest://locked";

pub(crate) trait LockEventSink: Send + Sync {
    fn emit_locked(&self) -> Result<(), LockError>;
}

trait ClipboardCleanup: Send + Sync {
    fn clear_if_owned(&self) -> Result<(), LockError>;
}

impl ClipboardCleanup for ClipboardService {
    fn clear_if_owned(&self) -> Result<(), LockError> {
        ClipboardService::clear_if_owned(self).map_err(|_| LockError::ClipboardCleanupFailed)
    }
}

#[derive(Clone)]
pub(crate) struct LockCoordinator {
    auth: AuthService,
    clipboard: ClipboardService,
    events: Arc<dyn LockEventSink>,
    #[cfg(test)]
    clipboard_override: Option<Arc<dyn ClipboardCleanup>>,
}

impl LockCoordinator {
    pub(crate) fn new(
        auth: AuthService,
        clipboard: ClipboardService,
        events: Arc<dyn LockEventSink>,
    ) -> Self {
        Self {
            auth,
            clipboard,
            events,
            #[cfg(test)]
            clipboard_override: None,
        }
    }

    pub(crate) fn lock_and_emit(&self) -> Result<AuthStatus, LockError> {
        let outcome = self.auth.lock();
        let clipboard_result = self.clear_clipboard();
        let event_result = if outcome.transitioned {
            self.events.emit_locked()
        } else {
            Ok(())
        };

        event_result?;
        clipboard_result?;
        Ok(outcome.status)
    }

    fn clear_clipboard(&self) -> Result<(), LockError> {
        #[cfg(test)]
        if let Some(cleanup) = &self.clipboard_override {
            return cleanup.clear_if_owned();
        }
        ClipboardCleanup::clear_if_owned(&self.clipboard)
    }

    #[cfg(test)]
    fn with_clipboard_override(mut self, cleanup: Arc<dyn ClipboardCleanup>) -> Self {
        self.clipboard_override = Some(cleanup);
        self
    }
}

#[derive(Clone)]
pub(crate) struct TauriLockEventSink {
    app: AppHandle,
}

impl TauriLockEventSink {
    pub(crate) fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl LockEventSink for TauriLockEventSink {
    fn emit_locked(&self) -> Result<(), LockError> {
        self.app
            .emit(LOCKED_EVENT, ())
            .map_err(|_| LockError::EventEmissionFailed)
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub(crate) enum LockError {
    #[error("clipboard cleanup failed")]
    ClipboardCleanupFailed,
    #[error("lock event emission failed")]
    EventEmissionFailed,
}

impl super::auto_lock::LockActions for LockCoordinator {
    fn status(&self) -> AuthStatus {
        self.auth.status()
    }

    fn lock(&self) -> Result<AuthStatus, LockError> {
        self.lock_and_emit()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc, Barrier, Mutex,
        },
        thread,
        time::Duration,
    };

    use tempfile::tempdir;

    use super::{ClipboardCleanup, LockCoordinator, LockError, LockEventSink};
    use crate::security::{
        clipboard::{ClipboardError, ClipboardPort},
        crypto::{CryptoError, EntropySource},
        AuthService, AuthStatus, ClipboardService, KdfParams, ProfileStore,
    };

    #[derive(Default)]
    struct NoopClipboardPort;

    impl ClipboardPort for NoopClipboardPort {
        fn write_text(&self, _value: &str) -> Result<(), ClipboardError> {
            Ok(())
        }

        fn read_text(&self) -> Result<String, ClipboardError> {
            Ok(String::new())
        }

        fn clear(&self) -> Result<(), ClipboardError> {
            Ok(())
        }
    }

    struct FixedEntropy;

    impl EntropySource for FixedEntropy {
        fn fill(&self, destination: &mut [u8]) -> Result<(), CryptoError> {
            for (index, byte) in destination.iter_mut().enumerate() {
                *byte = index as u8;
            }
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecordingCleanup {
        calls: AtomicUsize,
        fail: AtomicBool,
        operations: Arc<Mutex<Vec<&'static str>>>,
    }

    impl ClipboardCleanup for RecordingCleanup {
        fn clear_if_owned(&self) -> Result<(), LockError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.operations.lock().unwrap().push("clipboard");
            if self.fail.load(Ordering::SeqCst) {
                Err(LockError::ClipboardCleanupFailed)
            } else {
                Ok(())
            }
        }
    }

    #[derive(Default)]
    struct RecordingLockEventSink {
        calls: AtomicUsize,
        fail: AtomicBool,
        operations: Arc<Mutex<Vec<&'static str>>>,
    }

    impl LockEventSink for RecordingLockEventSink {
        fn emit_locked(&self) -> Result<(), LockError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.operations.lock().unwrap().push("event");
            if self.fail.load(Ordering::SeqCst) {
                Err(LockError::EventEmissionFailed)
            } else {
                Ok(())
            }
        }
    }

    fn fixture(
        unlocked: bool,
    ) -> (
        LockCoordinator,
        AuthService,
        Arc<RecordingCleanup>,
        Arc<RecordingLockEventSink>,
    ) {
        let directory = tempdir().unwrap();
        let params = KdfParams::testing();
        let auth = AuthService::load(
            ProfileStore::new(directory.path().to_path_buf(), params),
            params,
            Arc::new(FixedEntropy),
        );
        if unlocked {
            auth.create_master_password("a secure master password")
                .unwrap();
        }
        let port = Arc::new(NoopClipboardPort);
        let clipboard = ClipboardService::new(port, Duration::from_secs(30));
        let operations = Arc::new(Mutex::new(Vec::new()));
        let cleanup = Arc::new(RecordingCleanup {
            operations: operations.clone(),
            ..Default::default()
        });
        let events = Arc::new(RecordingLockEventSink {
            operations,
            ..Default::default()
        });
        let coordinator = LockCoordinator::new(auth.clone(), clipboard, events.clone())
            .with_clipboard_override(cleanup.clone());
        (coordinator, auth, cleanup, events)
    }

    #[test]
    fn repeated_lock_triggers_emit_only_for_the_transition_but_always_clean_up() {
        let (coordinator, auth, cleanup, events) = fixture(true);

        assert_eq!(coordinator.lock_and_emit().unwrap(), AuthStatus::Locked);
        assert_eq!(coordinator.lock_and_emit().unwrap(), AuthStatus::Locked);

        assert_eq!(auth.status(), AuthStatus::Locked);
        assert_eq!(cleanup.calls.load(Ordering::SeqCst), 2);
        assert_eq!(events.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn lock_event_is_emitted_after_clipboard_cleanup_is_requested() {
        let (coordinator, _, cleanup, events) = fixture(true);

        coordinator.lock_and_emit().unwrap();

        assert_eq!(
            cleanup.operations.lock().unwrap().as_slice(),
            ["clipboard", "event"]
        );
        assert_eq!(events.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn setup_state_does_not_emit_but_still_requests_cleanup() {
        let (coordinator, _, cleanup, events) = fixture(false);

        assert_eq!(
            coordinator.lock_and_emit().unwrap(),
            AuthStatus::SetupRequired
        );
        assert_eq!(cleanup.calls.load(Ordering::SeqCst), 1);
        assert_eq!(events.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn cleanup_failure_still_emits_and_keeps_auth_locked() {
        let (coordinator, auth, cleanup, events) = fixture(true);
        cleanup.fail.store(true, Ordering::SeqCst);

        assert_eq!(
            coordinator.lock_and_emit(),
            Err(LockError::ClipboardCleanupFailed)
        );

        assert_eq!(auth.status(), AuthStatus::Locked);
        assert_eq!(events.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            cleanup.operations.lock().unwrap().as_slice(),
            ["clipboard", "event"]
        );
    }

    #[test]
    fn event_failure_happens_after_cleanup_and_keeps_auth_locked() {
        let (coordinator, auth, cleanup, events) = fixture(true);
        events.fail.store(true, Ordering::SeqCst);

        assert_eq!(
            coordinator.lock_and_emit(),
            Err(LockError::EventEmissionFailed)
        );

        assert_eq!(auth.status(), AuthStatus::Locked);
        assert_eq!(
            cleanup.operations.lock().unwrap().as_slice(),
            ["clipboard", "event"]
        );
    }

    #[test]
    fn simultaneous_triggers_emit_at_most_once_and_never_restore_unlocked_state() {
        let (coordinator, auth, cleanup, events) = fixture(true);
        let coordinator = Arc::new(coordinator);
        let barrier = Arc::new(Barrier::new(9));
        let mut workers = Vec::new();
        for _ in 0..8 {
            let coordinator = coordinator.clone();
            let barrier = barrier.clone();
            workers.push(thread::spawn(move || {
                barrier.wait();
                coordinator.lock_and_emit().unwrap();
            }));
        }
        barrier.wait();
        for worker in workers {
            worker.join().unwrap();
        }

        assert_eq!(auth.status(), AuthStatus::Locked);
        assert_eq!(cleanup.calls.load(Ordering::SeqCst), 8);
        assert_eq!(events.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn errors_are_secret_safe() {
        assert_eq!(
            LockError::ClipboardCleanupFailed.to_string(),
            "clipboard cleanup failed"
        );
        assert_eq!(
            LockError::EventEmissionFailed.to_string(),
            "lock event emission failed"
        );
    }
}
