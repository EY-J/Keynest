use std::{
    sync::{Arc, Condvar, Mutex, MutexGuard},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use super::{AuthStatus, LockError, SecurityOperationGuard};

pub(crate) trait LockActions: Send + Sync {
    fn status(&self) -> AuthStatus;
    fn lock(&self) -> Result<AuthStatus, LockError>;

    fn lock_with_operation_guard(
        &self,
        _guard: &SecurityOperationGuard<'_>,
    ) -> Result<AuthStatus, LockError> {
        self.lock()
    }
}

trait MonotonicClock: Send + Sync {
    fn now(&self) -> Instant;
}

struct SystemClock;

impl MonotonicClock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

#[derive(Clone)]
pub(crate) struct AutoLockService {
    supervisor: Arc<Supervisor>,
}

struct Supervisor {
    shared: Arc<(Mutex<AutoLockState>, Condvar)>,
    actions: Arc<dyn LockActions>,
    clock: Arc<dyn MonotonicClock>,
    worker: Mutex<Option<JoinHandle<()>>>,
    lifecycle: Arc<(Mutex<WorkerLifecycle>, Condvar)>,
}

struct AutoLockState {
    armed: bool,
    last_activity: Instant,
    timeout: Duration,
    shutdown: bool,
    epoch: u64,
    locks_in_flight: usize,
    pending_arm: Option<ArmRequest>,
}

#[derive(Clone, Copy)]
struct ArmRequest {
    at: Instant,
    epoch: u64,
}

struct WorkerLifecycle {
    shutdown_requested: bool,
    worker_terminated: bool,
    worker_joined: bool,
}

impl AutoLockService {
    pub(crate) fn new(actions: Arc<dyn LockActions>, timeout: Duration) -> Self {
        Self::new_inner(actions, timeout, Arc::new(SystemClock), true)
    }

    fn new_inner(
        actions: Arc<dyn LockActions>,
        timeout: Duration,
        clock: Arc<dyn MonotonicClock>,
        start_worker: bool,
    ) -> Self {
        let shared = Arc::new((
            Mutex::new(AutoLockState {
                armed: false,
                last_activity: clock.now(),
                timeout,
                shutdown: false,
                epoch: 0,
                locks_in_flight: 0,
                pending_arm: None,
            }),
            Condvar::new(),
        ));
        let lifecycle = Arc::new((
            Mutex::new(WorkerLifecycle {
                shutdown_requested: false,
                worker_terminated: !start_worker,
                worker_joined: !start_worker,
            }),
            Condvar::new(),
        ));
        let worker = if start_worker {
            let worker_shared = shared.clone();
            let worker_actions = actions.clone();
            let worker_clock = clock.clone();
            let worker_lifecycle = lifecycle.clone();
            Some(
                thread::Builder::new()
                    .name("keynest-auto-lock".to_owned())
                    .spawn(move || {
                        supervise(worker_shared, worker_actions, worker_clock);
                        mark_worker_terminated(&worker_lifecycle);
                    })
                    .expect("failed to start KeyNest auto-lock supervisor"),
            )
        } else {
            None
        };
        Self {
            supervisor: Arc::new(Supervisor {
                shared,
                actions,
                clock,
                worker: Mutex::new(worker),
                lifecycle,
            }),
        }
    }

    pub(crate) fn arm(&self) {
        self.arm_at(self.supervisor.clock.now());
    }

    pub(crate) fn disarm(&self) {
        let (state, wake) = &*self.supervisor.shared;
        let mut state = state.lock_unpoisoned();
        state.epoch = state.epoch.wrapping_add(1);
        state.armed = false;
        state.pending_arm = None;
        wake.notify_all();
    }

    pub(crate) fn record_activity(&self) {
        self.record_activity_at(self.supervisor.clock.now());
    }

    pub(crate) fn set_timeout(&self, timeout: Duration) -> Result<(), LockError> {
        self.set_timeout_at(timeout, self.supervisor.clock.now(), None)
    }

    pub(crate) fn set_timeout_with_operation_guard(
        &self,
        timeout: Duration,
        guard: &SecurityOperationGuard<'_>,
    ) -> Result<(), LockError> {
        self.set_timeout_at(timeout, self.supervisor.clock.now(), Some(guard))
    }

    pub(crate) fn lock_now(&self) -> Result<AuthStatus, LockError> {
        self.lock_now_with_optional_guard(None)
    }

    pub(crate) fn lock_now_with_operation_guard(
        &self,
        guard: &SecurityOperationGuard<'_>,
    ) -> Result<AuthStatus, LockError> {
        self.lock_now_with_optional_guard(Some(guard))
    }

    fn lock_now_with_optional_guard(
        &self,
        guard: Option<&SecurityOperationGuard<'_>>,
    ) -> Result<AuthStatus, LockError> {
        self.begin_explicit_lock();
        let result = match guard {
            Some(guard) => self.supervisor.actions.lock_with_operation_guard(guard),
            None => self.supervisor.actions.lock(),
        };
        self.reconcile_after_lock();
        result
    }

    /// Requests shutdown immediately and moves the worker handle exactly once to a dedicated
    /// joiner. It never waits for lock/clipboard/event I/O on the caller (Tauri event-loop) thread.
    pub(crate) fn shutdown(&self) {
        request_shutdown(&self.supervisor.shared, &self.supervisor.lifecycle);
        if let Some(worker) = self.supervisor.worker.lock_unpoisoned().take() {
            let lifecycle = self.supervisor.lifecycle.clone();
            thread::Builder::new()
                .name("keynest-auto-lock-join".to_owned())
                .spawn(move || {
                    let _ = worker.join();
                    mark_worker_joined(&lifecycle);
                })
                .expect("failed to start KeyNest auto-lock joiner");
        }
    }

    fn arm_at(&self, now: Instant) {
        let request = {
            let (state, wake) = &*self.supervisor.shared;
            let mut state = state.lock_unpoisoned();
            if state.shutdown {
                return;
            }
            state.epoch = state.epoch.wrapping_add(1);
            let request = ArmRequest {
                at: now,
                epoch: state.epoch,
            };
            state.armed = false;
            if state.locks_in_flight > 0 {
                state.pending_arm = Some(request);
                wake.notify_all();
                return;
            }
            request
        };

        let unlocked = self.supervisor.actions.status() == AuthStatus::Unlocked;
        self.finish_arm(request, unlocked);
    }

    fn finish_arm(&self, request: ArmRequest, unlocked: bool) {
        let (state, wake) = &*self.supervisor.shared;
        let mut state = state.lock_unpoisoned();
        if state.shutdown || state.epoch != request.epoch {
            return;
        }
        if state.locks_in_flight > 0 {
            state.pending_arm = Some(request);
        } else if unlocked {
            state.last_activity = request.at;
            state.armed = true;
            state.pending_arm = None;
        } else {
            state.armed = false;
            state.pending_arm = None;
        }
        wake.notify_all();
    }

    fn record_activity_at(&self, now: Instant) {
        let (state, wake) = &*self.supervisor.shared;
        let mut state = state.lock_unpoisoned();
        if state.armed && !state.shutdown {
            state.last_activity = now;
            wake.notify_all();
        }
    }

    fn set_timeout_at(
        &self,
        timeout: Duration,
        now: Instant,
        guard: Option<&SecurityOperationGuard<'_>>,
    ) -> Result<(), LockError> {
        let should_lock = {
            let (state, wake) = &*self.supervisor.shared;
            let mut state = state.lock_unpoisoned();
            state.timeout = timeout;
            let should_lock = state.armed && deadline_reached(&state, now);
            if should_lock {
                begin_lock(&mut state, false);
            }
            wake.notify_all();
            should_lock
        };
        if !should_lock {
            return Ok(());
        }
        let result = match guard {
            Some(guard) => self.supervisor.actions.lock_with_operation_guard(guard),
            None => self.supervisor.actions.lock(),
        };
        self.reconcile_after_lock();
        result.map(|_| ())
    }

    fn begin_explicit_lock(&self) {
        let (state, wake) = &*self.supervisor.shared;
        let mut state = state.lock_unpoisoned();
        begin_lock(&mut state, true);
        wake.notify_all();
    }

    fn reconcile_after_lock(&self) {
        reconcile_after_lock(&self.supervisor);
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(actions: Arc<dyn LockActions>, timeout: Duration) -> Self {
        Self::new_inner(actions, timeout, Arc::new(SystemClock), false)
    }

    #[cfg(test)]
    fn new_with_worker_for_test(actions: Arc<dyn LockActions>, timeout: Duration) -> Self {
        Self::new_inner(actions, timeout, Arc::new(SystemClock), true)
    }

    #[cfg(test)]
    fn new_with_clock_for_test(
        actions: Arc<dyn LockActions>,
        timeout: Duration,
        clock: Arc<dyn MonotonicClock>,
    ) -> Self {
        Self::new_inner(actions, timeout, clock, true)
    }

    #[cfg(test)]
    pub(crate) fn arm_at_for_test(&self, now: Instant) {
        self.arm_at(now);
    }

    #[cfg(test)]
    pub(crate) fn record_activity_at_for_test(&self, now: Instant) {
        self.record_activity_at(now);
    }

    #[cfg(test)]
    fn set_timeout_at_for_test(&self, timeout: Duration, now: Instant) -> Result<(), LockError> {
        self.set_timeout_at(timeout, now, None)
    }

    #[cfg(test)]
    pub(crate) fn expire_at_for_test(&self, now: Instant) -> bool {
        let should_lock = {
            let (state, _) = &*self.supervisor.shared;
            let mut state = state.lock_unpoisoned();
            let should_lock = state.armed && deadline_reached(&state, now);
            if should_lock {
                begin_lock(&mut state, false);
            }
            should_lock
        };
        if should_lock {
            let _ = self.supervisor.actions.lock();
            self.reconcile_after_lock();
        }
        should_lock
    }

    #[cfg(test)]
    pub(crate) fn is_armed_for_test(&self) -> bool {
        self.supervisor.shared.0.lock_unpoisoned().armed
    }

    #[cfg(test)]
    pub(crate) fn timeout_for_test(&self) -> Duration {
        self.supervisor.shared.0.lock_unpoisoned().timeout
    }

    #[cfg(test)]
    fn notify_for_test(&self) {
        self.supervisor.shared.1.notify_all();
    }

    #[cfg(test)]
    fn shutdown_requested_for_test(&self) -> bool {
        self.supervisor
            .lifecycle
            .0
            .lock_unpoisoned()
            .shutdown_requested
    }

    #[cfg(test)]
    fn wait_for_worker_termination_for_test(&self, timeout: Duration) -> bool {
        let (lifecycle, wake) = &*self.supervisor.lifecycle;
        let lifecycle = lifecycle.lock_unpoisoned();
        let (lifecycle, _) = wake
            .wait_timeout_while(lifecycle, timeout, |state| {
                !state.worker_terminated || !state.worker_joined
            })
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        lifecycle.worker_terminated && lifecycle.worker_joined
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        request_shutdown(&self.shared, &self.lifecycle);
        if let Some(worker) = self.worker.lock_unpoisoned().take() {
            if worker.thread().id() != thread::current().id() {
                let _ = worker.join();
                mark_worker_joined(&self.lifecycle);
            }
        }
    }
}

fn begin_lock(state: &mut AutoLockState, clear_pending_arm: bool) {
    state.epoch = state.epoch.wrapping_add(1);
    state.armed = false;
    if clear_pending_arm {
        state.pending_arm = None;
    }
    state.locks_in_flight = state.locks_in_flight.saturating_add(1);
}

fn reconcile_after_lock(supervisor: &Supervisor) {
    let pending = {
        let (state, wake) = &*supervisor.shared;
        let mut state = state.lock_unpoisoned();
        state.locks_in_flight = state.locks_in_flight.saturating_sub(1);
        if state.shutdown || state.locks_in_flight > 0 {
            wake.notify_all();
            return;
        }
        state.pending_arm.take()
    };
    let Some(request) = pending else {
        return;
    };
    let unlocked = supervisor.actions.status() == AuthStatus::Unlocked;
    let (state, wake) = &*supervisor.shared;
    let mut state = state.lock_unpoisoned();
    if state.shutdown || state.epoch != request.epoch {
        return;
    }
    if state.locks_in_flight > 0 {
        state.pending_arm = Some(request);
    } else if unlocked {
        state.last_activity = request.at;
        state.armed = true;
    } else {
        state.armed = false;
    }
    wake.notify_all();
}

fn supervise(
    shared: Arc<(Mutex<AutoLockState>, Condvar)>,
    actions: Arc<dyn LockActions>,
    clock: Arc<dyn MonotonicClock>,
) {
    let supervisor = SupervisorView {
        shared: &shared,
        actions: &actions,
    };
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
        let now = clock.now();
        if deadline_reached(&state, now) {
            begin_lock(&mut state, false);
            drop(state);
            let _ = actions.lock();
            reconcile_after_lock_view(&supervisor);
            state = state_lock.lock_unpoisoned();
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

struct SupervisorView<'a> {
    shared: &'a Arc<(Mutex<AutoLockState>, Condvar)>,
    actions: &'a Arc<dyn LockActions>,
}

fn reconcile_after_lock_view(supervisor: &SupervisorView<'_>) {
    let pending = {
        let (state, wake) = &**supervisor.shared;
        let mut state = state.lock_unpoisoned();
        state.locks_in_flight = state.locks_in_flight.saturating_sub(1);
        if state.shutdown || state.locks_in_flight > 0 {
            wake.notify_all();
            return;
        }
        state.pending_arm.take()
    };
    let Some(request) = pending else {
        return;
    };
    let unlocked = supervisor.actions.status() == AuthStatus::Unlocked;
    let (state, wake) = &**supervisor.shared;
    let mut state = state.lock_unpoisoned();
    if state.shutdown || state.epoch != request.epoch {
        return;
    }
    if state.locks_in_flight > 0 {
        state.pending_arm = Some(request);
    } else if unlocked {
        state.last_activity = request.at;
        state.armed = true;
    }
    wake.notify_all();
}

fn request_shutdown(
    shared: &Arc<(Mutex<AutoLockState>, Condvar)>,
    lifecycle: &Arc<(Mutex<WorkerLifecycle>, Condvar)>,
) {
    let (state, wake) = &**shared;
    state.lock_unpoisoned().shutdown = true;
    wake.notify_all();
    let (lifecycle, lifecycle_wake) = &**lifecycle;
    lifecycle.lock_unpoisoned().shutdown_requested = true;
    lifecycle_wake.notify_all();
}

fn mark_worker_terminated(lifecycle: &Arc<(Mutex<WorkerLifecycle>, Condvar)>) {
    let (state, wake) = &**lifecycle;
    state.lock_unpoisoned().worker_terminated = true;
    wake.notify_all();
}

fn mark_worker_joined(lifecycle: &Arc<(Mutex<WorkerLifecycle>, Condvar)>) {
    let (state, wake) = &**lifecycle;
    state.lock_unpoisoned().worker_joined = true;
    wake.notify_all();
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
            mpsc, Arc, Condvar, Mutex,
        },
        thread,
        time::{Duration, Instant},
    };

    use super::{AutoLockService, LockActions, MonotonicClock, MutexExt};
    use crate::security::{AuthStatus, LockError};

    #[derive(Default)]
    struct FakeLockActions {
        calls: AtomicUsize,
        fail: AtomicBool,
        locked: AtomicBool,
        called: (Mutex<usize>, Condvar),
    }

    impl FakeLockActions {
        fn lock_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }

        fn wait_for_calls(&self, count: usize) -> bool {
            let calls = self.called.0.lock_unpoisoned();
            let (calls, _) = self
                .called
                .1
                .wait_timeout_while(calls, Duration::from_secs(1), |calls| *calls < count)
                .unwrap();
            *calls >= count
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
            *self.called.0.lock_unpoisoned() += 1;
            self.called.1.notify_all();
            self.locked.store(true, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) {
                Err(LockError::EventEmissionFailed)
            } else {
                Ok(AuthStatus::Locked)
            }
        }
    }

    struct ManualClock(Mutex<Instant>);

    impl ManualClock {
        fn new(now: Instant) -> Self {
            Self(Mutex::new(now))
        }

        fn advance(&self, duration: Duration) {
            let mut now = self.0.lock_unpoisoned();
            *now += duration;
        }
    }

    impl MonotonicClock for ManualClock {
        fn now(&self) -> Instant {
            *self.0.lock_unpoisoned()
        }
    }

    struct BlockingLockActions {
        locked: AtomicBool,
        started: (Mutex<bool>, Condvar),
        released: (Mutex<bool>, Condvar),
    }

    impl Default for BlockingLockActions {
        fn default() -> Self {
            Self {
                locked: AtomicBool::new(false),
                started: (Mutex::new(false), Condvar::new()),
                released: (Mutex::new(false), Condvar::new()),
            }
        }
    }

    impl BlockingLockActions {
        fn wait_until_started(&self) {
            let started = self.started.0.lock_unpoisoned();
            let (started, timeout) = self
                .started
                .1
                .wait_timeout_while(started, Duration::from_secs(1), |started| !*started)
                .unwrap();
            assert!(*started && !timeout.timed_out());
        }

        fn release(&self) {
            *self.released.0.lock_unpoisoned() = true;
            self.released.1.notify_all();
        }

        fn simulate_successful_unlock(&self) {
            self.locked.store(false, Ordering::SeqCst);
        }
    }

    impl LockActions for BlockingLockActions {
        fn status(&self) -> AuthStatus {
            if self.locked.load(Ordering::SeqCst) {
                AuthStatus::Locked
            } else {
                AuthStatus::Unlocked
            }
        }

        fn lock(&self) -> Result<AuthStatus, LockError> {
            self.locked.store(true, Ordering::SeqCst);
            *self.started.0.lock_unpoisoned() = true;
            self.started.1.notify_all();
            let released = self.released.0.lock_unpoisoned();
            drop(
                self.released
                    .1
                    .wait_while(released, |released| !*released)
                    .unwrap(),
            );
            Ok(AuthStatus::Locked)
        }
    }

    struct ReentrantLockActions {
        service: Mutex<Option<AutoLockService>>,
    }

    impl LockActions for ReentrantLockActions {
        fn status(&self) -> AuthStatus {
            AuthStatus::Unlocked
        }

        fn lock(&self) -> Result<AuthStatus, LockError> {
            let service = self.service.lock_unpoisoned().clone().unwrap();
            service.record_activity();
            service.disarm();
            Ok(AuthStatus::Locked)
        }
    }

    struct BlockingStatusActions {
        locked: AtomicBool,
        block_next_status: AtomicBool,
        status_started: (Mutex<bool>, Condvar),
        status_released: (Mutex<bool>, Condvar),
    }

    impl BlockingStatusActions {
        fn new() -> Self {
            Self {
                locked: AtomicBool::new(false),
                block_next_status: AtomicBool::new(true),
                status_started: (Mutex::new(false), Condvar::new()),
                status_released: (Mutex::new(false), Condvar::new()),
            }
        }

        fn wait_for_status(&self) {
            let started = self.status_started.0.lock_unpoisoned();
            let (started, timeout) = self
                .status_started
                .1
                .wait_timeout_while(started, Duration::from_secs(1), |started| !*started)
                .unwrap();
            assert!(*started && !timeout.timed_out());
        }

        fn release_status(&self) {
            *self.status_released.0.lock_unpoisoned() = true;
            self.status_released.1.notify_all();
        }
    }

    impl LockActions for BlockingStatusActions {
        fn status(&self) -> AuthStatus {
            if self.block_next_status.swap(false, Ordering::SeqCst) {
                *self.status_started.0.lock_unpoisoned() = true;
                self.status_started.1.notify_all();
                let released = self.status_released.0.lock_unpoisoned();
                drop(
                    self.status_released
                        .1
                        .wait_while(released, |released| !*released)
                        .unwrap(),
                );
            }
            if self.locked.load(Ordering::SeqCst) {
                AuthStatus::Locked
            } else {
                AuthStatus::Unlocked
            }
        }

        fn lock(&self) -> Result<AuthStatus, LockError> {
            self.locked.store(true, Ordering::SeqCst);
            Ok(AuthStatus::Locked)
        }
    }

    #[derive(Default)]
    struct CycleOwner {
        service: Mutex<Option<AutoLockService>>,
    }

    struct CycleActions {
        owner: Arc<CycleOwner>,
    }

    impl LockActions for CycleActions {
        fn status(&self) -> AuthStatus {
            let _keep_owner_alive = &self.owner;
            AuthStatus::Unlocked
        }

        fn lock(&self) -> Result<AuthStatus, LockError> {
            Ok(AuthStatus::Locked)
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
    fn real_supervisor_recalculates_activity_and_expires_at_exact_manual_time() {
        let start = Instant::now();
        let clock = Arc::new(ManualClock::new(start));
        let actions = Arc::new(FakeLockActions::default());
        let service = AutoLockService::new_with_clock_for_test(
            actions.clone(),
            Duration::from_secs(300),
            clock.clone(),
        );
        service.arm();
        clock.advance(Duration::from_secs(100));
        service.record_activity();
        clock.advance(Duration::from_secs(299));
        service.notify_for_test();
        assert_eq!(actions.lock_count(), 0);
        clock.advance(Duration::from_secs(1));
        service.notify_for_test();
        assert!(actions.wait_for_calls(1));
        assert_eq!(actions.lock_count(), 1);
        service.shutdown();
        assert!(service.wait_for_worker_termination_for_test(Duration::from_secs(1)));
    }

    #[test]
    fn disarmed_and_locked_states_never_lock_or_arm() {
        let actions = Arc::new(FakeLockActions::default());
        let service = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(300));
        assert!(!service.expire_at_for_test(Instant::now() + Duration::from_secs(900)));
        actions.locked.store(true, Ordering::SeqCst);
        service.arm();
        assert!(!service.is_armed_for_test());
        assert_eq!(actions.lock_count(), 0);
    }

    #[test]
    fn shortening_timeout_locks_immediately_and_extending_does_not_lock_early() {
        let actions = Arc::new(FakeLockActions::default());
        let service = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(60));
        let start = Instant::now();
        service.arm_at_for_test(start);
        service
            .set_timeout_at_for_test(Duration::from_secs(300), start + Duration::from_secs(59))
            .unwrap();
        assert!(!service.expire_at_for_test(start + Duration::from_secs(299)));
        service.record_activity_at_for_test(start + Duration::from_secs(100));
        service
            .set_timeout_at_for_test(Duration::from_secs(60), start + Duration::from_secs(170))
            .unwrap();
        assert_eq!(actions.lock_count(), 1);
        assert!(!service.is_armed_for_test());
    }

    #[test]
    fn manual_lock_disarms_and_failure_does_not_retry() {
        let actions = Arc::new(FakeLockActions::default());
        actions.fail.store(true, Ordering::SeqCst);
        let service = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(300));
        let start = Instant::now();
        service.arm_at_for_test(start);
        assert_eq!(service.lock_now(), Err(LockError::EventEmissionFailed));
        assert!(!service.expire_at_for_test(start + Duration::from_secs(301)));
        assert_eq!(actions.lock_count(), 1);
    }

    #[test]
    fn explicit_shutdown_is_idempotent_non_blocking_and_terminates_the_worker() {
        let actions = Arc::new(FakeLockActions::default());
        let service =
            AutoLockService::new_with_worker_for_test(actions, Duration::from_secs(60 * 60));
        let before = Instant::now();
        service.shutdown();
        service.shutdown();
        assert!(before.elapsed() < Duration::from_secs(1));
        assert!(service.wait_for_worker_termination_for_test(Duration::from_secs(1)));
        assert!(service.shutdown_requested_for_test());
    }

    #[test]
    fn spurious_notification_does_not_lock_early() {
        let start = Instant::now();
        let clock = Arc::new(ManualClock::new(start));
        let actions = Arc::new(FakeLockActions::default());
        let service = AutoLockService::new_with_clock_for_test(
            actions.clone(),
            Duration::from_secs(300),
            clock.clone(),
        );
        service.arm();
        service.notify_for_test();
        clock.advance(Duration::from_secs(299));
        service.notify_for_test();
        assert_eq!(actions.lock_count(), 0);
        clock.advance(Duration::from_secs(1));
        service.notify_for_test();
        assert!(actions.wait_for_calls(1));
        service.shutdown();
        assert!(service.wait_for_worker_termination_for_test(Duration::from_secs(1)));
    }

    #[test]
    fn blocked_action_does_not_block_timer_controls_or_shutdown_request() {
        let actions = Arc::new(BlockingLockActions::default());
        let service = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(300));
        service.arm();
        let locking_service = service.clone();
        let locking = thread::spawn(move || locking_service.lock_now());
        actions.wait_until_started();

        let before = Instant::now();
        service.record_activity();
        service.disarm();
        service.set_timeout(Duration::from_secs(60)).unwrap();
        service.shutdown();
        assert!(before.elapsed() < Duration::from_secs(1));

        actions.release();
        assert_eq!(locking.join().unwrap().unwrap(), AuthStatus::Locked);
    }

    #[test]
    fn reentrant_action_can_call_timer_controls_without_deadlock() {
        let actions = Arc::new(ReentrantLockActions {
            service: Mutex::new(None),
        });
        let service = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(300));
        *actions.service.lock_unpoisoned() = Some(service.clone());
        let (finished_tx, finished_rx) = mpsc::channel();
        let locking_service = service.clone();
        thread::spawn(move || {
            let _ = finished_tx.send(locking_service.lock_now());
        });

        assert_eq!(
            finished_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            Ok(AuthStatus::Locked)
        );
        actions.service.lock_unpoisoned().take();
    }

    #[test]
    fn successful_unlock_that_finishes_during_lock_io_rearms_after_reconciliation() {
        let actions = Arc::new(BlockingLockActions::default());
        let service = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(300));
        let locking_service = service.clone();
        let locking = thread::spawn(move || locking_service.lock_now());
        actions.wait_until_started();

        actions.simulate_successful_unlock();
        service.arm();
        assert!(!service.is_armed_for_test());
        actions.release();
        locking.join().unwrap().unwrap();

        assert!(service.is_armed_for_test());
    }

    #[test]
    fn arm_started_before_lock_transition_cannot_leave_locked_state_armed() {
        let actions = Arc::new(BlockingStatusActions::new());
        let service = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(300));
        let arming_service = service.clone();
        let arming = thread::spawn(move || arming_service.arm());
        actions.wait_for_status();

        assert_eq!(service.lock_now().unwrap(), AuthStatus::Locked);
        actions.release_status();
        arming.join().unwrap();

        assert!(!service.is_armed_for_test());
    }

    #[test]
    fn real_supervisor_failure_disarms_without_retrying() {
        let start = Instant::now();
        let clock = Arc::new(ManualClock::new(start));
        let actions = Arc::new(FakeLockActions::default());
        actions.fail.store(true, Ordering::SeqCst);
        let service = AutoLockService::new_with_clock_for_test(
            actions.clone(),
            Duration::from_secs(300),
            clock.clone(),
        );
        service.arm();
        clock.advance(Duration::from_secs(300));
        service.notify_for_test();
        assert!(actions.wait_for_calls(1));
        service.notify_for_test();
        assert_eq!(actions.lock_count(), 1);
        assert!(!service.is_armed_for_test());
        service.shutdown();
        assert!(service.wait_for_worker_termination_for_test(Duration::from_secs(1)));
    }

    #[test]
    fn explicit_shutdown_terminates_worker_even_when_ownership_is_cyclic() {
        let owner = Arc::new(CycleOwner::default());
        let actions = Arc::new(CycleActions {
            owner: owner.clone(),
        });
        let service =
            AutoLockService::new_with_worker_for_test(actions, Duration::from_secs(60 * 60));
        *owner.service.lock_unpoisoned() = Some(service.clone());

        service.shutdown();
        assert!(service.wait_for_worker_termination_for_test(Duration::from_secs(1)));
        assert!(service.shutdown_requested_for_test());

        owner.service.lock_unpoisoned().take();
    }
}
