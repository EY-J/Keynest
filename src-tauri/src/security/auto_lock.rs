use std::{
    sync::{Arc, Condvar, Mutex, MutexGuard},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use super::{AuthStatus, LockError};

pub(crate) trait LockActions: Send + Sync {
    fn status(&self) -> AuthStatus;
    fn lock(&self) -> Result<AuthStatus, LockError>;
}

#[derive(Clone)]
pub(crate) struct AutoLockService {
    supervisor: Arc<Supervisor>,
}

struct Supervisor {
    shared: Arc<(Mutex<AutoLockState>, Condvar)>,
    actions: Arc<dyn LockActions>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

struct AutoLockState {
    armed: bool,
    last_activity: Instant,
    timeout: Duration,
    shutdown: bool,
}

impl AutoLockService {
    pub(crate) fn new(actions: Arc<dyn LockActions>, timeout: Duration) -> Self {
        Self::new_inner(actions, timeout, true)
    }

    fn new_inner(actions: Arc<dyn LockActions>, timeout: Duration, start_worker: bool) -> Self {
        let shared = Arc::new((
            Mutex::new(AutoLockState {
                armed: false,
                last_activity: Instant::now(),
                timeout,
                shutdown: false,
            }),
            Condvar::new(),
        ));
        let worker = if start_worker {
            let worker_shared = shared.clone();
            let worker_actions = actions.clone();
            Some(
                thread::Builder::new()
                    .name("keynest-auto-lock".to_owned())
                    .spawn(move || supervise(worker_shared, worker_actions))
                    .expect("failed to start KeyNest auto-lock supervisor"),
            )
        } else {
            None
        };
        Self {
            supervisor: Arc::new(Supervisor {
                shared,
                actions,
                worker: Mutex::new(worker),
            }),
        }
    }

    pub(crate) fn arm(&self) {
        self.arm_at(Instant::now());
    }

    pub(crate) fn disarm(&self) {
        let (state, wake) = &*self.supervisor.shared;
        state.lock_unpoisoned().armed = false;
        wake.notify_all();
    }

    pub(crate) fn record_activity(&self) {
        self.record_activity_at(Instant::now());
    }

    pub(crate) fn set_timeout(&self, timeout: Duration) -> Result<(), LockError> {
        self.set_timeout_at(timeout, Instant::now())
    }

    pub(crate) fn lock_now(&self) -> Result<AuthStatus, LockError> {
        let (state, wake) = &*self.supervisor.shared;
        let mut state = state.lock_unpoisoned();
        state.armed = false;
        wake.notify_all();
        self.supervisor.actions.lock()
    }

    fn arm_at(&self, now: Instant) {
        let (state, wake) = &*self.supervisor.shared;
        let mut state = state.lock_unpoisoned();
        state.last_activity = now;
        state.armed = self.supervisor.actions.status() == AuthStatus::Unlocked;
        wake.notify_all();
    }

    fn record_activity_at(&self, now: Instant) {
        let (state, wake) = &*self.supervisor.shared;
        let mut state = state.lock_unpoisoned();
        if state.armed {
            state.last_activity = now;
            wake.notify_all();
        }
    }

    fn set_timeout_at(&self, timeout: Duration, now: Instant) -> Result<(), LockError> {
        let (state, wake) = &*self.supervisor.shared;
        let mut state = state.lock_unpoisoned();
        state.timeout = timeout;
        let should_lock = state.armed && deadline_reached(&state, now);
        if should_lock {
            state.armed = false;
            self.supervisor.actions.lock()?;
        }
        wake.notify_all();
        Ok(())
    }

    #[cfg(test)]
    fn new_for_test(actions: Arc<dyn LockActions>, timeout: Duration) -> Self {
        Self::new_inner(actions, timeout, false)
    }

    #[cfg(test)]
    fn new_with_worker_for_test(actions: Arc<dyn LockActions>, timeout: Duration) -> Self {
        Self::new_inner(actions, timeout, true)
    }

    #[cfg(test)]
    fn arm_at_for_test(&self, now: Instant) {
        self.arm_at(now);
    }

    #[cfg(test)]
    fn record_activity_at_for_test(&self, now: Instant) {
        self.record_activity_at(now);
    }

    #[cfg(test)]
    fn set_timeout_at_for_test(&self, timeout: Duration, now: Instant) -> Result<(), LockError> {
        self.set_timeout_at(timeout, now)
    }

    #[cfg(test)]
    fn expire_at_for_test(&self, now: Instant) -> bool {
        let (state, _) = &*self.supervisor.shared;
        let mut state = state.lock_unpoisoned();
        let should_lock = state.armed && deadline_reached(&state, now);
        if should_lock {
            state.armed = false;
            let _ = self.supervisor.actions.lock();
        }
        should_lock
    }

    #[cfg(test)]
    fn is_armed_for_test(&self) -> bool {
        self.supervisor.shared.0.lock_unpoisoned().armed
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        let (state, wake) = &*self.shared;
        state.lock_unpoisoned().shutdown = true;
        wake.notify_all();
        if let Some(worker) = self.worker.lock_unpoisoned().take() {
            if worker.thread().id() != thread::current().id() {
                let _ = worker.join();
            }
        }
    }
}

fn supervise(shared: Arc<(Mutex<AutoLockState>, Condvar)>, actions: Arc<dyn LockActions>) {
    let (state_lock, wake) = &*shared;
    let mut state = state_lock.lock_unpoisoned();
    loop {
        if state.shutdown {
            return;
        }
        if !state.armed {
            state = wake
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            continue;
        }

        let now = Instant::now();
        if deadline_reached(&state, now) {
            state.armed = false;
            let _ = actions.lock();
            continue;
        }

        let remaining = state
            .last_activity
            .checked_add(state.timeout)
            .and_then(|deadline| deadline.checked_duration_since(now))
            .unwrap_or(Duration::ZERO);
        let (next_state, _) = wake
            .wait_timeout(state, remaining)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state = next_state;
    }
}

fn deadline_reached(state: &AutoLockState, now: Instant) -> bool {
    state
        .last_activity
        .checked_add(state.timeout)
        .is_some_and(|deadline| now >= deadline)
}

trait MutexExt<T> {
    fn lock_unpoisoned(&self) -> MutexGuard<'_, T>;
}

impl<T> MutexExt<T> for Mutex<T> {
    fn lock_unpoisoned(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        },
        time::{Duration, Instant},
    };

    use super::{AutoLockService, LockActions};
    use crate::security::{AuthStatus, LockError};

    #[derive(Default)]
    struct FakeLockActions {
        calls: AtomicUsize,
        fail: AtomicBool,
        locked: AtomicBool,
    }

    impl FakeLockActions {
        fn lock_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl LockActions for FakeLockActions {
        fn status(&self) -> AuthStatus {
            if self.locked.load(Ordering::SeqCst) {
                AuthStatus::Locked
            } else {
                AuthStatus::Unlocked
            }
        }

        fn lock(&self) -> Result<AuthStatus, LockError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.locked.store(true, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) {
                Err(LockError::EventEmissionFailed)
            } else {
                Ok(AuthStatus::Locked)
            }
        }
    }

    #[test]
    fn deadline_expiry_locks_once_at_the_exact_boundary() {
        let actions = Arc::new(FakeLockActions::default());
        let service = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(300));
        let start = Instant::now();
        service.arm_at_for_test(start);

        assert!(!service.expire_at_for_test(start + Duration::from_secs(299)));
        assert!(service.expire_at_for_test(start + Duration::from_secs(300)));
        assert!(!service.expire_at_for_test(start + Duration::from_secs(301)));
        assert_eq!(actions.lock_count(), 1);
    }

    #[test]
    fn disarmed_state_never_locks() {
        let actions = Arc::new(FakeLockActions::default());
        let service = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(300));

        assert!(!service.expire_at_for_test(Instant::now() + Duration::from_secs(900)));
        assert_eq!(actions.lock_count(), 0);
    }

    #[test]
    fn locked_auth_state_cannot_be_armed() {
        let actions = Arc::new(FakeLockActions::default());
        actions.locked.store(true, Ordering::SeqCst);
        let service = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(300));
        let start = Instant::now();

        service.arm_at_for_test(start);

        assert!(!service.is_armed_for_test());
        assert!(!service.expire_at_for_test(start + Duration::from_secs(300)));
        assert_eq!(actions.lock_count(), 0);
    }

    #[test]
    fn activity_moves_the_deadline_only_while_armed() {
        let actions = Arc::new(FakeLockActions::default());
        let service = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(300));
        let start = Instant::now();
        service.record_activity_at_for_test(start + Duration::from_secs(200));
        service.arm_at_for_test(start);
        service.record_activity_at_for_test(start + Duration::from_secs(100));

        assert!(!service.expire_at_for_test(start + Duration::from_secs(399)));
        assert!(service.expire_at_for_test(start + Duration::from_secs(400)));
        assert_eq!(actions.lock_count(), 1);
    }

    #[test]
    fn shortening_timeout_can_lock_immediately() {
        let actions = Arc::new(FakeLockActions::default());
        let service = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(300));
        let start = Instant::now();
        service.arm_at_for_test(start);
        service.record_activity_at_for_test(start + Duration::from_secs(100));

        service
            .set_timeout_at_for_test(Duration::from_secs(60), start + Duration::from_secs(170))
            .unwrap();

        assert_eq!(actions.lock_count(), 1);
        assert!(!service.is_armed_for_test());
    }

    #[test]
    fn extending_timeout_does_not_lock_early() {
        let actions = Arc::new(FakeLockActions::default());
        let service = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(60));
        let start = Instant::now();
        service.arm_at_for_test(start);

        service
            .set_timeout_at_for_test(Duration::from_secs(300), start + Duration::from_secs(59))
            .unwrap();

        assert!(!service.expire_at_for_test(start + Duration::from_secs(299)));
        assert!(service.expire_at_for_test(start + Duration::from_secs(300)));
        assert_eq!(actions.lock_count(), 1);
    }

    #[test]
    fn manual_lock_disarms_before_invoking_actions() {
        let actions = Arc::new(FakeLockActions::default());
        let service = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(300));
        service.arm_at_for_test(Instant::now());

        assert_eq!(service.lock_now().unwrap(), AuthStatus::Locked);

        assert!(!service.is_armed_for_test());
        assert_eq!(actions.lock_count(), 1);
    }

    #[test]
    fn lock_action_failure_leaves_service_disarmed_without_retry() {
        let actions = Arc::new(FakeLockActions::default());
        actions.fail.store(true, Ordering::SeqCst);
        let service = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(300));
        let start = Instant::now();
        service.arm_at_for_test(start);

        assert!(service.expire_at_for_test(start + Duration::from_secs(300)));
        assert!(!service.expire_at_for_test(start + Duration::from_secs(301)));
        assert!(!service.is_armed_for_test());
        assert_eq!(actions.lock_count(), 1);
    }

    #[test]
    fn final_drop_wakes_and_joins_a_waiting_worker_without_waiting_for_deadline() {
        let actions = Arc::new(FakeLockActions::default());
        let service =
            AutoLockService::new_with_worker_for_test(actions, Duration::from_secs(60 * 60));
        service.arm();
        let before = Instant::now();

        drop(service);

        assert!(before.elapsed() < Duration::from_secs(1));
    }
}
