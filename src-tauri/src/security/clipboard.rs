use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
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

    fn clear_if_matches(&self, expected: &str) -> Result<ClearOutcome, ClipboardError> {
        let current = Zeroizing::new(self.read_text()?);
        if current.as_str() != expected {
            return Ok(ClearOutcome::Changed);
        }
        self.clear()?;
        Ok(ClearOutcome::Cleared)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClearOutcome {
    Cleared,
    Changed,
}

type ScheduledJob = Box<dyn FnOnce() + Send + 'static>;

pub(crate) trait JobScheduler: Send + Sync {
    fn schedule(&self, delay: Duration, job: ScheduledJob) -> Result<(), ClipboardError>;
}

struct ThreadJobScheduler;

impl JobScheduler for ThreadJobScheduler {
    fn schedule(&self, delay: Duration, job: ScheduledJob) -> Result<(), ClipboardError> {
        std::thread::Builder::new()
            .name("keynest-clipboard".to_owned())
            .spawn(move || {
                if !delay.is_zero() {
                    std::thread::sleep(delay);
                }
                job();
            })
            .map(|_| ())
            .map_err(|_| ClipboardError::SchedulingFailed)
    }
}

#[derive(Clone)]
pub(crate) struct ClipboardService {
    inner: Arc<Mutex<ClipboardState>>,
    port: Arc<dyn ClipboardPort>,
    port_gate: Arc<Mutex<()>>,
    scheduler: Arc<dyn JobScheduler>,
    process_exit_started: Arc<AtomicBool>,
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
        Self::new_with_scheduler(port, timeout, Arc::new(ThreadJobScheduler))
    }

    fn new_with_scheduler(
        port: Arc<dyn ClipboardPort>,
        timeout: Duration,
        scheduler: Arc<dyn JobScheduler>,
    ) -> Self {
        debug_assert!(ALLOWED_TIMEOUTS.contains(&timeout));
        Self {
            inner: Arc::new(Mutex::new(ClipboardState {
                next_generation: 1,
                owned: None,
                timeout,
            })),
            port,
            port_gate: Arc::new(Mutex::new(())),
            scheduler,
            process_exit_started: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn copy_secret(&self, value: &str) -> Result<(), ClipboardError> {
        let plan = self.claim_secret(value)?;
        let service = self.clone();
        match self.scheduler.schedule(
            plan.timeout,
            Box::new(move || {
                // Expiry errors are deliberately ignored here: they contain no clipboard data,
                // and a failed read must never turn into a blind clear.
                let _ = service.expire_generation(plan.generation);
            }),
        ) {
            Ok(()) => Ok(()),
            Err(_) => {
                let _ = self.expire_generation(plan.generation);
                Err(ClipboardError::SchedulingFailed)
            }
        }
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

    pub(crate) fn begin_process_exit_cleanup(&self) -> bool {
        self.process_exit_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(crate) fn start_process_exit_cleanup(
        &self,
        deadline: Duration,
        finish: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<(), ClipboardError> {
        let completed = Arc::new(AtomicBool::new(false));
        let deadline_completed = completed.clone();
        let deadline_finish = finish.clone();
        if self
            .scheduler
            .schedule(
                deadline,
                Box::new(move || complete_once(&deadline_completed, &deadline_finish)),
            )
            .is_err()
        {
            complete_once(&completed, &finish);
            return Err(ClipboardError::SchedulingFailed);
        }

        let service = self.clone();
        let cleanup_completed = completed.clone();
        let cleanup_finish = finish.clone();
        if self
            .scheduler
            .schedule(
                Duration::ZERO,
                Box::new(move || {
                    let _ = service.clear_if_owned();
                    complete_once(&cleanup_completed, &cleanup_finish);
                }),
            )
            .is_err()
        {
            complete_once(&completed, &finish);
            return Err(ClipboardError::SchedulingFailed);
        }
        Ok(())
    }

    fn claim_secret(&self, value: &str) -> Result<CopyPlan, ClipboardError> {
        let _port_guard = self.lock_port_gate();
        let generation = {
            let mut state = self.lock_state();
            let generation = state.next_generation;
            let Some(next) = generation.checked_add(1) else {
                return Err(ClipboardError::GenerationExhausted);
            };
            state.next_generation = next;
            generation
        };

        // Clipboard callbacks may block or re-enter the service, so no service lock is held here.
        self.port.write_text(value)?;

        let mut state = self.lock_state();
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

        self.port.clear_if_matches(owned.value.as_str())?;
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

fn complete_once(completed: &AtomicBool, finish: &Arc<dyn Fn() + Send + Sync>) {
    if completed
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        finish();
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
    #[error("clipboard cleanup scheduling failed")]
    SchedulingFailed,
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

    #[cfg(windows)]
    fn clear_if_matches(&self, expected: &str) -> Result<ClearOutcome, ClipboardError> {
        windows_compare_and_clear(expected)
    }
}

#[cfg(windows)]
fn windows_compare_and_clear(expected: &str) -> Result<ClearOutcome, ClipboardError> {
    windows_compare_and_clear_with(&SystemWindowsClipboardApi, expected)
}

#[cfg(windows)]
trait WindowsClipboardApi {
    fn open(&self) -> bool;
    fn close(&self);
    fn text_handle(&self) -> Option<usize>;
    fn global_size(&self, handle: usize) -> usize;
    fn global_lock(&self, handle: usize) -> Option<*const u16>;
    fn global_unlock(&self, handle: usize);
    fn empty(&self) -> bool;
}

#[cfg(windows)]
struct SystemWindowsClipboardApi;

#[cfg(windows)]
impl WindowsClipboardApi for SystemWindowsClipboardApi {
    fn open(&self) -> bool {
        use windows_sys::Win32::System::DataExchange::OpenClipboard;
        unsafe { OpenClipboard(std::ptr::null_mut()) != 0 }
    }

    fn close(&self) {
        use windows_sys::Win32::System::DataExchange::CloseClipboard;
        unsafe {
            CloseClipboard();
        }
    }

    fn text_handle(&self) -> Option<usize> {
        use windows_sys::Win32::System::DataExchange::GetClipboardData;
        const CF_UNICODETEXT: u32 = 13;
        let handle = unsafe { GetClipboardData(CF_UNICODETEXT) };
        (!handle.is_null()).then_some(handle as usize)
    }

    fn global_size(&self, handle: usize) -> usize {
        use windows_sys::Win32::System::Memory::GlobalSize;
        unsafe { GlobalSize(handle as *mut core::ffi::c_void) }
    }

    fn global_lock(&self, handle: usize) -> Option<*const u16> {
        use windows_sys::Win32::System::Memory::GlobalLock;
        let pointer = unsafe { GlobalLock(handle as *mut core::ffi::c_void) }.cast::<u16>();
        (!pointer.is_null()).then_some(pointer)
    }

    fn global_unlock(&self, handle: usize) {
        use windows_sys::Win32::System::Memory::GlobalUnlock;
        unsafe {
            GlobalUnlock(handle as *mut core::ffi::c_void);
        }
    }

    fn empty(&self) -> bool {
        use windows_sys::Win32::System::DataExchange::EmptyClipboard;
        unsafe { EmptyClipboard() != 0 }
    }
}

#[cfg(windows)]
struct ClipboardGuard<'a, A: WindowsClipboardApi>(&'a A);

#[cfg(windows)]
impl<A: WindowsClipboardApi> Drop for ClipboardGuard<'_, A> {
    fn drop(&mut self) {
        self.0.close();
    }
}

#[cfg(windows)]
struct GlobalUnlockGuard<'a, A: WindowsClipboardApi> {
    api: &'a A,
    handle: usize,
}

#[cfg(windows)]
impl<A: WindowsClipboardApi> Drop for GlobalUnlockGuard<'_, A> {
    fn drop(&mut self) {
        self.api.global_unlock(self.handle);
    }
}

#[cfg(windows)]
fn windows_compare_and_clear_with(
    api: &impl WindowsClipboardApi,
    expected: &str,
) -> Result<ClearOutcome, ClipboardError> {
    use std::slice;

    if !api.open() {
        return Err(ClipboardError::ReadFailed);
    }
    let _clipboard_guard = ClipboardGuard(api);
    let Some(handle) = api.text_handle() else {
        return Ok(ClearOutcome::Changed);
    };
    let Some(pointer) = api.global_lock(handle) else {
        return Err(ClipboardError::ReadFailed);
    };
    let _unlock_guard = GlobalUnlockGuard { api, handle };
    let size = api.global_size(handle);
    if size < size_of::<u16>() {
        return Err(ClipboardError::ReadFailed);
    }

    let units = unsafe { slice::from_raw_parts(pointer, size / size_of::<u16>()) };
    let length = units
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(units.len());
    let matches = units[..length].iter().copied().eq(expected.encode_utf16());
    if !matches {
        return Ok(ClearOutcome::Changed);
    }
    if !api.empty() {
        return Err(ClipboardError::ClearFailed);
    }
    Ok(ClearOutcome::Cleared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{mpsc, Arc, Condvar, Mutex},
        time::Duration,
    };

    type ScheduledJob = (Duration, Box<dyn FnOnce() + Send>);

    #[derive(Default)]
    struct FakeScheduler {
        jobs: Mutex<Vec<ScheduledJob>>,
        fail: Mutex<bool>,
    }

    impl FakeScheduler {
        fn failing() -> Self {
            Self {
                jobs: Mutex::new(Vec::new()),
                fail: Mutex::new(true),
            }
        }

        fn run_next(&self) {
            let (_, job) = self.jobs.lock().unwrap().remove(0);
            job();
        }

        fn delays(&self) -> Vec<Duration> {
            self.jobs
                .lock()
                .unwrap()
                .iter()
                .map(|(delay, _)| *delay)
                .collect()
        }
    }

    impl JobScheduler for FakeScheduler {
        fn schedule(
            &self,
            delay: Duration,
            job: Box<dyn FnOnce() + Send>,
        ) -> Result<(), ClipboardError> {
            if *self.fail.lock().unwrap() {
                return Err(ClipboardError::SchedulingFailed);
            }
            self.jobs.lock().unwrap().push((delay, job));
            Ok(())
        }
    }

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
        external_write_during_compare: Mutex<Option<String>>,
        block_compare: (Mutex<bool>, Condvar),
        block_write: (Mutex<WriteBlock>, Condvar),
    }

    #[derive(Default)]
    struct WriteBlock {
        blocked: bool,
        entered: bool,
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

        fn attempt_external_write_during_compare(&self, value: &str) {
            *self.external_write_during_compare.lock().unwrap() = Some(value.to_owned());
        }

        fn block_compare(&self) {
            *self.block_compare.0.lock().unwrap() = true;
        }

        fn release_compare(&self) {
            *self.block_compare.0.lock().unwrap() = false;
            self.block_compare.1.notify_all();
        }

        fn block_write(&self) {
            self.block_write.0.lock().unwrap().blocked = true;
        }

        fn wait_until_write_is_blocked(&self) {
            let mut state = self.block_write.0.lock().unwrap();
            while !state.entered {
                state = self.block_write.1.wait(state).unwrap();
            }
        }

        fn release_write(&self) {
            let mut state = self.block_write.0.lock().unwrap();
            state.blocked = false;
            self.block_write.1.notify_all();
        }
    }

    impl ClipboardPort for FakeClipboard {
        fn write_text(&self, value: &str) -> Result<(), ClipboardError> {
            self.invoke_callback();
            let mut write = self.block_write.0.lock().unwrap();
            if write.blocked {
                write.entered = true;
                self.block_write.1.notify_all();
                while write.blocked {
                    write = self.block_write.1.wait(write).unwrap();
                }
            }
            drop(write);
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

        fn clear_if_matches(&self, expected: &str) -> Result<ClearOutcome, ClipboardError> {
            self.invoke_callback();
            if *self.failure.lock().unwrap() == Some(Failure::Read) {
                return Err(ClipboardError::ReadFailed);
            }
            let mut blocked = self.block_compare.0.lock().unwrap();
            while *blocked {
                blocked = self.block_compare.1.wait(blocked).unwrap();
            }
            drop(blocked);

            let mut text = self.text.lock().unwrap();
            if text.as_str() != expected {
                return Ok(ClearOutcome::Changed);
            }
            let pending = self.external_write_during_compare.lock().unwrap().take();
            if *self.failure.lock().unwrap() == Some(Failure::Clear) {
                return Err(ClipboardError::ClearFailed);
            }
            *self.clears.lock().unwrap() += 1;
            text.clear();
            drop(text);
            if let Some(value) = pending {
                self.set_text(&value);
            }
            Ok(ClearOutcome::Cleared)
        }
    }

    fn service(port: Arc<FakeClipboard>) -> ClipboardService {
        ClipboardService::new(port, Duration::from_secs(30))
    }

    fn service_with_scheduler(
        port: Arc<FakeClipboard>,
        scheduler: Arc<dyn JobScheduler>,
    ) -> ClipboardService {
        ClipboardService::new_with_scheduler(port, Duration::from_secs(30), scheduler)
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
    fn external_write_attempted_between_compare_and_clear_is_preserved() {
        let port = Arc::new(FakeClipboard::default());
        let service = service(port.clone());
        let generation = service.copy_secret_for_test("secret-one").unwrap();
        port.attempt_external_write_during_compare("newer user value");

        service.expire_generation_for_test(generation).unwrap();

        assert_eq!(port.text(), "newer user value");
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
    fn production_copy_secret_schedules_and_expires_the_owned_generation() {
        let port = Arc::new(FakeClipboard::default());
        let scheduler = Arc::new(FakeScheduler::default());
        let service = service_with_scheduler(port.clone(), scheduler.clone());

        service.copy_secret("secret").unwrap();

        assert_eq!(scheduler.delays(), vec![Duration::from_secs(30)]);
        scheduler.run_next();
        assert_eq!(port.text(), "");
        assert!(!service.has_owned_value_for_test());
    }

    #[test]
    fn copy_captures_timeout_after_a_blocked_write_succeeds() {
        let port = Arc::new(FakeClipboard::default());
        port.block_write();
        let scheduler = Arc::new(FakeScheduler::default());
        let service = ClipboardService::new_with_scheduler(
            port.clone(),
            Duration::from_secs(60),
            scheduler.clone(),
        );
        let copy_service = service.clone();
        let copy = std::thread::spawn(move || copy_service.copy_secret("secret"));

        port.wait_until_write_is_blocked();
        service.set_timeout(Duration::from_secs(10)).unwrap();
        port.release_write();

        copy.join().unwrap().unwrap();
        assert_eq!(scheduler.delays(), vec![Duration::from_secs(10)]);
        assert_eq!(port.text(), "secret");
        assert!(service.has_owned_value_for_test());
    }

    #[cfg(windows)]
    mod windows_api_tests {
        use super::*;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct FakeWindowsClipboardApi {
            open: bool,
            has_handle: bool,
            lock: bool,
            size: usize,
            text: Vec<u16>,
            empty: bool,
            closes: AtomicUsize,
            unlocks: AtomicUsize,
            empties: AtomicUsize,
        }

        impl FakeWindowsClipboardApi {
            fn text(value: &str) -> Self {
                let mut text: Vec<u16> = value.encode_utf16().collect();
                text.push(0);
                Self {
                    open: true,
                    has_handle: true,
                    lock: true,
                    size: text.len() * size_of::<u16>(),
                    text,
                    empty: true,
                    closes: AtomicUsize::new(0),
                    unlocks: AtomicUsize::new(0),
                    empties: AtomicUsize::new(0),
                }
            }
        }

        impl WindowsClipboardApi for FakeWindowsClipboardApi {
            fn open(&self) -> bool {
                self.open
            }

            fn close(&self) {
                self.closes.fetch_add(1, Ordering::SeqCst);
            }

            fn text_handle(&self) -> Option<usize> {
                self.has_handle.then_some(1)
            }

            fn global_size(&self, _handle: usize) -> usize {
                self.size
            }

            fn global_lock(&self, _handle: usize) -> Option<*const u16> {
                self.lock.then_some(self.text.as_ptr())
            }

            fn global_unlock(&self, _handle: usize) {
                self.unlocks.fetch_add(1, Ordering::SeqCst);
            }

            fn empty(&self) -> bool {
                self.empties.fetch_add(1, Ordering::SeqCst);
                self.empty
            }
        }

        fn assert_balanced(
            api: &FakeWindowsClipboardApi,
            result: Result<ClearOutcome, ClipboardError>,
            expected: Result<ClearOutcome, ClipboardError>,
            unlocks: usize,
            empties: usize,
        ) {
            assert_eq!(format!("{result:?}"), format!("{expected:?}"));
            assert_eq!(api.closes.load(Ordering::SeqCst), usize::from(api.open));
            assert_eq!(api.unlocks.load(Ordering::SeqCst), unlocks);
            assert_eq!(api.empties.load(Ordering::SeqCst), empties);
        }

        #[test]
        fn windows_wrapper_balances_guards_on_every_result_path() {
            let mut open_failed = FakeWindowsClipboardApi::text("secret");
            open_failed.open = false;
            assert_balanced(
                &open_failed,
                windows_compare_and_clear_with(&open_failed, "secret"),
                Err(ClipboardError::ReadFailed),
                0,
                0,
            );

            let matching = FakeWindowsClipboardApi::text("secret");
            assert_balanced(
                &matching,
                windows_compare_and_clear_with(&matching, "secret"),
                Ok(ClearOutcome::Cleared),
                1,
                1,
            );

            let changed = FakeWindowsClipboardApi::text("changed");
            assert_balanced(
                &changed,
                windows_compare_and_clear_with(&changed, "secret"),
                Ok(ClearOutcome::Changed),
                1,
                0,
            );

            let mut missing = FakeWindowsClipboardApi::text("secret");
            missing.has_handle = false;
            assert_balanced(
                &missing,
                windows_compare_and_clear_with(&missing, "secret"),
                Ok(ClearOutcome::Changed),
                0,
                0,
            );

            let mut lock_failed = FakeWindowsClipboardApi::text("secret");
            lock_failed.lock = false;
            assert_balanced(
                &lock_failed,
                windows_compare_and_clear_with(&lock_failed, "secret"),
                Err(ClipboardError::ReadFailed),
                0,
                0,
            );

            let mut malformed = FakeWindowsClipboardApi::text("secret");
            malformed.size = 1;
            assert_balanced(
                &malformed,
                windows_compare_and_clear_with(&malformed, "secret"),
                Err(ClipboardError::ReadFailed),
                1,
                0,
            );

            let mut empty_failed = FakeWindowsClipboardApi::text("secret");
            empty_failed.empty = false;
            assert_balanced(
                &empty_failed,
                windows_compare_and_clear_with(&empty_failed, "secret"),
                Err(ClipboardError::ClearFailed),
                1,
                1,
            );
        }
    }

    #[test]
    fn scheduler_failure_rolls_back_ownership_and_returns_a_safe_error() {
        let secret = "secret-that-must-not-escape";
        let port = Arc::new(FakeClipboard::default());
        let scheduler = Arc::new(FakeScheduler::failing());
        let service = service_with_scheduler(port.clone(), scheduler);

        let error = service.copy_secret(secret).unwrap_err();

        assert!(matches!(error, ClipboardError::SchedulingFailed));
        assert_eq!(port.text(), "");
        assert!(!service.has_owned_value_for_test());
        assert!(!error.to_string().contains(secret));
        assert!(!format!("{error:?}").contains(secret));
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
    fn generation_exhaustion_returns_before_any_port_io_even_when_port_would_fail() {
        let port = Arc::new(FakeClipboard::default());
        let service = service(port.clone());
        service.copy_secret_for_test("prior secret").unwrap();
        service.lock_state().next_generation = u64::MAX;
        port.fail_with(Failure::Read);

        assert!(matches!(
            service.copy_secret_for_test("final secret"),
            Err(ClipboardError::GenerationExhausted)
        ));
        assert_eq!(port.text(), "prior secret");
        assert!(service.has_owned_value_for_test());
        port.fail_with(Failure::Clear);
        assert!(matches!(
            service.copy_secret_for_test("final secret"),
            Err(ClipboardError::GenerationExhausted)
        ));
        assert_eq!(port.text(), "prior secret");
    }

    #[test]
    fn process_exit_cleanup_is_non_blocking_and_bounded() {
        let port = Arc::new(FakeClipboard::default());
        let service = service(port.clone());
        service.copy_secret_for_test("secret").unwrap();
        port.block_compare();
        let (finished_tx, finished_rx) = mpsc::channel();

        assert!(service.begin_process_exit_cleanup());
        let started = std::time::Instant::now();
        service
            .start_process_exit_cleanup(
                Duration::from_millis(25),
                Arc::new(move || {
                    let _ = finished_tx.send(());
                }),
            )
            .unwrap();
        assert!(started.elapsed() < Duration::from_millis(100));
        finished_rx
            .recv_timeout(Duration::from_millis(500))
            .unwrap();
        assert!(started.elapsed() < Duration::from_millis(500));
        port.release_compare();
    }

    #[test]
    fn process_exit_cleanup_allows_progress_and_prevents_recursive_restart() {
        let port = Arc::new(FakeClipboard::default());
        let service = service(port.clone());
        service.copy_secret_for_test("secret").unwrap();
        port.block_compare();
        let (finished_tx, finished_rx) = mpsc::channel();

        assert!(service.begin_process_exit_cleanup());
        assert!(!service.begin_process_exit_cleanup());
        service
            .start_process_exit_cleanup(
                Duration::from_secs(1),
                Arc::new(move || {
                    let _ = finished_tx.send(());
                }),
            )
            .unwrap();
        port.release_compare();

        finished_rx
            .recv_timeout(Duration::from_millis(500))
            .unwrap();
        assert_eq!(port.text(), "");
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
