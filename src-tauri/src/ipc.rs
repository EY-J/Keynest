use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use serde::Serialize;
use tauri::State;
use thiserror::Error;
use zeroize::Zeroize;

use crate::{
    platform::startup::{StartupError, StartupService},
    security::{
        AuthError, AuthService, AuthStatus, AutoLockService, ClipboardError, ClipboardService,
        LockError, SecurityOperationGate,
    },
    settings::{SettingsError, SettingsService, SettingsSnapshot},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublicIpcError {
    pub(crate) code: &'static str,
    pub(crate) message: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) retry_after_ms: Option<u64>,
}

impl PublicIpcError {
    fn new(code: &'static str, message: &'static str) -> Self {
        Self {
            code,
            message,
            retry_after_ms: None,
        }
    }

    fn internal() -> Self {
        Self::new(
            "internal-error",
            "KeyNest could not complete the security request.",
        )
    }
}

impl From<AuthError> for PublicIpcError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::PasswordTooShort => {
                Self::new("password-too-short", "Use at least 12 characters.")
            }
            AuthError::AlreadyInitialized => Self::new(
                "already-initialized",
                "KeyNest already has a master password.",
            ),
            AuthError::NotInitialized => Self::new(
                "not-initialized",
                "Create a master password before unlocking KeyNest.",
            ),
            AuthError::InvalidCredentials => {
                Self::new("invalid-credentials", "The master password is incorrect.")
            }
            AuthError::Throttled { retry_after_ms } => Self {
                code: "throttled",
                message: "Wait a moment before trying again.",
                retry_after_ms: Some(retry_after_ms),
            },
            AuthError::InvalidResetConfirmation => Self::new(
                "invalid-reset-confirmation",
                "Type RESET KEYNEST exactly to confirm.",
            ),
            AuthError::Unauthorized => Self::new("unauthorized", "KeyNest is locked."),
            AuthError::DataDamaged => Self::new(
                "data-error",
                "KeyNest's encrypted local data is damaged or unsupported.",
            ),
            AuthError::LocalDataFailure => Self::new(
                "local-data-error",
                "KeyNest could not access its encrypted local data.",
            ),
        }
    }
}

impl From<SettingsError> for PublicIpcError {
    fn from(error: SettingsError) -> Self {
        match error {
            SettingsError::InvalidAutoLockSeconds => Self::new(
                "invalid-auto-lock",
                "Choose a supported automatic lock duration.",
            ),
            SettingsError::InvalidClipboardClearSeconds => Self::new(
                "invalid-clipboard-duration",
                "Choose a supported clipboard clearing duration.",
            ),
            SettingsError::InvalidTheme => {
                Self::new("invalid-theme", "Choose System, Dark, or Light.")
            }
            SettingsError::Storage(_) => Self::new(
                "settings-error",
                "KeyNest could not save the settings change.",
            ),
        }
    }
}

impl From<StartupError> for PublicIpcError {
    fn from(_: StartupError) -> Self {
        Self::new(
            "startup-error",
            "KeyNest could not update or confirm its startup setting.",
        )
    }
}

impl From<ClipboardError> for PublicIpcError {
    fn from(_: ClipboardError) -> Self {
        Self::new(
            "clipboard-error",
            "KeyNest could not safely clear its clipboard content.",
        )
    }
}

impl From<LockError> for PublicIpcError {
    fn from(error: LockError) -> Self {
        match error {
            LockError::ClipboardCleanupFailed => Self::from(ClipboardError::ClearFailed),
            LockError::EventEmissionFailed | LockError::OperationGateMismatch => Self::internal(),
        }
    }
}

pub(crate) trait FixedFolderOpener: Send + Sync {
    fn open(&self, path: &Path) -> Result<(), FolderError>;
}

struct SystemFolderOpener;

impl FixedFolderOpener for SystemFolderOpener {
    fn open(&self, path: &Path) -> Result<(), FolderError> {
        tauri_plugin_opener::open_path(path, None::<&str>).map_err(|_| FolderError::OpenFailed)
    }
}

#[derive(Clone)]
pub(crate) struct DataFolderService {
    app_data_dir: PathBuf,
    opener: Arc<dyn FixedFolderOpener>,
}

impl DataFolderService {
    pub(crate) fn new(app_data_dir: PathBuf) -> Self {
        Self {
            app_data_dir,
            opener: Arc::new(SystemFolderOpener),
        }
    }

    #[cfg(test)]
    fn with_opener(app_data_dir: PathBuf, opener: Arc<dyn FixedFolderOpener>) -> Self {
        Self {
            app_data_dir,
            opener,
        }
    }

