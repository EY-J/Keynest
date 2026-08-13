use std::{ffi::OsStr, sync::Arc};

use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use thiserror::Error;

pub(crate) trait StartupRegistration: Send + Sync {
    fn is_enabled(&self) -> Result<bool, StartupError>;
    fn enable(&self) -> Result<(), StartupError>;
    fn disable(&self) -> Result<(), StartupError>;
}

#[derive(Clone)]
pub(crate) struct StartupService {
    registration: Arc<dyn StartupRegistration>,
}

impl StartupService {
    pub(crate) fn new(registration: Arc<dyn StartupRegistration>) -> Self {
        Self { registration }
    }

    pub(crate) fn is_enabled(&self) -> Result<bool, StartupError> {
        self.registration.is_enabled()
    }

    pub(crate) fn set_enabled(&self, requested: bool) -> Result<bool, StartupError> {
        if requested {
            self.registration.enable()?;
        } else {
            self.registration.disable()?;
        }
        self.registration.is_enabled()
    }
}

#[derive(Clone)]
pub(crate) struct TauriStartupRegistration {
    app: AppHandle,
}

impl TauriStartupRegistration {
    pub(crate) fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl StartupRegistration for TauriStartupRegistration {
    fn is_enabled(&self) -> Result<bool, StartupError> {
        self.app
            .autolaunch()
            .is_enabled()
            .map_err(|_| StartupError::QueryFailed)
    }

    fn enable(&self) -> Result<(), StartupError> {
        self.app
            .autolaunch()
            .enable()
            .map_err(|_| StartupError::MutationFailed)
    }

    fn disable(&self) -> Result<(), StartupError> {
        self.app
            .autolaunch()
            .disable()
            .map_err(|_| StartupError::MutationFailed)
    }
}

pub(crate) trait MainWindowMinimizer {
    fn minimize_main(&self) -> Result<(), StartupError>;
}

pub(crate) struct TauriMainWindowMinimizer<'a> {
    app: &'a tauri::App,
}

impl<'a> TauriMainWindowMinimizer<'a> {
    pub(crate) fn new(app: &'a tauri::App) -> Self {
        Self { app }
    }
}

impl MainWindowMinimizer for TauriMainWindowMinimizer<'_> {
    fn minimize_main(&self) -> Result<(), StartupError> {
        let window = self
            .app
            .get_webview_window("main")
            .ok_or(StartupError::WindowUnavailable)?;
        window
            .minimize()
            .map_err(|_| StartupError::WindowUnavailable)
    }
}

pub(crate) fn is_autostart_launch<I, S>(arguments: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    arguments
        .into_iter()
        .any(|argument| argument.as_ref() == OsStr::new("--autostart"))
}

pub(crate) fn minimize_for_launch<I, S>(
    arguments: I,
    window: &dyn MainWindowMinimizer,
) -> Result<bool, StartupError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    if !is_autostart_launch(arguments) {
        return Ok(false);
    }
    window.minimize_main()?;
    Ok(true)
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub(crate) enum StartupError {
    #[error("startup registration could not be changed")]
    MutationFailed,
    #[error("startup registration state could not be read")]
    QueryFailed,
    #[error("startup registration did not reach the required state")]
    StateMismatch,
    #[error("the main window could not be minimized")]
    WindowUnavailable,
}

#[cfg(test)]
mod startup_tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Default)]
    struct FakeState {
        enabled: bool,
        fail_enable: bool,
        fail_disable: bool,
        fail_query: bool,
        enable_calls: usize,
        disable_calls: usize,
        query_calls: usize,
    }

    #[derive(Clone, Default)]
    struct FakeRegistration(Arc<Mutex<FakeState>>);

    impl FakeRegistration {
        fn enabled(enabled: bool) -> Self {
            let fake = Self::default();
            fake.0.lock().unwrap().enabled = enabled;
            fake
        }
    }

    impl StartupRegistration for FakeRegistration {
        fn is_enabled(&self) -> Result<bool, StartupError> {
            let mut state = self.0.lock().unwrap();
            state.query_calls += 1;
            if state.fail_query {
                Err(StartupError::QueryFailed)
            } else {
                Ok(state.enabled)
            }
        }

        fn enable(&self) -> Result<(), StartupError> {
            let mut state = self.0.lock().unwrap();
            state.enable_calls += 1;
            if state.fail_enable {
                Err(StartupError::MutationFailed)
            } else {
                state.enabled = true;
                Ok(())
            }
        }

        fn disable(&self) -> Result<(), StartupError> {
            let mut state = self.0.lock().unwrap();
            state.disable_calls += 1;
            if state.fail_disable {
                Err(StartupError::MutationFailed)
            } else {
                state.enabled = false;
                Ok(())
            }
        }
    }

    #[test]
    fn initially_disabled_and_enabled_states_are_queried_from_registration() {
        let disabled = StartupService::new(Arc::new(FakeRegistration::enabled(false)));
        let enabled = StartupService::new(Arc::new(FakeRegistration::enabled(true)));
        assert!(!disabled.is_enabled().unwrap());
        assert!(enabled.is_enabled().unwrap());
    }

    #[test]
    fn enable_and_disable_return_only_the_confirmed_actual_state() {
        let registration = FakeRegistration::enabled(false);
        let service = StartupService::new(Arc::new(registration.clone()));
        assert!(service.set_enabled(true).unwrap());
        assert!(!service.set_enabled(false).unwrap());
        let state = registration.0.lock().unwrap();
        assert_eq!(state.enable_calls, 1);
        assert_eq!(state.disable_calls, 1);
        assert_eq!(state.query_calls, 2);
    }

    #[test]
    fn action_failure_retains_actual_state_and_is_not_reported_optimistically() {
        let registration = FakeRegistration::enabled(false);
        registration.0.lock().unwrap().fail_enable = true;
        let service = StartupService::new(Arc::new(registration.clone()));
        assert_eq!(service.set_enabled(true), Err(StartupError::MutationFailed));
        let state = registration.0.lock().unwrap();
        assert!(!state.enabled);
        assert_eq!(state.query_calls, 0);
    }

    #[test]
    fn post_action_query_failure_is_returned_instead_of_requested_state() {
        let registration = FakeRegistration::enabled(false);
        registration.0.lock().unwrap().fail_query = true;
        let service = StartupService::new(Arc::new(registration.clone()));
        assert_eq!(service.set_enabled(true), Err(StartupError::QueryFailed));
        assert!(registration.0.lock().unwrap().enabled);
    }

    #[derive(Default)]
    struct FakeWindow(Mutex<usize>);

    impl MainWindowMinimizer for FakeWindow {
        fn minimize_main(&self) -> Result<(), StartupError> {
            *self.0.lock().unwrap() += 1;
            Ok(())
        }
    }

    #[test]
    fn exact_standalone_autostart_argument_minimizes_but_normal_launch_does_not() {
        let window = FakeWindow::default();
        assert!(!minimize_for_launch(["keynest", "--other"], &window).unwrap());
        assert!(!minimize_for_launch(["keynest", "--autostart=true"], &window).unwrap());
        assert!(minimize_for_launch(["keynest", "--autostart"], &window).unwrap());
        assert_eq!(*window.0.lock().unwrap(), 1);
    }

    #[test]
    fn startup_errors_are_path_and_command_safe() {
        assert_eq!(
            StartupError::MutationFailed.to_string(),
            "startup registration could not be changed"
        );
        assert!(!StartupError::QueryFailed.to_string().contains("registry"));
    }
}