    fn open(&self) -> Result<(), FolderError> {
        fs::create_dir_all(&self.app_data_dir).map_err(|_| FolderError::CreateFailed)?;
        self.opener.open(&self.app_data_dir)
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub(crate) enum FolderError {
    #[error("the KeyNest data folder could not be created")]
    CreateFailed,
    #[error("the KeyNest data folder could not be opened")]
    OpenFailed,
}

impl From<FolderError> for PublicIpcError {
    fn from(_: FolderError) -> Self {
        Self::new(
            "folder-open-error",
            "KeyNest could not open its data folder.",
        )
    }
}

fn require_unlocked(auth: &AuthService) -> Result<(), AuthError> {
    if auth.status() == AuthStatus::Unlocked {
        Ok(())
    } else {
        Err(AuthError::Unauthorized)
    }
}

pub(crate) fn create_master_password_and_arm(
    password: &str,
    auth: &AuthService,
    auto_lock: &AutoLockService,
    operation_gate: &SecurityOperationGate,
) -> Result<AuthStatus, AuthError> {
    create_master_password_and_arm_with_hook(password, auth, auto_lock, operation_gate, || {})
}

fn create_master_password_and_arm_with_hook(
    password: &str,
    auth: &AuthService,
    auto_lock: &AutoLockService,
    operation_gate: &SecurityOperationGate,
    after_create: impl FnOnce(),
) -> Result<AuthStatus, AuthError> {
    let _guard = operation_gate.lock();
    auth.create_master_password(password)?;
    after_create();
    auto_lock.arm();
    Ok(auth.status())
}

pub(crate) fn unlock_and_arm(
    password: &str,
    auth: &AuthService,
    auto_lock: &AutoLockService,
    operation_gate: &SecurityOperationGate,
) -> Result<AuthStatus, AuthError> {
    let _guard = operation_gate.lock();
    auth.unlock(password)?;
    auto_lock.arm();
    Ok(auth.status())
}

pub(crate) fn record_activity_if_unlocked(
    auth: &AuthService,
    auto_lock: &AutoLockService,
    operation_gate: &SecurityOperationGate,
) -> Result<(), AuthError> {
    let _guard = operation_gate.lock();
    require_unlocked(auth)?;
    auto_lock.record_activity();
    Ok(())
}

fn dispatch_record_activity(
    auth: AuthService,
    auto_lock: AutoLockService,
    operation_gate: SecurityOperationGate,
) -> tauri::async_runtime::JoinHandle<Result<(), PublicIpcError>> {
    tauri::async_runtime::spawn_blocking(move || {
        record_activity_if_unlocked(&auth, &auto_lock, &operation_gate).map_err(Into::into)
    })
}

pub(crate) fn get_settings_snapshot(
    settings: &SettingsService,
    startup: &StartupService,
    operation_gate: &SecurityOperationGate,
) -> Result<SettingsSnapshot, PublicIpcError> {
    let _guard = operation_gate.lock();
    settings_snapshot(settings, startup)
}

fn settings_snapshot(
    settings: &SettingsService,
    startup: &StartupService,
) -> Result<SettingsSnapshot, PublicIpcError> {
    let actual_startup = startup.is_enabled()?;
    Ok(settings.snapshot(actual_startup))
}

fn set_auto_lock_value(
    seconds: u64,
    auth: &AuthService,
    settings: &SettingsService,
    auto_lock: &AutoLockService,
    startup: &StartupService,
    operation_gate: &SecurityOperationGate,
) -> Result<SettingsSnapshot, PublicIpcError> {
    set_auto_lock_value_with_hook(
        seconds,
        auth,
        settings,
        auto_lock,
        startup,
        operation_gate,
        || {},
    )
}

fn set_auto_lock_value_with_hook(
    seconds: u64,
    auth: &AuthService,
    settings: &SettingsService,
    auto_lock: &AutoLockService,
    startup: &StartupService,
    operation_gate: &SecurityOperationGate,
    after_persist: impl FnOnce(),
) -> Result<SettingsSnapshot, PublicIpcError> {
    let guard = operation_gate.lock();
    require_unlocked(auth)?;
    settings.set_auto_lock_seconds(seconds)?;
    after_persist();
    auto_lock.set_timeout_with_operation_guard(Duration::from_secs(seconds), &guard)?;
    settings_snapshot(settings, startup)
}

fn set_clipboard_clear_value(
    seconds: u64,
    auth: &AuthService,
    settings: &SettingsService,
    clipboard: &ClipboardService,
    startup: &StartupService,
    operation_gate: &SecurityOperationGate,
) -> Result<SettingsSnapshot, PublicIpcError> {
    set_clipboard_clear_value_with_hook(
        seconds,
        auth,
        settings,
        clipboard,
        startup,
        operation_gate,
        || {},
    )
}

fn set_clipboard_clear_value_with_hook(
    seconds: u64,
    auth: &AuthService,
    settings: &SettingsService,
    clipboard: &ClipboardService,
    startup: &StartupService,
    operation_gate: &SecurityOperationGate,
    after_persist: impl FnOnce(),
) -> Result<SettingsSnapshot, PublicIpcError> {
    let _guard = operation_gate.lock();
    require_unlocked(auth)?;
    settings.set_clipboard_clear_seconds(seconds)?;
    after_persist();
    clipboard.set_timeout(Duration::from_secs(seconds))?;
    settings_snapshot(settings, startup)
}

fn set_theme_value(
    theme: &str,
    auth: &AuthService,
    settings: &SettingsService,
    startup: &StartupService,
    operation_gate: &SecurityOperationGate,
) -> Result<SettingsSnapshot, PublicIpcError> {
    let _guard = operation_gate.lock();
    require_unlocked(auth)?;
    settings.set_theme_name(theme)?;
    settings_snapshot(settings, startup)
}

fn set_startup_value(
    enabled: bool,
    auth: &AuthService,
    settings: &SettingsService,
    startup: &StartupService,
    operation_gate: &SecurityOperationGate,
) -> Result<SettingsSnapshot, PublicIpcError> {
    let _guard = operation_gate.lock();
    require_unlocked(auth)?;
    let actual = startup.set_enabled(enabled)?;
    Ok(settings.snapshot(actual))
}

fn open_fixed_data_folder(
    auth: &AuthService,
    folder: &DataFolderService,
    operation_gate: &SecurityOperationGate,
) -> Result<(), PublicIpcError> {
    let _guard = operation_gate.lock();
    require_unlocked(auth)?;
    folder.open()?;
    Ok(())
}

fn change_master_password_value(
    current_password: &str,
    new_password: &str,
    auth: &AuthService,
    operation_gate: &SecurityOperationGate,
) -> Result<AuthStatus, PublicIpcError> {
    let _guard = operation_gate.lock();
    auth.change_master_password(current_password, new_password)?;
    Ok(auth.status())
}

fn disable_startup_for_reset(startup: &StartupService) -> Result<(), PublicIpcError> {
    if startup.set_enabled(false)? {
        return Err(StartupError::StateMismatch.into());
    }
    Ok(())
}

pub(crate) fn reset_authenticated(
    current_password: &str,
    confirmation: &str,
    auth: &AuthService,
    startup: &StartupService,
    clipboard: &ClipboardService,
    settings: &SettingsService,
    auto_lock: &AutoLockService,
    operation_gate: &SecurityOperationGate,
) -> Result<AuthStatus, PublicIpcError> {
    reset_authenticated_with_hook(
        current_password,
        confirmation,
        auth,
        startup,
        clipboard,
        settings,
        auto_lock,
        operation_gate,
        || {},
    )
}

fn reset_authenticated_with_hook(
    current_password: &str,
    confirmation: &str,
    auth: &AuthService,
    startup: &StartupService,
    clipboard: &ClipboardService,
    settings: &SettingsService,
    auto_lock: &AutoLockService,
    operation_gate: &SecurityOperationGate,
    after_validation: impl FnOnce(),
) -> Result<AuthStatus, PublicIpcError> {
    let _guard = operation_gate.lock();
    auth.validate_authenticated_reset(current_password, confirmation)?;
    after_validation();
    disable_startup_for_reset(startup)?;
    clipboard.clear_if_owned()?;
    settings.reset()?;
    auth.finish_reset()?;
    auto_lock.disarm();
    Ok(auth.status())
}

fn reset_recovery(
    confirmation: &str,
    auth: &AuthService,
    startup: &StartupService,
    clipboard: &ClipboardService,
    settings: &SettingsService,
    auto_lock: &AutoLockService,
    operation_gate: &SecurityOperationGate,
) -> Result<AuthStatus, PublicIpcError> {
    reset_recovery_with_hook(
        confirmation,
        auth,
        startup,
        clipboard,
        settings,
        auto_lock,
        operation_gate,
        || {},
    )
}

fn reset_recovery_with_hook(
    confirmation: &str,
    auth: &AuthService,
    startup: &StartupService,
    clipboard: &ClipboardService,
    settings: &SettingsService,
    auto_lock: &AutoLockService,
    operation_gate: &SecurityOperationGate,
    after_validation: impl FnOnce(),
) -> Result<AuthStatus, PublicIpcError> {
    let _guard = operation_gate.lock();
    match auth.status() {
        AuthStatus::Unlocked => return Err(AuthError::Unauthorized.into()),
        AuthStatus::SetupRequired => return Err(AuthError::NotInitialized.into()),
        AuthStatus::Locked | AuthStatus::DataError => {}
    }
    auth.validate_reset_confirmation(confirmation)?;
    after_validation();
    disable_startup_for_reset(startup)?;
    clipboard.clear_if_owned()?;
    settings.reset()?;
    auth.finish_reset()?;
    auto_lock.disarm();
    Ok(auth.status())
}

#[tauri::command]
pub(crate) fn get_auth_status(auth: State<'_, AuthService>) -> AuthStatus {
    auth.status()
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn create_master_password(
    mut password: String,
    auth: State<'_, AuthService>,
    auto_lock: State<'_, AutoLockService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<AuthStatus, PublicIpcError> {
    let auth = auth.inner().clone();
    let auto_lock = auto_lock.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = create_master_password_and_arm(&password, &auth, &auto_lock, &operation_gate);
        password.zeroize();
        result.map_err(Into::into)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn unlock(
    mut password: String,
    auth: State<'_, AuthService>,
    auto_lock: State<'_, AutoLockService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<AuthStatus, PublicIpcError> {
    let auth = auth.inner().clone();
    let auto_lock = auto_lock.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = unlock_and_arm(&password, &auth, &auto_lock, &operation_gate);
        password.zeroize();
        result.map_err(Into::into)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command]
pub(crate) async fn lock(
    auto_lock: State<'_, AutoLockService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<AuthStatus, PublicIpcError> {
    let auto_lock = auto_lock.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let guard = operation_gate.lock();
        auto_lock
            .lock_now_with_operation_guard(&guard)
            .map_err(Into::into)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command]
pub(crate) async fn get_settings(
    settings: State<'_, SettingsService>,
    startup: State<'_, StartupService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<SettingsSnapshot, PublicIpcError> {
    let settings = settings.inner().clone();
    let startup = startup.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        get_settings_snapshot(&settings, &startup, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn set_auto_lock_seconds(
    seconds: u64,
    auth: State<'_, AuthService>,
    settings: State<'_, SettingsService>,
    auto_lock: State<'_, AutoLockService>,
    startup: State<'_, StartupService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<SettingsSnapshot, PublicIpcError> {
    let auth = auth.inner().clone();
    let settings = settings.inner().clone();
    let auto_lock = auto_lock.inner().clone();
    let startup = startup.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        set_auto_lock_value(
            seconds,
            &auth,
            &settings,
            &auto_lock,
            &startup,
            &operation_gate,
        )
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn set_clipboard_clear_seconds(
    seconds: u64,
    auth: State<'_, AuthService>,
    settings: State<'_, SettingsService>,
    clipboard: State<'_, ClipboardService>,
    startup: State<'_, StartupService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<SettingsSnapshot, PublicIpcError> {
    let auth = auth.inner().clone();
    let settings = settings.inner().clone();
    let clipboard = clipboard.inner().clone();
    let startup = startup.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        set_clipboard_clear_value(
            seconds,
            &auth,
            &settings,
            &clipboard,
            &startup,
            &operation_gate,
        )
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn set_theme(
    theme: String,
    auth: State<'_, AuthService>,
    settings: State<'_, SettingsService>,
    startup: State<'_, StartupService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<SettingsSnapshot, PublicIpcError> {
    let auth = auth.inner().clone();
    let settings = settings.inner().clone();
    let startup = startup.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        set_theme_value(&theme, &auth, &settings, &startup, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn set_launch_at_startup(
    enabled: bool,
    auth: State<'_, AuthService>,
    settings: State<'_, SettingsService>,
    startup: State<'_, StartupService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<SettingsSnapshot, PublicIpcError> {
    let auth = auth.inner().clone();
    let settings = settings.inner().clone();
    let startup = startup.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        set_startup_value(enabled, &auth, &settings, &startup, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command]
pub(crate) async fn record_activity(
    auth: State<'_, AuthService>,
    auto_lock: State<'_, AutoLockService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<(), PublicIpcError> {
    let task = dispatch_record_activity(
        auth.inner().clone(),
        auto_lock.inner().clone(),
        operation_gate.inner().clone(),
    );
    task.await.map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn change_master_password(
    mut current_password: String,
    mut new_password: String,
    auth: State<'_, AuthService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<AuthStatus, PublicIpcError> {
    let auth = auth.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result =
            change_master_password_value(&current_password, &new_password, &auth, &operation_gate);
        current_password.zeroize();
        new_password.zeroize();
        result
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn reset_keynest(
    mut confirmation: String,
    auth: State<'_, AuthService>,
    startup: State<'_, StartupService>,
    clipboard: State<'_, ClipboardService>,
    settings: State<'_, SettingsService>,
    auto_lock: State<'_, AutoLockService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<AuthStatus, PublicIpcError> {
    let auth = auth.inner().clone();
    let startup = startup.inner().clone();
    let clipboard = clipboard.inner().clone();
    let settings = settings.inner().clone();
    let auto_lock = auto_lock.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = reset_recovery(
            &confirmation,
            &auth,
            &startup,
            &clipboard,
            &settings,
            &auto_lock,
            &operation_gate,
        );
        confirmation.zeroize();
        result
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn reset_keynest_authenticated(
    mut current_password: String,
    mut confirmation: String,
    auth: State<'_, AuthService>,
    startup: State<'_, StartupService>,
    clipboard: State<'_, ClipboardService>,
    settings: State<'_, SettingsService>,
    auto_lock: State<'_, AutoLockService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<AuthStatus, PublicIpcError> {
    let auth = auth.inner().clone();
    let startup = startup.inner().clone();
    let clipboard = clipboard.inner().clone();
    let settings = settings.inner().clone();
    let auto_lock = auto_lock.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = reset_authenticated(
            &current_password,
            &confirmation,
            &auth,
            &startup,
            &clipboard,
            &settings,
            &auto_lock,
            &operation_gate,
        );
        current_password.zeroize();
        confirmation.zeroize();
        result
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command]
pub(crate) async fn open_keynest_data_folder(
    auth: State<'_, AuthService>,
    folder: State<'_, DataFolderService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<(), PublicIpcError> {
    let auth = auth.inner().clone();
    let folder = folder.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        open_fixed_data_folder(&auth, &folder, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[cfg(test)]
mod command_tests {
    use std::{
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            mpsc, Arc, Barrier, Mutex,
        },
        thread,
        time::{Duration, Instant},
    };

    use tempfile::{tempdir, TempDir};

    use super::*;
    use crate::{
        platform::startup::StartupRegistration,
        security::{
            ClipboardPort, CryptoError, EntropySource, KdfParams, LockCoordinator, LockEventSink,
            ProfileStore,
        },
        settings::{SettingsStore, ThemePreference},
    };

    const PASSWORD: &str = "a secure master password";

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
    struct FakeStartupState {
        enabled: bool,
        fail_enable: bool,
        fail_disable: bool,
        fail_query: bool,
        ignore_disable: bool,
        disable_calls: usize,
    }

    #[derive(Clone, Default)]
    struct FakeStartup(Arc<Mutex<FakeStartupState>>);

    impl StartupRegistration for FakeStartup {
        fn is_enabled(&self) -> Result<bool, StartupError> {
            let state = self.0.lock().unwrap();
            if state.fail_query {
                Err(StartupError::QueryFailed)
            } else {
                Ok(state.enabled)
            }
        }

        fn enable(&self) -> Result<(), StartupError> {
            let mut state = self.0.lock().unwrap();
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
                if !state.ignore_disable {
                    state.enabled = false;
                }
                Ok(())
            }
        }
    }

    #[derive(Default)]
    struct FakeClipboardPort {
        value: Mutex<String>,
        fail_read: AtomicBool,
        clear_calls: AtomicUsize,
    }

    impl ClipboardPort for FakeClipboardPort {
        fn write_text(&self, value: &str) -> Result<(), ClipboardError> {
            *self.value.lock().unwrap() = value.to_owned();
            Ok(())
        }

        fn read_text(&self) -> Result<String, ClipboardError> {
            if self.fail_read.load(Ordering::SeqCst) {
                Err(ClipboardError::ReadFailed)
            } else {
                Ok(self.value.lock().unwrap().clone())
            }
        }

        fn clear(&self) -> Result<(), ClipboardError> {
            self.clear_calls.fetch_add(1, Ordering::SeqCst);
            self.value.lock().unwrap().clear();
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeLockEvents {
        fail: AtomicBool,
        calls: AtomicUsize,
    }

    impl LockEventSink for FakeLockEvents {
        fn emit_locked(&self) -> Result<(), LockError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) {
                Err(LockError::EventEmissionFailed)
            } else {
                Ok(())
            }
        }
    }

    struct CommandFixture {
        temp: TempDir,
        store: SettingsStore,
        settings: SettingsService,
        auth: AuthService,
        lock_events: Arc<FakeLockEvents>,
        auto_lock: AutoLockService,
        startup_fake: FakeStartup,
        startup: StartupService,
        clipboard_port: Arc<FakeClipboardPort>,
        clipboard: ClipboardService,
        operation_gate: SecurityOperationGate,
    }

    impl CommandFixture {
        fn new() -> Self {
            let temp = tempdir().unwrap();
            let params = KdfParams::testing();
            let auth = AuthService::load(
                ProfileStore::new(temp.path().to_path_buf(), params),
                params,
                Arc::new(FixedEntropy),
            );
            let store = SettingsStore::new(temp.path().to_path_buf());
            let settings = SettingsService::load(store.clone()).unwrap();
            let startup_fake = FakeStartup::default();
            let startup = StartupService::new(Arc::new(startup_fake.clone()));
            let clipboard_port = Arc::new(FakeClipboardPort::default());
            let clipboard = ClipboardService::new(clipboard_port.clone(), Duration::from_secs(30));
            let operation_gate = SecurityOperationGate::new();
            let lock_events = Arc::new(FakeLockEvents::default());
            let coordinator = Arc::new(LockCoordinator::new(
                auth.clone(),
                clipboard.clone(),
                lock_events.clone(),
                operation_gate.clone(),
            ));
            let auto_lock = AutoLockService::new_for_test(coordinator, Duration::from_secs(300));
            Self {
                temp,
                store,
                settings,
                auth,
                lock_events,
                auto_lock,
                startup_fake,
                startup,
                clipboard_port,
                clipboard,
                operation_gate,
            }
        }

        fn create_unlocked_profile(&self) {
            create_master_password_and_arm(
                PASSWORD,
                &self.auth,
                &self.auto_lock,
                &self.operation_gate,
            )
            .unwrap();
            self.settings.set_theme_name("dark").unwrap();
            self.clipboard.copy_secret("owned secret").unwrap();
            self.startup_fake.0.lock().unwrap().enabled = true;
        }

        fn reset_authenticated(
            &self,
            password: &str,
            confirmation: &str,
        ) -> Result<AuthStatus, PublicIpcError> {
            reset_authenticated(
                password,
                confirmation,
                &self.auth,
                &self.startup,
                &self.clipboard,
                &self.settings,
                &self.auto_lock,
                &self.operation_gate,
            )
        }
    }

    #[test]
    fn public_error_taxonomy_is_safe_and_camel_case() {
        let cases = [
            (
                PublicIpcError::from(SettingsError::InvalidAutoLockSeconds),
                "invalid-auto-lock",
            ),
            (
                PublicIpcError::from(SettingsError::InvalidClipboardClearSeconds),
                "invalid-clipboard-duration",
            ),
            (
                PublicIpcError::from(SettingsError::InvalidTheme),
                "invalid-theme",
            ),
            (
                PublicIpcError::from(StartupError::MutationFailed),
                "startup-error",
            ),
            (
                PublicIpcError::from(ClipboardError::ClearFailed),
                "clipboard-error",
            ),
            (
                PublicIpcError::from(FolderError::OpenFailed),
                "folder-open-error",
            ),
        ];
        for (error, code) in cases {
            assert_eq!(error.code, code);
            let value = serde_json::to_value(error).unwrap();
            assert_eq!(value["code"], code);
            assert!(value.get("retryAfterMs").is_none());
            assert!(!value["message"].as_str().unwrap().contains('\\'));
        }
        let throttled = serde_json::to_value(PublicIpcError::from(AuthError::Throttled {
            retry_after_ms: 250,
        }))
        .unwrap();
        assert_eq!(throttled["retryAfterMs"], 250);

        for error in [
            StartupError::MutationFailed,
            StartupError::QueryFailed,
            StartupError::StateMismatch,
            StartupError::WindowUnavailable,
        ] {
            assert_eq!(PublicIpcError::from(error).code, "startup-error");
        }
        for error in [
            ClipboardError::WriteFailed,
            ClipboardError::ReadFailed,
            ClipboardError::ClearFailed,
            ClipboardError::InvalidTimeout,
            ClipboardError::GenerationExhausted,
            ClipboardError::SchedulingFailed,
        ] {
            assert_eq!(PublicIpcError::from(error).code, "clipboard-error");
        }
        assert_eq!(
            PublicIpcError::from(LockError::ClipboardCleanupFailed).code,
            "clipboard-error"
        );
        assert_eq!(
            PublicIpcError::from(LockError::EventEmissionFailed).code,
            "internal-error"
        );
        assert_eq!(
            PublicIpcError::from(LockError::OperationGateMismatch).code,
            "internal-error"
        );
        assert_eq!(
            PublicIpcError::from(FolderError::CreateFailed).code,
            "folder-open-error"
        );
    }

    #[test]
    fn auth_error_codes_and_messages_remain_stable() {
        let cases = [
            (AuthError::PasswordTooShort, "password-too-short"),
            (AuthError::AlreadyInitialized, "already-initialized"),
            (AuthError::NotInitialized, "not-initialized"),
            (AuthError::InvalidCredentials, "invalid-credentials"),
            (
                AuthError::InvalidResetConfirmation,
                "invalid-reset-confirmation",
            ),
            (AuthError::Unauthorized, "unauthorized"),
            (AuthError::DataDamaged, "data-error"),
            (AuthError::LocalDataFailure, "local-data-error"),
        ];
        for (error, code) in cases {
            assert_eq!(PublicIpcError::from(error).code, code);
        }
        assert_eq!(
            PublicIpcError::from(AuthError::Unauthorized).message,
            "KeyNest is locked."
        );
        assert_eq!(
            PublicIpcError::from(AuthError::InvalidResetConfirmation).message,
            "Type RESET KEYNEST exactly to confirm."
        );
    }

    #[test]
    fn settings_snapshot_has_only_camel_case_validated_fields_and_actual_startup() {
        let fixture = CommandFixture::new();
        fixture.startup_fake.0.lock().unwrap().enabled = true;
        let value = serde_json::to_value(
            get_settings_snapshot(&fixture.settings, &fixture.startup, &fixture.operation_gate)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(value.as_object().unwrap().len(), 4);
        assert_eq!(value["autoLockSeconds"], 300);
        assert_eq!(value["clipboardClearSeconds"], 30);
        assert_eq!(value["theme"], "system");
        assert_eq!(value["launchAtStartup"], true);
        assert!(value.get("auto_lock_seconds").is_none());
    }

    #[test]
    fn damaged_settings_warning_serializes_without_weakening_defaults() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("settings.json"), b"damaged").unwrap();
        let settings =
            SettingsService::load(SettingsStore::new(temp.path().to_path_buf())).unwrap();
        let startup = StartupService::new(Arc::new(FakeStartup::default()));
        let operation_gate = SecurityOperationGate::new();
        let value = serde_json::to_value(
            get_settings_snapshot(&settings, &startup, &operation_gate).unwrap(),
        )
        .unwrap();
        assert_eq!(value["autoLockSeconds"], 300);
        assert_eq!(value["clipboardClearSeconds"], 30);
        assert!(value["warning"]
            .as_str()
            .unwrap()
            .contains("secure settings defaults"));
    }

    #[test]
    fn get_settings_returns_startup_error_instead_of_a_stale_boolean() {
        let fixture = CommandFixture::new();
        fixture.startup_fake.0.lock().unwrap().fail_query = true;
        assert_eq!(
            get_settings_snapshot(&fixture.settings, &fixture.startup, &fixture.operation_gate,)
                .unwrap_err()
                .code,
            "startup-error"
        );
    }

    #[test]
    fn all_allowed_settings_values_apply_and_zero_or_unknown_values_do_not() {
        let fixture = CommandFixture::new();
        fixture.auth.create_master_password(PASSWORD).unwrap();
        for seconds in [60, 300, 900, 1800] {
            assert_eq!(
                set_auto_lock_value(
                    seconds,
                    &fixture.auth,
                    &fixture.settings,
                    &fixture.auto_lock,
                    &fixture.startup,
                    &fixture.operation_gate,
                )
                .unwrap()
                .auto_lock_seconds,
                seconds
            );
        }
        for seconds in [10, 30, 60] {
            assert_eq!(
                set_clipboard_clear_value(
                    seconds,
                    &fixture.auth,
                    &fixture.settings,
                    &fixture.clipboard,
                    &fixture.startup,
                    &fixture.operation_gate,
                )
                .unwrap()
                .clipboard_clear_seconds,
                seconds
            );
        }
        for (name, expected) in [
            ("system", ThemePreference::System),
            ("dark", ThemePreference::Dark),
            ("light", ThemePreference::Light),
        ] {
            assert_eq!(
                set_theme_value(
                    name,
                    &fixture.auth,
                    &fixture.settings,
                    &fixture.startup,
                    &fixture.operation_gate,
                )
                .unwrap()
                .theme,
                expected
            );
        }
        assert_eq!(
            set_auto_lock_value(
                0,
                &fixture.auth,
                &fixture.settings,
                &fixture.auto_lock,
                &fixture.startup,
                &fixture.operation_gate,
            )
            .unwrap_err()
            .code,
            "invalid-auto-lock"
        );
        assert_eq!(
            set_clipboard_clear_value(
                0,
                &fixture.auth,
                &fixture.settings,
                &fixture.clipboard,
                &fixture.startup,
                &fixture.operation_gate,
            )
            .unwrap_err()
            .code,
            "invalid-clipboard-duration"
        );
        assert_eq!(
            set_theme_value(
                "never",
                &fixture.auth,
                &fixture.settings,
                &fixture.startup,
                &fixture.operation_gate,
            )
            .unwrap_err()
            .code,
            "invalid-theme"
        );
    }

    #[test]
    fn settings_mutations_require_unlocked_before_any_change() {
        let fixture = CommandFixture::new();
        let before = fixture.settings.snapshot(false);
        assert_eq!(
            set_auto_lock_value(
                60,
                &fixture.auth,
                &fixture.settings,
                &fixture.auto_lock,
                &fixture.startup,
                &fixture.operation_gate,
            )
            .unwrap_err()
            .code,
            "unauthorized"
        );
        assert_eq!(
            set_clipboard_clear_value(
                10,
                &fixture.auth,
                &fixture.settings,
                &fixture.clipboard,
                &fixture.startup,
                &fixture.operation_gate,
            )
            .unwrap_err()
            .code,
            "unauthorized"
        );
        assert_eq!(
            set_theme_value(
                "dark",
                &fixture.auth,
                &fixture.settings,
                &fixture.startup,
                &fixture.operation_gate,
            )
            .unwrap_err()
            .code,
            "unauthorized"
        );
        assert_eq!(
            set_startup_value(
                true,
                &fixture.auth,
                &fixture.settings,
                &fixture.startup,
                &fixture.operation_gate,
            )
            .unwrap_err()
            .code,
            "unauthorized"
        );
        assert_eq!(fixture.settings.snapshot(false), before);
        let state = fixture.startup_fake.0.lock().unwrap();
        assert!(!state.enabled);
        assert_eq!(state.disable_calls, 0);
    }

    #[test]
    fn protected_mutation_waiting_behind_a_linearized_lock_has_no_side_effect() {
        let fixture = CommandFixture::new();
        create_master_password_and_arm(
            PASSWORD,
            &fixture.auth,
            &fixture.auto_lock,
            &fixture.operation_gate,
        )
        .unwrap();
        let guard = fixture.operation_gate.lock();

        let auth = fixture.auth.clone();
        let settings = fixture.settings.clone();
        let startup = fixture.startup.clone();
        let operation_gate = fixture.operation_gate.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let mutation = thread::spawn(move || {
            started_tx.send(()).unwrap();
            set_theme_value("dark", &auth, &settings, &startup, &operation_gate)
        });
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();

        assert_eq!(
            fixture
                .auto_lock
                .lock_now_with_operation_guard(&guard)
                .unwrap(),
            AuthStatus::Locked
        );
        drop(guard);

        assert_eq!(mutation.join().unwrap().unwrap_err().code, "unauthorized");
        assert_eq!(
            fixture.settings.snapshot(false).theme,
            ThemePreference::System
        );
        assert!(!fixture.temp.path().join("settings.json").exists());
    }

    #[test]
    fn persistence_precedes_runtime_application_and_runtime_failure_is_truthful() {
        let fixture = CommandFixture::new();
        fixture.auth.create_master_password(PASSWORD).unwrap();
        fixture
            .auto_lock
            .arm_at_for_test(Instant::now() - Duration::from_secs(120));
        fixture.lock_events.fail.store(true, Ordering::SeqCst);
        assert_eq!(
            set_auto_lock_value(
                60,
                &fixture.auth,
                &fixture.settings,
                &fixture.auto_lock,
                &fixture.startup,
                &fixture.operation_gate,
            )
            .unwrap_err()
            .code,
            "internal-error"
        );
        assert_eq!(fixture.settings.snapshot(false).auto_lock_seconds, 60);
        assert_eq!(
            fixture.auto_lock.timeout_for_test(),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn concurrent_auto_lock_mutations_cannot_apply_a_stale_weaker_runtime_timeout_last() {
        let fixture = CommandFixture::new();
        fixture.auth.create_master_password(PASSWORD).unwrap();
        let persisted = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));

        let auth = fixture.auth.clone();
        let settings = fixture.settings.clone();
        let auto_lock = fixture.auto_lock.clone();
        let startup = fixture.startup.clone();
        let operation_gate = fixture.operation_gate.clone();
        let persisted_a = persisted.clone();
        let release_a = release.clone();
        let first = thread::spawn(move || {
            set_auto_lock_value_with_hook(
                1800,
                &auth,
                &settings,
                &auto_lock,
                &startup,
                &operation_gate,
                || {
                    persisted_a.wait();
                    release_a.wait();
                },
            )
            .unwrap()
        });
        persisted.wait();

        let auth = fixture.auth.clone();
        let settings = fixture.settings.clone();
        let auto_lock = fixture.auto_lock.clone();
        let startup = fixture.startup.clone();
        let operation_gate = fixture.operation_gate.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let (returned_tx, returned_rx) = mpsc::channel();
        let second = thread::spawn(move || {
            started_tx.send(()).unwrap();
            let result =
                set_auto_lock_value(60, &auth, &settings, &auto_lock, &startup, &operation_gate);
            returned_tx.send(()).unwrap();
            result.unwrap()
        });
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(returned_rx
            .recv_timeout(Duration::from_millis(100))
            .is_err());

        release.wait();
        assert_eq!(first.join().unwrap().auto_lock_seconds, 1800);
        assert_eq!(second.join().unwrap().auto_lock_seconds, 60);
        assert_eq!(fixture.settings.snapshot(false).auto_lock_seconds, 60);
        assert_eq!(
            fixture.auto_lock.timeout_for_test(),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn shorter_elapsed_timeout_reuses_matching_guard_without_deadlock_and_locks_once() {
        let fixture = CommandFixture::new();
        create_master_password_and_arm(
            PASSWORD,
            &fixture.auth,
            &fixture.auto_lock,
            &fixture.operation_gate,
        )
        .unwrap();
        fixture.clipboard.copy_secret("owned secret").unwrap();
        fixture
            .auto_lock
            .arm_at_for_test(Instant::now() - Duration::from_secs(120));

        let auth = fixture.auth.clone();
        let settings = fixture.settings.clone();
        let auto_lock = fixture.auto_lock.clone();
        let startup = fixture.startup.clone();
        let operation_gate = fixture.operation_gate.clone();
        let (returned_tx, returned_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            let result =
                set_auto_lock_value(60, &auth, &settings, &auto_lock, &startup, &operation_gate);
            returned_tx.send(result).unwrap();
        });

        let snapshot = returned_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("guard-aware immediate lock must not self-deadlock")
            .unwrap();
        worker.join().unwrap();

        assert_eq!(snapshot.auto_lock_seconds, 60);
        assert_eq!(fixture.auth.status(), AuthStatus::Locked);
        assert_eq!(fixture.lock_events.calls.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.clipboard_port.clear_calls.load(Ordering::SeqCst), 1);
        assert!(!fixture.auto_lock.is_armed_for_test());
    }

    #[test]
    fn concurrent_clipboard_mutations_cannot_apply_a_stale_weaker_runtime_timeout_last() {
        let fixture = CommandFixture::new();
        fixture.auth.create_master_password(PASSWORD).unwrap();
        let persisted = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));

        let auth = fixture.auth.clone();
        let settings = fixture.settings.clone();
        let clipboard = fixture.clipboard.clone();
        let startup = fixture.startup.clone();
        let operation_gate = fixture.operation_gate.clone();
        let persisted_a = persisted.clone();
        let release_a = release.clone();
        let first = thread::spawn(move || {
            set_clipboard_clear_value_with_hook(
                60,
                &auth,
                &settings,
                &clipboard,
                &startup,
                &operation_gate,
                || {
                    persisted_a.wait();
                    release_a.wait();
                },
            )
            .unwrap()
        });
        persisted.wait();

        let auth = fixture.auth.clone();
        let settings = fixture.settings.clone();
        let clipboard = fixture.clipboard.clone();
        let startup = fixture.startup.clone();
        let operation_gate = fixture.operation_gate.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let (returned_tx, returned_rx) = mpsc::channel();
        let second = thread::spawn(move || {
            started_tx.send(()).unwrap();
            let result = set_clipboard_clear_value(
                10,
                &auth,
                &settings,
                &clipboard,
                &startup,
                &operation_gate,
            );
            returned_tx.send(()).unwrap();
            result.unwrap()
        });
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(returned_rx
            .recv_timeout(Duration::from_millis(100))
            .is_err());

        release.wait();
        assert_eq!(first.join().unwrap().clipboard_clear_seconds, 60);
        assert_eq!(second.join().unwrap().clipboard_clear_seconds, 10);
        assert_eq!(fixture.settings.snapshot(false).clipboard_clear_seconds, 10);
        assert_eq!(
            fixture.clipboard.timeout_for_test(),
            Duration::from_secs(10)
        );
    }

    #[test]
    fn clipboard_runtime_timeout_and_actual_startup_snapshot_update_after_persistence() {
        let fixture = CommandFixture::new();
        fixture.auth.create_master_password(PASSWORD).unwrap();
        let snapshot = set_clipboard_clear_value(
            60,
            &fixture.auth,
            &fixture.settings,
            &fixture.clipboard,
            &fixture.startup,
            &fixture.operation_gate,
        )
        .unwrap();
        assert_eq!(
            fixture.clipboard.timeout_for_test(),
            Duration::from_secs(60)
        );
        assert_eq!(snapshot.clipboard_clear_seconds, 60);
        {
            let mut startup = fixture.startup_fake.0.lock().unwrap();
            startup.enabled = true;
            startup.ignore_disable = true;
        }
        let snapshot = set_startup_value(
            false,
            &fixture.auth,
            &fixture.settings,
            &fixture.startup,
            &fixture.operation_gate,
        )
        .unwrap();
        assert!(snapshot.launch_at_startup);
    }

    #[test]
    fn startup_or_persistence_failure_does_not_fabricate_success() {
        let fixture = CommandFixture::new();
        fixture.auth.create_master_password(PASSWORD).unwrap();
        fixture.startup_fake.0.lock().unwrap().fail_enable = true;
        assert_eq!(
            set_startup_value(
                true,
                &fixture.auth,
                &fixture.settings,
                &fixture.startup,
                &fixture.operation_gate,
            )
            .unwrap_err()
            .code,
            "startup-error"
        );
        let temp = tempdir().unwrap();
        let blocked = temp.path().join("blocked");
        fs::write(&blocked, b"file").unwrap();
        let settings = SettingsService::load(SettingsStore::new(blocked)).unwrap();
        assert_eq!(
            set_theme_value(
                "dark",
                &fixture.auth,
                &settings,
                &fixture.startup,
                &fixture.operation_gate,
            )
            .unwrap_err()
            .code,
            "settings-error"
        );
        assert_eq!(settings.snapshot(false).theme, ThemePreference::System);
    }

    #[derive(Default)]
    struct RecordingFolderOpener {
        paths: Mutex<Vec<PathBuf>>,
        fail: AtomicBool,
    }

    impl FixedFolderOpener for RecordingFolderOpener {
        fn open(&self, path: &Path) -> Result<(), FolderError> {
            self.paths.lock().unwrap().push(path.to_path_buf());
            if self.fail.load(Ordering::SeqCst) {
                Err(FolderError::OpenFailed)
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn data_folder_action_creates_and_opens_only_the_fixed_resolved_path() {
        let fixture = CommandFixture::new();
        fixture.auth.create_master_password(PASSWORD).unwrap();
        let fixed = fixture.temp.path().join("fixed").join("keynest");
        let opener = Arc::new(RecordingFolderOpener::default());
        let folder = DataFolderService::with_opener(fixed.clone(), opener.clone());
        open_fixed_data_folder(&fixture.auth, &folder, &fixture.operation_gate).unwrap();
        assert!(fixed.is_dir());
        assert_eq!(opener.paths.lock().unwrap().as_slice(), [fixed]);
    }

    #[test]
    fn unauthorized_or_failed_folder_open_is_safe_and_has_no_caller_path() {
        let fixture = CommandFixture::new();
        let fixed = fixture.temp.path().join("fixed");
        let opener = Arc::new(RecordingFolderOpener::default());
        let folder = DataFolderService::with_opener(fixed.clone(), opener.clone());
        assert_eq!(
            open_fixed_data_folder(&fixture.auth, &folder, &fixture.operation_gate)
                .unwrap_err()
                .code,
            "unauthorized"
        );
        assert!(!fixed.exists());
        assert!(opener.paths.lock().unwrap().is_empty());
        fixture.auth.create_master_password(PASSWORD).unwrap();
        opener.fail.store(true, Ordering::SeqCst);
        assert_eq!(
            open_fixed_data_folder(&fixture.auth, &folder, &fixture.operation_gate)
                .unwrap_err()
                .code,
            "folder-open-error"
        );
    }

    #[test]
    fn data_folder_creation_failure_is_safe_and_never_calls_the_opener() {
        let fixture = CommandFixture::new();
        fixture.auth.create_master_password(PASSWORD).unwrap();
        let blocked_parent = fixture.temp.path().join("blocked-parent");
        fs::write(&blocked_parent, b"regular file").unwrap();
        let fixed = blocked_parent.join("keynest");
        let opener = Arc::new(RecordingFolderOpener::default());
        let folder = DataFolderService::with_opener(fixed, opener.clone());

        let error =
            open_fixed_data_folder(&fixture.auth, &folder, &fixture.operation_gate).unwrap_err();

        assert_eq!(error.code, "folder-open-error");
        assert!(opener.paths.lock().unwrap().is_empty());
    }

    #[test]
    fn reset_validation_failures_change_nothing() {
        let fixture = CommandFixture::new();
        fixture.create_unlocked_profile();
        for (password, phrase) in [("wrong password", "RESET KEYNEST"), (PASSWORD, "RESET")] {
            assert!(fixture.reset_authenticated(password, phrase).is_err());
            assert!(fixture.temp.path().join("profile.json").is_file());
            assert!(fixture.temp.path().join("settings.json").is_file());
            assert!(fixture.startup_fake.0.lock().unwrap().enabled);
            assert_eq!(fixture.clipboard_port.clear_calls.load(Ordering::SeqCst), 0);
            assert_eq!(fixture.auth.status(), AuthStatus::Unlocked);
        }
    }

    #[test]
    fn reset_aborts_before_deletion_when_autostart_disable_or_confirmation_fails() {
        for failure in ["disable", "query", "mismatch"] {
            let fixture = CommandFixture::new();
            fixture.create_unlocked_profile();
            {
                let mut startup = fixture.startup_fake.0.lock().unwrap();
                match failure {
                    "disable" => startup.fail_disable = true,
                    "query" => startup.fail_query = true,
                    "mismatch" => startup.ignore_disable = true,
                    _ => unreachable!(),
                }
            }
            assert_eq!(
                fixture
                    .reset_authenticated(PASSWORD, "RESET KEYNEST")
                    .unwrap_err()
                    .code,
                "startup-error"
            );
            assert!(fixture.temp.path().join("profile.json").is_file());
            assert!(fixture.temp.path().join("settings.json").is_file());
            assert_eq!(fixture.clipboard_port.clear_calls.load(Ordering::SeqCst), 0);
        }
    }

    #[test]
    fn clipboard_failure_aborts_before_settings_and_encrypted_deletion() {
        let fixture = CommandFixture::new();
        fixture.create_unlocked_profile();
        fixture
            .clipboard_port
            .fail_read
            .store(true, Ordering::SeqCst);
        assert_eq!(
            fixture
                .reset_authenticated(PASSWORD, "RESET KEYNEST")
                .unwrap_err()
                .code,
            "clipboard-error"
        );
        assert!(fixture.temp.path().join("profile.json").is_file());
        assert!(fixture.temp.path().join("settings.json").is_file());
        assert_eq!(fixture.auth.status(), AuthStatus::Unlocked);
    }

    #[test]
    fn settings_failure_leaves_encrypted_data_and_auth_intact_after_prior_cleanup() {
        let fixture = CommandFixture::new();
        fixture.create_unlocked_profile();
        fixture.store.fail_next_reset_for_test();
        assert_eq!(
            fixture
                .reset_authenticated(PASSWORD, "RESET KEYNEST")
                .unwrap_err()
                .code,
            "settings-error"
        );
        assert!(fixture.temp.path().join("profile.json").is_file());
        assert!(fixture.temp.path().join("settings.json").is_file());
        assert_eq!(fixture.auth.status(), AuthStatus::Unlocked);
        assert!(!fixture.startup_fake.0.lock().unwrap().enabled);
        assert_eq!(fixture.clipboard_port.clear_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn final_encrypted_deletion_failure_leaves_secure_defaults_and_retry_state_armed() {
        let fixture = CommandFixture::new();
        fixture.create_unlocked_profile();
        fs::create_dir(fixture.temp.path().join("vault.enc")).unwrap();
        assert_eq!(
            fixture
                .reset_authenticated(PASSWORD, "RESET KEYNEST")
                .unwrap_err()
                .code,
            "local-data-error"
        );
        assert!(!fixture.temp.path().join("settings.json").exists());
        assert_eq!(fixture.settings.snapshot(false).auto_lock_seconds, 300);
        assert_eq!(fixture.settings.snapshot(false).clipboard_clear_seconds, 30);
        assert_eq!(fixture.auth.status(), AuthStatus::Unlocked);
        assert!(fixture.auto_lock.is_armed_for_test());
        assert!(fixture.temp.path().join("profile.json").is_file());
    }

    #[test]
    fn successful_reset_orders_cleanup_preserves_unrelated_and_disarms_after_deletion() {
        let fixture = CommandFixture::new();
        fixture.create_unlocked_profile();
        fs::write(fixture.temp.path().join("vault.enc"), b"vault").unwrap();
        fs::write(fixture.temp.path().join("keep.txt"), b"keep").unwrap();
        assert_eq!(
            fixture
                .reset_authenticated(PASSWORD, "RESET KEYNEST")
                .unwrap(),
            AuthStatus::SetupRequired
        );
        assert!(!fixture.temp.path().join("profile.json").exists());
        assert!(!fixture.temp.path().join("vault.enc").exists());
        assert!(!fixture.temp.path().join("settings.json").exists());
        assert_eq!(
            fs::read(fixture.temp.path().join("keep.txt")).unwrap(),
            b"keep"
        );
        assert!(!fixture.startup_fake.0.lock().unwrap().enabled);
        assert_eq!(fixture.clipboard_port.clear_calls.load(Ordering::SeqCst), 1);
        assert!(!fixture.auto_lock.is_armed_for_test());
        let defaults = fixture.settings.snapshot(false);
        assert_eq!(
            (
                defaults.auto_lock_seconds,
                defaults.clipboard_clear_seconds,
                defaults.theme
            ),
            (300, 30, ThemePreference::System)
        );
    }

    #[test]
    fn authenticated_reset_is_atomic_against_all_contending_protected_operations() {
        let fixture = CommandFixture::new();
        fixture.create_unlocked_profile();
        fs::write(fixture.temp.path().join("keep.txt"), b"keep").unwrap();
        let validated = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));

        let auth = fixture.auth.clone();
        let startup = fixture.startup.clone();
        let clipboard = fixture.clipboard.clone();
        let settings = fixture.settings.clone();
        let auto_lock = fixture.auto_lock.clone();
        let operation_gate = fixture.operation_gate.clone();
        let validated_reset = validated.clone();
        let release_reset = release.clone();
        let reset = thread::spawn(move || {
            reset_authenticated_with_hook(
                PASSWORD,
                "RESET KEYNEST",
                &auth,
                &startup,
                &clipboard,
                &settings,
                &auto_lock,
                &operation_gate,
                || {
                    validated_reset.wait();
                    release_reset.wait();
                },
            )
        });
        validated.wait();

        let ready = Arc::new(Barrier::new(8));

        let auth = fixture.auth.clone();
        let settings = fixture.settings.clone();
        let startup = fixture.startup.clone();
        let operation_gate = fixture.operation_gate.clone();
        let ready_startup = ready.clone();
        let startup_toggle = thread::spawn(move || {
            ready_startup.wait();
            set_startup_value(true, &auth, &settings, &startup, &operation_gate)
        });

        let auth = fixture.auth.clone();
        let settings = fixture.settings.clone();
        let startup = fixture.startup.clone();
        let operation_gate = fixture.operation_gate.clone();
        let ready_settings = ready.clone();
        let setting = thread::spawn(move || {
            ready_settings.wait();
            set_theme_value("light", &auth, &settings, &startup, &operation_gate)
        });

        let auth = fixture.auth.clone();
        let settings = fixture.settings.clone();
        let auto_lock = fixture.auto_lock.clone();
        let startup = fixture.startup.clone();
        let operation_gate = fixture.operation_gate.clone();
        let ready_timeout = ready.clone();
        let timeout = thread::spawn(move || {
            ready_timeout.wait();
            set_auto_lock_value(
                1800,
                &auth,
                &settings,
                &auto_lock,
                &startup,
                &operation_gate,
            )
        });

        let auth = fixture.auth.clone();
        let auto_lock = fixture.auto_lock.clone();
        let operation_gate = fixture.operation_gate.clone();
        let ready_unlock = ready.clone();
        let unlock = thread::spawn(move || {
            ready_unlock.wait();
            unlock_and_arm(PASSWORD, &auth, &auto_lock, &operation_gate)
        });

        let auth = fixture.auth.clone();
        let operation_gate = fixture.operation_gate.clone();
        let ready_change = ready.clone();
        let password_change = thread::spawn(move || {
            ready_change.wait();
            change_master_password_value(
                PASSWORD,
                "a replacement secure password",
                &auth,
                &operation_gate,
            )
        });

        let auto_lock = fixture.auto_lock.clone();
        let ready_manual_lock = ready.clone();
        let manual_lock = thread::spawn(move || {
            ready_manual_lock.wait();
            auto_lock.lock_now()
        });

        let auto_lock = fixture.auto_lock.clone();
        let ready_resume_lock = ready.clone();
        let resume_lock = thread::spawn(move || {
            ready_resume_lock.wait();
            auto_lock.lock_now()
        });

        ready.wait();
        release.wait();

        assert_eq!(reset.join().unwrap().unwrap(), AuthStatus::SetupRequired);
        assert_eq!(
            startup_toggle.join().unwrap().unwrap_err().code,
            "unauthorized"
        );
        assert_eq!(setting.join().unwrap().unwrap_err().code, "unauthorized");
        assert_eq!(timeout.join().unwrap().unwrap_err().code, "unauthorized");
        assert_eq!(unlock.join().unwrap(), Err(AuthError::NotInitialized));
        assert_eq!(
            password_change.join().unwrap().unwrap_err().code,
            "not-initialized"
        );
        assert_eq!(
            manual_lock.join().unwrap().unwrap(),
            AuthStatus::SetupRequired
        );
        assert_eq!(
            resume_lock.join().unwrap().unwrap(),
            AuthStatus::SetupRequired
        );

        assert_eq!(fixture.auth.status(), AuthStatus::SetupRequired);
        assert!(!fixture.startup_fake.0.lock().unwrap().enabled);
        assert!(!fixture.temp.path().join("settings.json").exists());
        let defaults = fixture.settings.snapshot(false);
        assert_eq!(defaults.auto_lock_seconds, 300);
        assert_eq!(defaults.clipboard_clear_seconds, 30);
        assert_eq!(defaults.theme, ThemePreference::System);
        assert!(!fixture.auto_lock.is_armed_for_test());
        assert_eq!(
            fs::read(fixture.temp.path().join("keep.txt")).unwrap(),
            b"keep"
        );
    }

    #[test]
    fn create_master_password_linearizes_before_waiting_reset_and_is_fully_erased() {
        let fixture = CommandFixture::new();
        fixture.settings.set_theme_name("dark").unwrap();
        fixture.startup_fake.0.lock().unwrap().enabled = true;
        fs::write(fixture.temp.path().join("keep.txt"), b"keep").unwrap();
        let created = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));

        let auth = fixture.auth.clone();
        let auto_lock = fixture.auto_lock.clone();
        let operation_gate = fixture.operation_gate.clone();
        let created_call = created.clone();
        let release_call = release.clone();
        let create = thread::spawn(move || {
            create_master_password_and_arm_with_hook(
                PASSWORD,
                &auth,
                &auto_lock,
                &operation_gate,
                || {
                    created_call.wait();
                    release_call.wait();
                },
            )
        });
        created.wait();

        let auth = fixture.auth.clone();
        let startup = fixture.startup.clone();
        let clipboard = fixture.clipboard.clone();
        let settings = fixture.settings.clone();
        let auto_lock = fixture.auto_lock.clone();
        let operation_gate = fixture.operation_gate.clone();
        let (reset_started_tx, reset_started_rx) = mpsc::channel();
        let reset = thread::spawn(move || {
            reset_started_tx.send(()).unwrap();
            reset_authenticated(
                PASSWORD,
                "RESET KEYNEST",
                &auth,
                &startup,
                &clipboard,
                &settings,
                &auto_lock,
                &operation_gate,
            )
        });
        reset_started_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        release.wait();
        assert_eq!(create.join().unwrap(), Ok(AuthStatus::Unlocked));
        assert_eq!(reset.join().unwrap().unwrap(), AuthStatus::SetupRequired);
        assert_eq!(fixture.auth.status(), AuthStatus::SetupRequired);
        assert!(!fixture.temp.path().join("profile.json").exists());
        assert!(!fixture.temp.path().join("settings.json").exists());
        assert!(!fixture.startup_fake.0.lock().unwrap().enabled);
        assert!(!fixture.auto_lock.is_armed_for_test());
        assert_eq!(
            fs::read(fixture.temp.path().join("keep.txt")).unwrap(),
            b"keep"
        );
    }

    #[test]
    fn recovery_reset_rejects_unlocked_bypass_before_side_effects() {
        let fixture = CommandFixture::new();
        fixture.create_unlocked_profile();
        assert_eq!(
            reset_recovery(
                "RESET KEYNEST",
                &fixture.auth,
                &fixture.startup,
                &fixture.clipboard,
                &fixture.settings,
                &fixture.auto_lock,
                &fixture.operation_gate,
            )
            .unwrap_err()
            .code,
            "unauthorized"
        );
        assert!(fixture.startup_fake.0.lock().unwrap().enabled);
        assert_eq!(fixture.clipboard_port.clear_calls.load(Ordering::SeqCst), 0);
        assert!(fixture.temp.path().join("profile.json").is_file());
    }

    #[test]
    fn locked_recovery_invalid_phrase_has_no_side_effects() {
        let fixture = CommandFixture::new();
        fixture.create_unlocked_profile();
        fixture.auto_lock.lock_now().unwrap();
        fixture.startup_fake.0.lock().unwrap().enabled = true;
        let clears_before_reset = fixture.clipboard_port.clear_calls.load(Ordering::SeqCst);

        assert_eq!(
            reset_recovery(
                "RESET",
                &fixture.auth,
                &fixture.startup,
                &fixture.clipboard,
                &fixture.settings,
                &fixture.auto_lock,
                &fixture.operation_gate,
            )
            .unwrap_err()
            .code,
            "invalid-reset-confirmation"
        );
        assert!(fixture.startup_fake.0.lock().unwrap().enabled);
        assert_eq!(
            fixture.clipboard_port.clear_calls.load(Ordering::SeqCst),
            clears_before_reset
        );
        assert!(fixture.temp.path().join("profile.json").is_file());
        assert!(fixture.temp.path().join("settings.json").is_file());
        assert_eq!(fixture.auth.status(), AuthStatus::Locked);
    }

    #[test]
    fn recovery_reset_state_check_and_deletion_are_atomic_against_unlock() {
        let fixture = CommandFixture::new();
        fixture.create_unlocked_profile();
        fixture.auto_lock.lock_now().unwrap();
        fixture.startup_fake.0.lock().unwrap().enabled = true;
        let validated = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));

        let auth = fixture.auth.clone();
        let startup = fixture.startup.clone();
        let clipboard = fixture.clipboard.clone();
        let settings = fixture.settings.clone();
        let auto_lock = fixture.auto_lock.clone();
        let operation_gate = fixture.operation_gate.clone();
        let validated_reset = validated.clone();
        let release_reset = release.clone();
        let reset = thread::spawn(move || {
            reset_recovery_with_hook(
                "RESET KEYNEST",
                &auth,
                &startup,
                &clipboard,
                &settings,
                &auto_lock,
                &operation_gate,
                || {
                    validated_reset.wait();
                    release_reset.wait();
                },
            )
        });
        validated.wait();

        let auth = fixture.auth.clone();
        let auto_lock = fixture.auto_lock.clone();
        let operation_gate = fixture.operation_gate.clone();
        let (unlock_started_tx, unlock_started_rx) = mpsc::channel();
        let unlock = thread::spawn(move || {
            unlock_started_tx.send(()).unwrap();
            unlock_and_arm(PASSWORD, &auth, &auto_lock, &operation_gate)
        });
        unlock_started_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        release.wait();
        assert_eq!(reset.join().unwrap().unwrap(), AuthStatus::SetupRequired);
        assert_eq!(unlock.join().unwrap(), Err(AuthError::NotInitialized));
        assert_eq!(fixture.auth.status(), AuthStatus::SetupRequired);
        assert!(!fixture.temp.path().join("profile.json").exists());
        assert!(!fixture.temp.path().join("settings.json").exists());
        assert!(!fixture.auto_lock.is_armed_for_test());
    }

    #[test]
    fn locked_recovery_reset_uses_the_same_cleanup_order_and_succeeds() {
        let fixture = CommandFixture::new();
        fixture.create_unlocked_profile();
        fixture.auto_lock.lock_now().unwrap();
        fixture.startup_fake.0.lock().unwrap().enabled = true;
        assert_eq!(
            reset_recovery(
                "RESET KEYNEST",
                &fixture.auth,
                &fixture.startup,
                &fixture.clipboard,
                &fixture.settings,
                &fixture.auto_lock,
                &fixture.operation_gate,
            )
            .unwrap(),
            AuthStatus::SetupRequired
        );
        assert!(!fixture.temp.path().join("profile.json").exists());
        assert!(!fixture.temp.path().join("settings.json").exists());
        assert!(!fixture.startup_fake.0.lock().unwrap().enabled);
    }

    #[test]
    fn create_unlock_activity_and_manual_lock_behavior_is_preserved() {
        let fixture = CommandFixture::new();
        assert_eq!(
            create_master_password_and_arm(
                "short",
                &fixture.auth,
                &fixture.auto_lock,
                &fixture.operation_gate,
            ),
            Err(AuthError::PasswordTooShort)
        );
        assert!(!fixture.auto_lock.is_armed_for_test());
        assert_eq!(
            create_master_password_and_arm(
                PASSWORD,
                &fixture.auth,
                &fixture.auto_lock,
                &fixture.operation_gate,
            ),
            Ok(AuthStatus::Unlocked)
        );
        let old = Instant::now() - Duration::from_secs(30);
        fixture.auto_lock.arm_at_for_test(old);
        record_activity_if_unlocked(&fixture.auth, &fixture.auto_lock, &fixture.operation_gate)
            .unwrap();
        assert!(!fixture
            .auto_lock
            .expire_at_for_test(old + Duration::from_secs(300)));
        assert_eq!(fixture.auto_lock.lock_now().unwrap(), AuthStatus::Locked);
        assert_eq!(
            record_activity_if_unlocked(&fixture.auth, &fixture.auto_lock, &fixture.operation_gate,),
            Err(AuthError::Unauthorized)
        );
        assert_eq!(
            unlock_and_arm(
                PASSWORD,
                &fixture.auth,
                &fixture.auto_lock,
                &fixture.operation_gate,
            ),
            Ok(AuthStatus::Unlocked)
        );
    }

    #[test]
    fn activity_dispatch_returns_pending_promptly_then_records_after_gate_release() {
        let fixture = CommandFixture::new();
        create_master_password_and_arm(
            PASSWORD,
            &fixture.auth,
            &fixture.auto_lock,
            &fixture.operation_gate,
        )
        .unwrap();
        let old_activity = Instant::now() - Duration::from_secs(30);
        fixture.auto_lock.arm_at_for_test(old_activity);
        let guard = fixture.operation_gate.lock();

        let dispatched = dispatch_record_activity(
            fixture.auth.clone(),
            fixture.auto_lock.clone(),
            fixture.operation_gate.clone(),
        );

        assert!(!dispatched.inner().is_finished());
        drop(guard);
        tauri::async_runtime::block_on(dispatched).unwrap().unwrap();
        assert!(!fixture
            .auto_lock
            .expire_at_for_test(old_activity + Duration::from_secs(300)));
    }

    #[test]
    fn activity_dispatch_rechecks_authorization_after_waiting_for_gate() {
        let fixture = CommandFixture::new();
        create_master_password_and_arm(
            PASSWORD,
            &fixture.auth,
            &fixture.auto_lock,
            &fixture.operation_gate,
        )
        .unwrap();
        let guard = fixture.operation_gate.lock();
        let dispatched = dispatch_record_activity(
            fixture.auth.clone(),
            fixture.auto_lock.clone(),
            fixture.operation_gate.clone(),
        );
        assert!(!dispatched.inner().is_finished());

        assert_eq!(
            fixture
                .auto_lock
                .lock_now_with_operation_guard(&guard)
                .unwrap(),
            AuthStatus::Locked
        );
        drop(guard);

        let error = tauri::async_runtime::block_on(dispatched)
            .unwrap()
            .unwrap_err();
        assert_eq!(error.code, "unauthorized");
        assert_eq!(fixture.auth.status(), AuthStatus::Locked);
    }
}
