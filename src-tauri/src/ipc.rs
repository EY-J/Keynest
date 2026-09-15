use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tauri::State;
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::{
    autofill::{
        AutofillService, HostApprovalCandidate, HostApprovalCompleted, HostApprovalError,
        PendingHostApprovalView,
    },
    notes::{NoteInput, NoteRecord, NotesError, NotesService},
    platform::startup::{StartupError, StartupService},
    profile::{AvatarUpdate, ProfileError, ProfileService, ProfileSnapshot},
    recently_deleted::{DeletedItem, DeletedItemType},
    security::{
        AuthError, AuthService, AuthStatus, AutoLockService, ClipboardError, ClipboardService,
        LockError, PinStatus, RecoveryStatus, SecurityOperationGate,
    },
    settings::{SettingsError, SettingsService, SettingsSnapshot},
    vault::{VaultError, VaultRecord, VaultRecordInput, VaultRecordSummary, VaultService},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublicIpcError {
    pub(crate) code: &'static str,
    pub(crate) message: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) retry_after_ms: Option<u64>,
}

#[derive(PartialEq, Eq, Serialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecoveryKeyResult {
    #[zeroize(skip)]
    pub(crate) status: AuthStatus,
    pub(crate) recovery_key: String,
}

impl std::fmt::Debug for RecoveryKeyResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecoveryKeyResult")
            .field("status", &self.status)
            .field("recovery_key", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod secret_response_tests {
    use super::*;

    #[test]
    fn recovery_response_is_redacted_and_zeroizes_without_changing_wire_contract() {
        fn assert_wipes_on_drop<T: Zeroize + ZeroizeOnDrop>() {}
        assert_wipes_on_drop::<RecoveryKeyResult>();
        let mut result = RecoveryKeyResult {
            status: AuthStatus::Unlocked,
            recovery_key: "fixture-only-secret".into(),
        };
        assert!(!format!("{result:?}").contains("fixture-only-secret"));
        assert_eq!(
            serde_json::to_value(&result).unwrap()["recoveryKey"],
            "fixture-only-secret"
        );
        result.zeroize();
        assert!(result.recovery_key.is_empty());
    }
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

    fn clipboard_copy() -> Self {
        Self::new(
            "clipboard-copy-error",
            "KeyNest could not copy this password.",
        )
    }

    fn username_clipboard_copy() -> Self {
        Self::new(
            "clipboard-copy-error",
            "KeyNest could not copy this username.",
        )
    }

    fn recovery_clipboard_copy() -> Self {
        Self::new(
            "clipboard-copy-error",
            "KeyNest could not copy the Recovery Key.",
        )
    }
}

impl From<HostApprovalError> for PublicIpcError {
    fn from(error: HostApprovalError) -> Self {
        match error {
            HostApprovalError::Locked => Self::new("unauthorized", "KeyNest is locked."),
            HostApprovalError::Unavailable => Self::new(
                "host-approval-unavailable",
                "This login-host approval request is no longer available.",
            ),
            HostApprovalError::CredentialNotFound => Self::new(
                "vault-record-not-found",
                "This credential no longer exists.",
            ),
            HostApprovalError::Internal => Self::internal(),
        }
    }
}

impl From<AuthError> for PublicIpcError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::PasswordTooShort => {
                Self::new("password-too-short", "Use at least 12 characters.")
            }
            AuthError::PasswordTooWeak => {
                Self::new("password-too-weak", "Master Password is too weak.")
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
            AuthError::RecoveryNotConfigured => Self::new(
                "recovery-not-configured",
                "This KeyNest profile does not have a Recovery Key yet.",
            ),
            AuthError::InvalidRecoveryKey => {
                Self::new("invalid-recovery-key", "The Recovery Key is incorrect.")
            }
            AuthError::Throttled { retry_after_ms } => Self {
                code: "throttled",
                message: "Wait a moment before trying again.",
                retry_after_ms: Some(retry_after_ms),
            },
            AuthError::InvalidPinFormat => Self::new("invalid-pin-format", "Enter a 6-digit PIN."),
            AuthError::PinConfirmationMismatch => {
                Self::new("pin-confirmation-mismatch", "The PINs do not match.")
            }
            AuthError::InvalidPin => Self::new("invalid-pin", "The device PIN is incorrect."),
            AuthError::PinNotConfigured => {
                Self::new("pin-not-configured", "Device PIN unlock is not configured.")
            }
            AuthError::PinAlreadyConfigured => Self::new(
                "pin-already-configured",
                "Device PIN unlock is already configured.",
            ),
            AuthError::PinThrottled { retry_after_ms } => Self {
                code: "pin-throttled",
                message: "Wait a moment before trying the device PIN again.",
                retry_after_ms: Some(retry_after_ms),
            },
            AuthError::PinRequiresMasterPassword => Self::new(
                "pin-requires-master-password",
                "Too many incorrect PIN attempts. Use your Master Password to unlock KeyNest.",
            ),
            AuthError::DeviceProtectionUnavailable => Self::new(
                "device-protection-unavailable",
                "Windows could not protect the device PIN on this account.",
            ),
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

impl From<ProfileError> for PublicIpcError {
    fn from(error: ProfileError) -> Self {
        match error {
            ProfileError::InvalidDisplayName => {
                Self::new("invalid-display-name", "Enter a display name.")
            }
            ProfileError::DisplayNameTooLong => Self::new(
                "display-name-too-long",
                "Display name must be 50 characters or fewer.",
            ),
            ProfileError::ImageTooLarge => Self::new(
                "profile-image-too-large",
                "Choose an image no larger than 5 MB.",
            ),
            ProfileError::UnsupportedImage | ProfileError::ImageTypeMismatch => Self::new(
                "unsupported-profile-image",
                "Choose a PNG, JPG, or WEBP image.",
            ),
            ProfileError::InvalidImageDimensions => Self::new(
                "invalid-profile-image-dimensions",
                "Choose an image no larger than 4096 pixels on either side.",
            ),
            ProfileError::InvalidImage => Self::new(
                "invalid-profile-image",
                "KeyNest could not read that image.",
            ),
            ProfileError::Serialization(_) | ProfileError::Storage(_) => Self::new(
                "profile-storage-error",
                "KeyNest could not save the local profile.",
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

impl From<VaultError> for PublicIpcError {
    fn from(error: VaultError) -> Self {
        match error {
            VaultError::InvalidName => Self::new("invalid-vault-name", "Enter a credential name."),
            VaultError::InvalidUsername => {
                Self::new("invalid-vault-username", "Enter a credential username.")
            }
            VaultError::InvalidPassword => {
                Self::new("invalid-vault-password", "Enter a credential password.")
            }
            VaultError::InvalidWebsite => {
                Self::new("invalid-vault-website", "Enter a valid credential website.")
            }
            VaultError::InvalidAllowedLoginHosts => Self::new(
                "invalid-vault-login-hosts",
                "Enter valid login hostnames without paths or wildcards.",
            ),
            VaultError::InvalidTags => {
                Self::new("invalid-vault-tags", "Check the credential tags.")
            }
            VaultError::NotFound => {
                Self::new("vault-record-not-found", "The credential was not found.")
            }
            VaultError::DataDamaged => Self::new(
                "vault-data-error",
                "KeyNest's encrypted vault data is damaged or unsupported.",
            ),
            VaultError::EntropyUnavailable => Self::new(
                "vault-entropy-error",
                "KeyNest could not generate secure vault data.",
            ),
            VaultError::StorageUnavailable => Self::new(
                "vault-storage-error",
                "KeyNest could not access its encrypted vault data.",
            ),
        }
    }
}

impl From<NotesError> for PublicIpcError {
    fn from(error: NotesError) -> Self {
        match error {
            NotesError::InvalidTitle => Self::new("invalid-note-title", "Enter a note title."),
            NotesError::InvalidContent => {
                Self::new("invalid-note-content", "This note is too long.")
            }
            NotesError::InvalidTags => Self::new("invalid-note-tags", "Check the note tags."),
            NotesError::NotFound => Self::new("note-not-found", "The note was not found."),
            NotesError::DataDamaged => Self::new(
                "notes-data-error",
                "KeyNest's encrypted notes data is damaged or unsupported.",
            ),
            NotesError::EntropyUnavailable => Self::new(
                "notes-entropy-error",
                "KeyNest could not generate secure notes data.",
            ),
            NotesError::StorageUnavailable => Self::new(
                "notes-storage-error",
                "KeyNest could not access its encrypted notes data.",
            ),
        }
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

#[cfg(test)]
pub(crate) fn create_master_password_and_arm(
    password: &str,
    auth: &AuthService,
    auto_lock: &AutoLockService,
    operation_gate: &SecurityOperationGate,
) -> Result<AuthStatus, AuthError> {
    create_master_password_and_arm_with_hook(password, auth, auto_lock, operation_gate, || {})
}

#[cfg(test)]
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

fn create_master_password_for_recovery(
    password: &str,
    auth: &AuthService,
    operation_gate: &SecurityOperationGate,
) -> Result<RecoveryKeyResult, PublicIpcError> {
    let _guard = operation_gate.lock();
    let recovery_key = auth.create_master_password(password)?;
    Ok(RecoveryKeyResult {
        status: auth.status(),
        recovery_key: recovery_key.to_string(),
    })
}

fn finish_recovery_key_display(
    auth: &AuthService,
    auto_lock: &AutoLockService,
    operation_gate: &SecurityOperationGate,
) -> Result<AuthStatus, PublicIpcError> {
    let _guard = operation_gate.lock();
    require_unlocked(auth)?;
    auto_lock.arm();
    Ok(auth.status())
}

fn recovery_status_value(
    auth: &AuthService,
    operation_gate: &SecurityOperationGate,
) -> Result<RecoveryStatus, PublicIpcError> {
    let _guard = operation_gate.lock();
    auth.recovery_status().map_err(Into::into)
}

fn pin_status_value(
    auth: &AuthService,
    operation_gate: &SecurityOperationGate,
) -> Result<PinStatus, PublicIpcError> {
    let _guard = operation_gate.lock();
    auth.pin_status().map_err(Into::into)
}

fn recover_master_password_value(
    recovery_key: &str,
    new_password: &str,
    auth: &AuthService,
    operation_gate: &SecurityOperationGate,
) -> Result<RecoveryKeyResult, PublicIpcError> {
    let _guard = operation_gate.lock();
    let replacement_key = auth.recover_master_password(recovery_key, new_password)?;
    Ok(RecoveryKeyResult {
        status: auth.status(),
        recovery_key: replacement_key.to_string(),
    })
}

fn regenerate_recovery_key_value(
    current_password: &str,
    auth: &AuthService,
    auto_lock: &AutoLockService,
    operation_gate: &SecurityOperationGate,
) -> Result<RecoveryKeyResult, PublicIpcError> {
    let _guard = operation_gate.lock();
    let recovery_key = auth.regenerate_recovery_key(current_password)?;
    // The new key exists nowhere else, so pause automatic locking until the UI confirms
    // the user has had an opportunity to save it.
    auto_lock.disarm();
    Ok(RecoveryKeyResult {
        status: auth.status(),
        recovery_key: recovery_key.to_string(),
    })
}

fn copy_recovery_key_value(
    recovery_key: &str,
    auth: &AuthService,
    clipboard: &ClipboardService,
    operation_gate: &SecurityOperationGate,
) -> Result<(), PublicIpcError> {
    let _guard = operation_gate.lock();
    require_unlocked(auth)?;
    clipboard
        .copy_secret(recovery_key)
        .map_err(|_| PublicIpcError::recovery_clipboard_copy())
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

pub(crate) fn unlock_with_pin_and_arm(
    pin: &str,
    auth: &AuthService,
    auto_lock: &AutoLockService,
    operation_gate: &SecurityOperationGate,
) -> Result<AuthStatus, AuthError> {
    let _guard = operation_gate.lock();
    auth.unlock_with_pin(pin)?;
    auto_lock.arm();
    Ok(auth.status())
}

fn setup_pin_value(
    current_password: &str,
    pin: &str,
    confirmation: &str,
    auth: &AuthService,
    operation_gate: &SecurityOperationGate,
) -> Result<PinStatus, PublicIpcError> {
    let _guard = operation_gate.lock();
    auth.setup_pin(current_password, pin, confirmation)?;
    auth.pin_status().map_err(Into::into)
}

fn change_pin_value(
    current_password: &str,
    pin: &str,
    confirmation: &str,
    auth: &AuthService,
    operation_gate: &SecurityOperationGate,
) -> Result<PinStatus, PublicIpcError> {
    let _guard = operation_gate.lock();
    auth.change_pin(current_password, pin, confirmation)?;
    auth.pin_status().map_err(Into::into)
}

fn remove_pin_value(
    current_password: &str,
    auth: &AuthService,
    operation_gate: &SecurityOperationGate,
) -> Result<PinStatus, PublicIpcError> {
    let _guard = operation_gate.lock();
    auth.remove_pin(current_password)?;
    auth.pin_status().map_err(Into::into)
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

fn set_lock_on_sleep_value(
    enabled: bool,
    auth: &AuthService,
    settings: &SettingsService,
    startup: &StartupService,
    operation_gate: &SecurityOperationGate,
) -> Result<SettingsSnapshot, PublicIpcError> {
    let _guard = operation_gate.lock();
    require_unlocked(auth)?;
    settings.set_lock_on_sleep(enabled)?;
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

fn with_vault_key<T>(
    auth: &AuthService,
    operation_gate: &SecurityOperationGate,
    operation: impl FnOnce(&[u8; 32]) -> Result<T, VaultError>,
) -> Result<T, PublicIpcError> {
    let _guard = operation_gate.lock();
    auth.require_vault_key(operation)
        .map_err(PublicIpcError::from)?
        .map_err(PublicIpcError::from)
}

fn list_vault_records_value(
    auth: &AuthService,
    vault: &VaultService,
    operation_gate: &SecurityOperationGate,
) -> Result<Vec<VaultRecordSummary>, PublicIpcError> {
    with_vault_key(auth, operation_gate, |vault_key| vault.list(vault_key))
}

fn create_vault_record_value(
    input: VaultRecordInput,
    auth: &AuthService,
    vault: &VaultService,
    operation_gate: &SecurityOperationGate,
) -> Result<VaultRecordSummary, PublicIpcError> {
    with_vault_key(auth, operation_gate, |vault_key| {
        vault.create(vault_key, input)
    })
}

fn get_vault_record_value(
    id: &str,
    auth: &AuthService,
    vault: &VaultService,
    operation_gate: &SecurityOperationGate,
) -> Result<VaultRecord, PublicIpcError> {
    with_vault_key(auth, operation_gate, |vault_key| vault.get(vault_key, id))
}

fn get_vault_record_summary_value(
    id: &str,
    auth: &AuthService,
    vault: &VaultService,
    operation_gate: &SecurityOperationGate,
) -> Result<VaultRecordSummary, PublicIpcError> {
    with_vault_key(auth, operation_gate, |key| {
        // Decryption stays here; the temporary full DTO wipes itself on drop.
        vault
            .get(key, id)
            .map(|record| VaultRecordSummary::from(&record))
    })
}

fn update_vault_record_value(
    id: &str,
    input: VaultRecordInput,
    auth: &AuthService,
    vault: &VaultService,
    operation_gate: &SecurityOperationGate,
) -> Result<VaultRecordSummary, PublicIpcError> {
    with_vault_key(auth, operation_gate, |vault_key| {
        vault.update(vault_key, id, input)
    })
}

fn delete_vault_record_value(
    id: &str,
    auth: &AuthService,
    vault: &VaultService,
    operation_gate: &SecurityOperationGate,
) -> Result<(), PublicIpcError> {
    with_vault_key(auth, operation_gate, |vault_key| {
        vault.delete(vault_key, id)
    })
}

fn with_notes_key<T>(
    auth: &AuthService,
    operation_gate: &SecurityOperationGate,
    operation: impl FnOnce(&[u8; 32]) -> Result<T, NotesError>,
) -> Result<T, PublicIpcError> {
    let _guard = operation_gate.lock();
    auth.require_vault_key(operation)
        .map_err(PublicIpcError::from)?
        .map_err(PublicIpcError::from)
}

fn list_notes_value(
    auth: &AuthService,
    notes: &NotesService,
    operation_gate: &SecurityOperationGate,
) -> Result<Vec<NoteRecord>, PublicIpcError> {
    with_notes_key(auth, operation_gate, |key| notes.list(key))
}

fn create_note_value(
    input: NoteInput,
    auth: &AuthService,
    notes: &NotesService,
    operation_gate: &SecurityOperationGate,
) -> Result<NoteRecord, PublicIpcError> {
    with_notes_key(auth, operation_gate, |key| notes.create(key, input))
}

fn update_note_value(
    id: &str,
    input: NoteInput,
    auth: &AuthService,
    notes: &NotesService,
    operation_gate: &SecurityOperationGate,
) -> Result<NoteRecord, PublicIpcError> {
    with_notes_key(auth, operation_gate, |key| notes.update(key, id, input))
}

fn delete_note_value(
    id: &str,
    auth: &AuthService,
    notes: &NotesService,
    operation_gate: &SecurityOperationGate,
) -> Result<(), PublicIpcError> {
    with_notes_key(auth, operation_gate, |key| notes.delete(key, id))
}

const DELETED_ITEM_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1_000;

fn deleted_item_cutoff_ms() -> Result<i64, PublicIpcError> {
    let now: i64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| PublicIpcError::internal())?
        .as_millis()
        .try_into()
        .map_err(|_| PublicIpcError::internal())?;
    now.checked_sub(DELETED_ITEM_RETENTION_MS)
        .ok_or_else(PublicIpcError::internal)
}

fn list_deleted_items_value(
    auth: &AuthService,
    vault: &VaultService,
    notes: &NotesService,
    operation_gate: &SecurityOperationGate,
) -> Result<Vec<DeletedItem>, PublicIpcError> {
    let _guard = operation_gate.lock();
    auth.require_vault_key(|key| {
        let cutoff = deleted_item_cutoff_ms()?;
        vault
            .purge_deleted_before(key, cutoff)
            .map_err(PublicIpcError::from)?;
        notes
            .purge_deleted_before(key, cutoff)
            .map_err(PublicIpcError::from)?;
        let mut items = vault.list_deleted(key).map_err(PublicIpcError::from)?;
        items.extend(notes.list_deleted(key).map_err(PublicIpcError::from)?);
        items.sort_by(|left, right| {
            right
                .deleted_at_ms
                .cmp(&left.deleted_at_ms)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(items)
    })
    .map_err(PublicIpcError::from)?
}

fn restore_deleted_item_value(
    id: &str,
    item_type: DeletedItemType,
    auth: &AuthService,
    vault: &VaultService,
    notes: &NotesService,
    operation_gate: &SecurityOperationGate,
) -> Result<(), PublicIpcError> {
    let _guard = operation_gate.lock();
    auth.require_vault_key(|key| match item_type {
        DeletedItemType::Credential => vault.restore(key, id).map_err(PublicIpcError::from),
        DeletedItemType::Note => notes.restore(key, id).map_err(PublicIpcError::from),
    })
    .map_err(PublicIpcError::from)?
}

fn permanently_delete_item_value(
    id: &str,
    item_type: DeletedItemType,
    auth: &AuthService,
    vault: &VaultService,
    notes: &NotesService,
    operation_gate: &SecurityOperationGate,
) -> Result<(), PublicIpcError> {
    let _guard = operation_gate.lock();
    auth.require_vault_key(|key| match item_type {
        DeletedItemType::Credential => vault
            .permanently_delete(key, id)
            .map_err(PublicIpcError::from),
        DeletedItemType::Note => notes
            .permanently_delete(key, id)
            .map_err(PublicIpcError::from),
    })
    .map_err(PublicIpcError::from)?
}

fn empty_recently_deleted_value(
    auth: &AuthService,
    vault: &VaultService,
    notes: &NotesService,
    operation_gate: &SecurityOperationGate,
) -> Result<(), PublicIpcError> {
    let _guard = operation_gate.lock();
    auth.require_vault_key(|key| {
        for item in vault.list_deleted(key).map_err(PublicIpcError::from)? {
            vault
                .permanently_delete(key, &item.id)
                .map_err(PublicIpcError::from)?;
        }
        for item in notes.list_deleted(key).map_err(PublicIpcError::from)? {
            notes
                .permanently_delete(key, &item.id)
                .map_err(PublicIpcError::from)?;
        }
        Ok(())
    })
    .map_err(PublicIpcError::from)?
}

fn copy_vault_password_value(
    id: &str,
    auth: &AuthService,
    vault: &VaultService,
    clipboard: &ClipboardService,
    operation_gate: &SecurityOperationGate,
) -> Result<(), PublicIpcError> {
    let _guard = operation_gate.lock();
    let password = auth
        .require_vault_key(|vault_key| vault.password_for_copy(vault_key, id))
        .map_err(PublicIpcError::from)?
        .map_err(PublicIpcError::from)?;
    clipboard
        .copy_secret(&password)
        .map_err(|_| PublicIpcError::clipboard_copy())?;
    Ok(())
}

fn copy_vault_username_value(
    id: &str,
    auth: &AuthService,
    vault: &VaultService,
    clipboard: &ClipboardService,
    operation_gate: &SecurityOperationGate,
) -> Result<(), PublicIpcError> {
    let _guard = operation_gate.lock();
    let record = auth
        .require_vault_key(|vault_key| vault.get(vault_key, id))
        .map_err(PublicIpcError::from)?
        .map_err(PublicIpcError::from)?;
    clipboard
        .copy_secret(&record.username)
        .map_err(|_| PublicIpcError::username_clipboard_copy())?;
    Ok(())
}

fn disable_startup_for_reset(startup: &StartupService) -> Result<(), PublicIpcError> {
    if startup.set_enabled(false)? {
        return Err(StartupError::StateMismatch.into());
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "the reset transaction requires each security service explicitly"
)]
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

#[expect(
    clippy::too_many_arguments,
    reason = "the reset transaction and its test hook require explicit dependencies"
)]
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

#[expect(
    clippy::too_many_arguments,
    reason = "the reset transaction and its test hook require explicit dependencies"
)]
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
    password: String,
    auth: State<'_, AuthService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<RecoveryKeyResult, PublicIpcError> {
    let mut password = Zeroizing::new(password);
    let auth = auth.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = create_master_password_for_recovery(&password, &auth, &operation_gate);
        password.zeroize();
        result
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command]
pub(crate) async fn complete_recovery_key_display(
    auth: State<'_, AuthService>,
    auto_lock: State<'_, AutoLockService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<AuthStatus, PublicIpcError> {
    let auth = auth.inner().clone();
    let auto_lock = auto_lock.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        finish_recovery_key_display(&auth, &auto_lock, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command]
pub(crate) async fn get_recovery_status(
    auth: State<'_, AuthService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<RecoveryStatus, PublicIpcError> {
    let auth = auth.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || recovery_status_value(&auth, &operation_gate))
        .await
        .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command]
pub(crate) async fn get_pin_status(
    auth: State<'_, AuthService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<PinStatus, PublicIpcError> {
    let auth = auth.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || pin_status_value(&auth, &operation_gate))
        .await
        .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn recover_master_password(
    recovery_key: String,
    new_password: String,
    auth: State<'_, AuthService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<RecoveryKeyResult, PublicIpcError> {
    let mut recovery_key = Zeroizing::new(recovery_key);
    let mut new_password = Zeroizing::new(new_password);
    let auth = auth.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result =
            recover_master_password_value(&recovery_key, &new_password, &auth, &operation_gate);
        recovery_key.zeroize();
        new_password.zeroize();
        result
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn regenerate_recovery_key(
    current_password: String,
    auth: State<'_, AuthService>,
    auto_lock: State<'_, AutoLockService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<RecoveryKeyResult, PublicIpcError> {
    let mut current_password = Zeroizing::new(current_password);
    let auth = auth.inner().clone();
    let auto_lock = auto_lock.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result =
            regenerate_recovery_key_value(&current_password, &auth, &auto_lock, &operation_gate);
        current_password.zeroize();
        result
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn copy_recovery_key(
    recovery_key: String,
    auth: State<'_, AuthService>,
    clipboard: State<'_, ClipboardService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<(), PublicIpcError> {
    let mut recovery_key = Zeroizing::new(recovery_key);
    let auth = auth.inner().clone();
    let clipboard = clipboard.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = copy_recovery_key_value(&recovery_key, &auth, &clipboard, &operation_gate);
        recovery_key.zeroize();
        result
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn unlock(
    password: String,
    auth: State<'_, AuthService>,
    auto_lock: State<'_, AutoLockService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<AuthStatus, PublicIpcError> {
    let mut password = Zeroizing::new(password);
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

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn unlock_with_pin(
    pin: String,
    auth: State<'_, AuthService>,
    auto_lock: State<'_, AutoLockService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<AuthStatus, PublicIpcError> {
    let mut pin = Zeroizing::new(pin);
    let auth = auth.inner().clone();
    let auto_lock = auto_lock.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = unlock_with_pin_and_arm(&pin, &auth, &auto_lock, &operation_gate);
        pin.zeroize();
        result.map_err(Into::into)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn setup_pin(
    current_password: String,
    pin: String,
    confirmation: String,
    auth: State<'_, AuthService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<PinStatus, PublicIpcError> {
    pin_mutation_command(
        current_password,
        pin,
        confirmation,
        auth,
        operation_gate,
        false,
    )
    .await
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn change_pin(
    current_password: String,
    pin: String,
    confirmation: String,
    auth: State<'_, AuthService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<PinStatus, PublicIpcError> {
    pin_mutation_command(
        current_password,
        pin,
        confirmation,
        auth,
        operation_gate,
        true,
    )
    .await
}

async fn pin_mutation_command(
    current_password: String,
    pin: String,
    confirmation: String,
    auth: State<'_, AuthService>,
    operation_gate: State<'_, SecurityOperationGate>,
    change: bool,
) -> Result<PinStatus, PublicIpcError> {
    let mut current_password = Zeroizing::new(current_password);
    let mut pin = Zeroizing::new(pin);
    let mut confirmation = Zeroizing::new(confirmation);
    let auth = auth.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = if change {
            change_pin_value(
                &current_password,
                &pin,
                &confirmation,
                &auth,
                &operation_gate,
            )
        } else {
            setup_pin_value(
                &current_password,
                &pin,
                &confirmation,
                &auth,
                &operation_gate,
            )
        };
        current_password.zeroize();
        pin.zeroize();
        confirmation.zeroize();
        result
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn remove_pin(
    current_password: String,
    auth: State<'_, AuthService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<PinStatus, PublicIpcError> {
    let mut current_password = Zeroizing::new(current_password);
    let auth = auth.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = remove_pin_value(&current_password, &auth, &operation_gate);
        current_password.zeroize();
        result
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
pub(crate) async fn get_profile(
    profile: State<'_, ProfileService>,
) -> Result<ProfileSnapshot, PublicIpcError> {
    let profile = profile.inner().clone();
    tauri::async_runtime::spawn_blocking(move || profile.snapshot().map_err(Into::into))
        .await
        .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn save_profile(
    display_name: String,
    avatar_update: AvatarUpdate,
    profile: State<'_, ProfileService>,
) -> Result<ProfileSnapshot, PublicIpcError> {
    let profile = profile.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        profile
            .save(display_name, avatar_update)
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
pub(crate) async fn set_lock_on_sleep(
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
        set_lock_on_sleep_value(enabled, &auth, &settings, &startup, &operation_gate)
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
    current_password: String,
    new_password: String,
    auth: State<'_, AuthService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<AuthStatus, PublicIpcError> {
    let mut current_password = Zeroizing::new(current_password);
    let mut new_password = Zeroizing::new(new_password);
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
#[expect(
    clippy::too_many_arguments,
    reason = "Tauri injects each managed security service as a command argument"
)]
pub(crate) async fn reset_keynest_authenticated(
    current_password: String,
    mut confirmation: String,
    auth: State<'_, AuthService>,
    startup: State<'_, StartupService>,
    clipboard: State<'_, ClipboardService>,
    settings: State<'_, SettingsService>,
    auto_lock: State<'_, AutoLockService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<AuthStatus, PublicIpcError> {
    let mut current_password = Zeroizing::new(current_password);
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

#[tauri::command]
pub(crate) async fn list_vault_records(
    auth: State<'_, AuthService>,
    vault: State<'_, VaultService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<Vec<VaultRecordSummary>, PublicIpcError> {
    let auth = auth.inner().clone();
    let vault = vault.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        list_vault_records_value(&auth, &vault, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn create_vault_record(
    input: VaultRecordInput,
    auth: State<'_, AuthService>,
    vault: State<'_, VaultService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<VaultRecordSummary, PublicIpcError> {
    let auth = auth.inner().clone();
    let vault = vault.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        create_vault_record_value(input, &auth, &vault, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn get_vault_record_summary(
    id: String,
    auth: State<'_, AuthService>,
    vault: State<'_, VaultService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<VaultRecordSummary, PublicIpcError> {
    let auth = auth.inner().clone();
    let vault = vault.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        get_vault_record_summary_value(&id, &auth, &vault, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn get_vault_record(
    id: String,
    auth: State<'_, AuthService>,
    vault: State<'_, VaultService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<VaultRecord, PublicIpcError> {
    let auth = auth.inner().clone();
    let vault = vault.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        get_vault_record_value(&id, &auth, &vault, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn update_vault_record(
    id: String,
    input: VaultRecordInput,
    auth: State<'_, AuthService>,
    vault: State<'_, VaultService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<VaultRecordSummary, PublicIpcError> {
    let auth = auth.inner().clone();
    let vault = vault.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        update_vault_record_value(&id, input, &auth, &vault, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn delete_vault_record(
    id: String,
    auth: State<'_, AuthService>,
    vault: State<'_, VaultService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<(), PublicIpcError> {
    let auth = auth.inner().clone();
    let vault = vault.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        delete_vault_record_value(&id, &auth, &vault, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command]
pub(crate) async fn list_notes(
    auth: State<'_, AuthService>,
    notes: State<'_, NotesService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<Vec<NoteRecord>, PublicIpcError> {
    let auth = auth.inner().clone();
    let notes = notes.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || list_notes_value(&auth, &notes, &operation_gate))
        .await
        .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn create_note(
    input: NoteInput,
    auth: State<'_, AuthService>,
    notes: State<'_, NotesService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<NoteRecord, PublicIpcError> {
    let auth = auth.inner().clone();
    let notes = notes.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        create_note_value(input, &auth, &notes, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn update_note(
    id: String,
    input: NoteInput,
    auth: State<'_, AuthService>,
    notes: State<'_, NotesService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<NoteRecord, PublicIpcError> {
    let auth = auth.inner().clone();
    let notes = notes.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        update_note_value(&id, input, &auth, &notes, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn delete_note(
    id: String,
    auth: State<'_, AuthService>,
    notes: State<'_, NotesService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<(), PublicIpcError> {
    let auth = auth.inner().clone();
    let notes = notes.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        delete_note_value(&id, &auth, &notes, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command]
pub(crate) async fn list_deleted_items(
    auth: State<'_, AuthService>,
    vault: State<'_, VaultService>,
    notes: State<'_, NotesService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<Vec<DeletedItem>, PublicIpcError> {
    let auth = auth.inner().clone();
    let vault = vault.inner().clone();
    let notes = notes.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        list_deleted_items_value(&auth, &vault, &notes, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn restore_deleted_item(
    id: String,
    item_type: DeletedItemType,
    auth: State<'_, AuthService>,
    vault: State<'_, VaultService>,
    notes: State<'_, NotesService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<(), PublicIpcError> {
    let auth = auth.inner().clone();
    let vault = vault.inner().clone();
    let notes = notes.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        restore_deleted_item_value(&id, item_type, &auth, &vault, &notes, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn permanently_delete_item(
    id: String,
    item_type: DeletedItemType,
    auth: State<'_, AuthService>,
    vault: State<'_, VaultService>,
    notes: State<'_, NotesService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<(), PublicIpcError> {
    let auth = auth.inner().clone();
    let vault = vault.inner().clone();
    let notes = notes.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        permanently_delete_item_value(&id, item_type, &auth, &vault, &notes, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command]
pub(crate) async fn empty_recently_deleted(
    auth: State<'_, AuthService>,
    vault: State<'_, VaultService>,
    notes: State<'_, NotesService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<(), PublicIpcError> {
    let auth = auth.inner().clone();
    let vault = vault.inner().clone();
    let notes = notes.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        empty_recently_deleted_value(&auth, &vault, &notes, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn copy_vault_password(
    id: String,
    auth: State<'_, AuthService>,
    vault: State<'_, VaultService>,
    clipboard: State<'_, ClipboardService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<(), PublicIpcError> {
    let auth = auth.inner().clone();
    let vault = vault.inner().clone();
    let clipboard = clipboard.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        copy_vault_password_value(&id, &auth, &vault, &clipboard, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn copy_vault_username(
    id: String,
    auth: State<'_, AuthService>,
    vault: State<'_, VaultService>,
    clipboard: State<'_, ClipboardService>,
    operation_gate: State<'_, SecurityOperationGate>,
) -> Result<(), PublicIpcError> {
    let auth = auth.inner().clone();
    let vault = vault.inner().clone();
    let clipboard = clipboard.inner().clone();
    let operation_gate = operation_gate.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        copy_vault_username_value(&id, &auth, &vault, &clipboard, &operation_gate)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
}

#[tauri::command]
pub(crate) async fn get_pending_host_approval(
    autofill: State<'_, AutofillService>,
) -> Result<Option<PendingHostApprovalView>, PublicIpcError> {
    let autofill = autofill.inner().clone();
    tauri::async_runtime::spawn_blocking(move || autofill.pending_host_approval())
        .await
        .map_err(|_| PublicIpcError::internal())?
        .map_err(PublicIpcError::from)
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn list_host_approval_candidates(
    approval_id: String,
    autofill: State<'_, AutofillService>,
) -> Result<Vec<HostApprovalCandidate>, PublicIpcError> {
    let autofill = autofill.inner().clone();
    tauri::async_runtime::spawn_blocking(move || autofill.host_approval_candidates(&approval_id))
        .await
        .map_err(|_| PublicIpcError::internal())?
        .map_err(PublicIpcError::from)
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn approve_login_host(
    approval_id: String,
    credential_id: String,
    autofill: State<'_, AutofillService>,
) -> Result<HostApprovalCompleted, PublicIpcError> {
    let autofill = autofill.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        autofill.approve_login_host(&approval_id, &credential_id)
    })
    .await
    .map_err(|_| PublicIpcError::internal())?
    .map_err(PublicIpcError::from)
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn cancel_host_approval(
    approval_id: String,
    autofill: State<'_, AutofillService>,
) -> Result<(), PublicIpcError> {
    let autofill = autofill.inner().clone();
    tauri::async_runtime::spawn_blocking(move || autofill.cancel_host_approval(&approval_id))
        .await
        .map_err(|_| PublicIpcError::internal())?
        .map_err(PublicIpcError::from)
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
        vault::VaultService,
    };

    const PASSWORD: &str = "a secure master password";

    #[derive(Default)]
    struct FixedEntropy(AtomicUsize);

    impl EntropySource for FixedEntropy {
        fn fill(&self, destination: &mut [u8]) -> Result<(), CryptoError> {
            let start = self.0.fetch_add(destination.len(), Ordering::SeqCst) as u8;
            for (index, byte) in destination.iter_mut().enumerate() {
                *byte = start.wrapping_add(index as u8);
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
        fail_write: AtomicBool,
        fail_read: AtomicBool,
        clear_calls: AtomicUsize,
    }

    impl ClipboardPort for FakeClipboardPort {
        fn write_text(&self, value: &str) -> Result<(), ClipboardError> {
            if self.fail_write.load(Ordering::SeqCst) {
                Err(ClipboardError::WriteFailed)
            } else {
                *self.value.lock().unwrap() = value.to_owned();
                Ok(())
            }
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
        vault: VaultService,
        operation_gate: SecurityOperationGate,
    }

    impl CommandFixture {
        fn new() -> Self {
            let temp = tempdir().unwrap();
            let params = KdfParams::testing();
            let auth = AuthService::load(
                ProfileStore::new(temp.path().to_path_buf()),
                params,
                Arc::new(FixedEntropy::default()),
            );
            let store = SettingsStore::new(temp.path().to_path_buf());
            let settings = SettingsService::load(store.clone()).unwrap();
            let startup_fake = FakeStartup::default();
            let startup = StartupService::new(Arc::new(startup_fake.clone()));
            let clipboard_port = Arc::new(FakeClipboardPort::default());
            let clipboard = ClipboardService::new(clipboard_port.clone(), Duration::from_secs(30));
            let vault =
                VaultService::new(temp.path().to_path_buf(), Arc::new(FixedEntropy::default()));
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
                vault,
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

    fn vault_input(password: &str) -> VaultRecordInput {
        named_vault_input("Example account", password)
    }

    fn named_vault_input(name: &str, password: &str) -> VaultRecordInput {
        VaultRecordInput {
            name: name.to_owned(),
            username: "alex@example.test".to_owned(),
            password: password.to_owned(),
            website: Some("https://example.test".to_owned()),
            allowed_login_hosts: vec![],
            tags: vec!["Important".to_owned()],
        }
    }

    #[test]
    fn locked_vault_commands_fail_before_vault_or_clipboard_access() {
        let fixture = CommandFixture::new();
        fixture
            .clipboard_port
            .value
            .lock()
            .unwrap()
            .push_str("unrelated clipboard text");
        let input = vault_input("selected password");

        assert_eq!(
            list_vault_records_value(&fixture.auth, &fixture.vault, &fixture.operation_gate)
                .unwrap_err()
                .code,
            "unauthorized"
        );
        assert_eq!(
            create_vault_record_value(
                input.clone(),
                &fixture.auth,
                &fixture.vault,
                &fixture.operation_gate,
            )
            .unwrap_err()
            .code,
            "unauthorized"
        );
        for result in [
            get_vault_record_summary_value(
                "missing",
                &fixture.auth,
                &fixture.vault,
                &fixture.operation_gate,
            )
            .map(|_| ()),
            get_vault_record_value(
                "missing",
                &fixture.auth,
                &fixture.vault,
                &fixture.operation_gate,
            )
            .map(|_| ()),
            update_vault_record_value(
                "missing",
                input,
                &fixture.auth,
                &fixture.vault,
                &fixture.operation_gate,
            )
            .map(|_| ()),
            delete_vault_record_value(
                "missing",
                &fixture.auth,
                &fixture.vault,
                &fixture.operation_gate,
            ),
            copy_vault_password_value(
                "missing",
                &fixture.auth,
                &fixture.vault,
                &fixture.clipboard,
                &fixture.operation_gate,
            ),
            copy_vault_username_value(
                "missing",
                &fixture.auth,
                &fixture.vault,
                &fixture.clipboard,
                &fixture.operation_gate,
            ),
        ] {
            assert_eq!(result.unwrap_err().code, "unauthorized");
        }
        assert!(!fixture.temp.path().join("vault.enc").exists());
        assert_eq!(
            *fixture.clipboard_port.value.lock().unwrap(),
            "unrelated clipboard text"
        );
    }

    #[test]
    fn vault_create_waiting_behind_a_linearized_lock_rechecks_authorization() {
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
        let vault = fixture.vault.clone();
        let operation_gate = fixture.operation_gate.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let (returned_tx, returned_rx) = mpsc::channel();
        let create = thread::spawn(move || {
            started_tx.send(()).unwrap();
            let result = create_vault_record_value(
                vault_input("must-never-be-persisted"),
                &auth,
                &vault,
                &operation_gate,
            );
            returned_tx.send(()).unwrap();
            result
        });
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(returned_rx
            .recv_timeout(Duration::from_millis(100))
            .is_err());

        assert_eq!(
            fixture
                .auto_lock
                .lock_now_with_operation_guard(&guard)
                .unwrap(),
            AuthStatus::Locked
        );
        drop(guard);

        assert_eq!(create.join().unwrap().unwrap_err().code, "unauthorized");
        assert!(!fixture.temp.path().join("vault.enc").exists());
    }

    #[test]
    fn unlocked_vault_commands_round_trip_and_copy_only_the_selected_password() {
        let fixture = CommandFixture::new();
        fixture.create_unlocked_profile();
        let first = create_vault_record_value(
            vault_input("first password"),
            &fixture.auth,
            &fixture.vault,
            &fixture.operation_gate,
        )
        .unwrap();
        let second = create_vault_record_value(
            named_vault_input("Second account", "second selected password"),
            &fixture.auth,
            &fixture.vault,
            &fixture.operation_gate,
        )
        .unwrap();

        let detail = get_vault_record_summary_value(
            &first.id,
            &fixture.auth,
            &fixture.vault,
            &fixture.operation_gate,
        )
        .unwrap();
        let wire = serde_json::to_value(&detail).unwrap();
        assert_eq!(detail.id, first.id);
        assert!(wire.get("password").is_none());
        assert!(!wire.to_string().contains("first password"));

        assert_eq!(
            list_vault_records_value(&fixture.auth, &fixture.vault, &fixture.operation_gate)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            get_vault_record_value(
                &first.id,
                &fixture.auth,
                &fixture.vault,
                &fixture.operation_gate
            )
            .unwrap()
            .password,
            "first password"
        );
        let updated = update_vault_record_value(
            &first.id,
            named_vault_input("Updated account", "updated password"),
            &fixture.auth,
            &fixture.vault,
            &fixture.operation_gate,
        )
        .unwrap();
        assert_eq!(updated.name, "Updated account");
        copy_vault_password_value(
            &second.id,
            &fixture.auth,
            &fixture.vault,
            &fixture.clipboard,
            &fixture.operation_gate,
        )
        .unwrap();
        assert_eq!(
            *fixture.clipboard_port.value.lock().unwrap(),
            "second selected password"
        );
        copy_vault_username_value(
            &second.id,
            &fixture.auth,
            &fixture.vault,
            &fixture.clipboard,
            &fixture.operation_gate,
        )
        .unwrap();
        assert_eq!(
            *fixture.clipboard_port.value.lock().unwrap(),
            "alex@example.test"
        );
        delete_vault_record_value(
            &first.id,
            &fixture.auth,
            &fixture.vault,
            &fixture.operation_gate,
        )
        .unwrap();
        assert_eq!(
            get_vault_record_value(
                &first.id,
                &fixture.auth,
                &fixture.vault,
                &fixture.operation_gate
            )
            .unwrap_err()
            .code,
            "vault-record-not-found"
        );
    }

    #[test]
    fn vault_copy_failure_uses_a_fixed_copy_specific_public_error() {
        let fixture = CommandFixture::new();
        fixture.create_unlocked_profile();
        let record = create_vault_record_value(
            vault_input("selected password"),
            &fixture.auth,
            &fixture.vault,
            &fixture.operation_gate,
        )
        .unwrap();
        fixture
            .clipboard_port
            .fail_write
            .store(true, Ordering::SeqCst);

        let error = copy_vault_password_value(
            &record.id,
            &fixture.auth,
            &fixture.vault,
            &fixture.clipboard,
            &fixture.operation_gate,
        )
        .unwrap_err();

        assert_eq!(error.code, "clipboard-copy-error");
        assert_eq!(error.message, "KeyNest could not copy this password.");
        assert_eq!(
            *fixture.clipboard_port.value.lock().unwrap(),
            "owned secret"
        );
    }

    #[test]
    fn vault_errors_serialize_to_fixed_safe_values_without_input_content() {
        let secret = "literal secret that must not escape";
        let cases = [
            (
                VaultError::InvalidName,
                "invalid-vault-name",
                "Enter a credential name.",
            ),
            (
                VaultError::InvalidUsername,
                "invalid-vault-username",
                "Enter a credential username.",
            ),
            (
                VaultError::InvalidPassword,
                "invalid-vault-password",
                "Enter a credential password.",
            ),
            (
                VaultError::InvalidWebsite,
                "invalid-vault-website",
                "Enter a valid credential website.",
            ),
            (
                VaultError::InvalidAllowedLoginHosts,
                "invalid-vault-login-hosts",
                "Enter valid login hostnames without paths or wildcards.",
            ),
            (
                VaultError::InvalidTags,
                "invalid-vault-tags",
                "Check the credential tags.",
            ),
            (
                VaultError::NotFound,
                "vault-record-not-found",
                "The credential was not found.",
            ),
            (
                VaultError::DataDamaged,
                "vault-data-error",
                "KeyNest's encrypted vault data is damaged or unsupported.",
            ),
            (
                VaultError::EntropyUnavailable,
                "vault-entropy-error",
                "KeyNest could not generate secure vault data.",
            ),
            (
                VaultError::StorageUnavailable,
                "vault-storage-error",
                "KeyNest could not access its encrypted vault data.",
            ),
        ];

        for (vault_error, code, message) in cases {
            let serialized = serde_json::to_string(&PublicIpcError::from(vault_error)).unwrap();
            assert!(serialized.contains(code));
            assert!(serialized.contains(message));
            assert!(!serialized.contains(secret));
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
    fn all_master_password_commands_enforce_the_shared_policy_vectors() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/master-password-policy.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let password = case["password"].as_str().unwrap();
            let allowed = case["strength"].as_str().unwrap() != "Weak";
            let setup = CommandFixture::new();
            let result =
                create_master_password_for_recovery(password, &setup.auth, &setup.operation_gate);
            assert_eq!(result.is_ok(), allowed, "setup case {}", case["id"]);
            if !allowed {
                assert_eq!(setup.auth.status(), AuthStatus::SetupRequired);
                assert!(!setup.temp.path().join("profile.json").exists());
            } else {
                setup.auth.lock();
                setup.auth.unlock(password).unwrap();
            }

            let fixture = CommandFixture::new();
            let recovery_key = fixture.auth.create_master_password(PASSWORD).unwrap();
            let before_key = fixture.auth.require_vault_key(|key| *key).unwrap();
            let profile_path = fixture.temp.path().join("profile.json");
            let before_profile = std::fs::read(&profile_path).unwrap();
            let result = change_master_password_value(
                PASSWORD,
                password,
                &fixture.auth,
                &fixture.operation_gate,
            );
            assert_eq!(result.is_ok(), allowed, "change case {}", case["id"]);
            if !allowed {
                assert_eq!(std::fs::read(&profile_path).unwrap(), before_profile);
            }
            assert_eq!(
                fixture.auth.require_vault_key(|key| *key).unwrap(),
                before_key
            );

            let before_profile = std::fs::read(&profile_path).unwrap();
            fixture.auth.lock();
            let result = recover_master_password_value(
                &recovery_key,
                password,
                &fixture.auth,
                &fixture.operation_gate,
            );
            assert_eq!(result.is_ok(), allowed, "recovery case {}", case["id"]);
            if !allowed {
                assert_eq!(fixture.auth.status(), AuthStatus::Locked);
                assert_eq!(std::fs::read(&profile_path).unwrap(), before_profile);
                fixture.auth.unlock(PASSWORD).unwrap();
            } else {
                fixture.auth.lock();
                fixture.auth.unlock(password).unwrap();
            }
            assert_eq!(
                fixture.auth.require_vault_key(|key| *key).unwrap(),
                before_key
            );
        }
    }

    #[test]
    fn direct_password_change_rejects_weak_passwords_without_mutation() {
        let fixture = CommandFixture::new();
        fixture.create_unlocked_profile();
        let profile_path = fixture.temp.path().join("profile.json");
        let before_profile = std::fs::read(&profile_path).unwrap();
        let before_key = fixture.auth.require_vault_key(|key| *key).unwrap();
        for weak in [
            "123456789012",
            "password1234",
            "111111111111",
            "aaaaaaaaaaaa",
            "abcdefghijkl",
            "abababababab",
        ] {
            let error = change_master_password_value(
                PASSWORD,
                weak,
                &fixture.auth,
                &fixture.operation_gate,
            )
            .unwrap_err();
            assert_eq!(error.code, "password-too-weak");
            assert_eq!(error.message, "Master Password is too weak.");
            assert_eq!(std::fs::read(&profile_path).unwrap(), before_profile);
            assert_eq!(
                fixture.auth.require_vault_key(|key| *key).unwrap(),
                before_key
            );
        }
        fixture.auth.lock();
        fixture.auth.unlock(PASSWORD).unwrap();
    }

    #[test]
    fn direct_password_change_accepts_good_and_strong_passwords() {
        for replacement in ["kqmwzptxvbnr", "V7!qR2@tL9#z"] {
            let fixture = CommandFixture::new();
            fixture.create_unlocked_profile();
            let before_key = fixture.auth.require_vault_key(|key| *key).unwrap();
            assert_eq!(
                change_master_password_value(
                    PASSWORD,
                    replacement,
                    &fixture.auth,
                    &fixture.operation_gate,
                )
                .unwrap(),
                AuthStatus::Unlocked
            );
            assert_eq!(
                fixture.auth.require_vault_key(|key| *key).unwrap(),
                before_key
            );
            fixture.auth.lock();
            fixture.auth.unlock(replacement).unwrap();
        }
    }

    #[test]
    fn auth_error_codes_and_messages_remain_stable() {
        let cases = [
            (AuthError::PasswordTooShort, "password-too-short"),
            (AuthError::PasswordTooWeak, "password-too-weak"),
            (AuthError::AlreadyInitialized, "already-initialized"),
            (AuthError::NotInitialized, "not-initialized"),
            (AuthError::InvalidCredentials, "invalid-credentials"),
            (AuthError::RecoveryNotConfigured, "recovery-not-configured"),
            (AuthError::InvalidRecoveryKey, "invalid-recovery-key"),
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
        assert_eq!(value.as_object().unwrap().len(), 5);
        assert_eq!(value["lockOnSleep"], true);
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
    fn sleep_lock_toggle_persists_both_values_and_rejects_locked_changes() {
        let fixture = CommandFixture::new();
        fixture.auth.create_master_password(PASSWORD).unwrap();
        for enabled in [false, true] {
            let snapshot = set_lock_on_sleep_value(
                enabled,
                &fixture.auth,
                &fixture.settings,
                &fixture.startup,
                &fixture.operation_gate,
            )
            .unwrap();
            assert_eq!(snapshot.lock_on_sleep, enabled);
            let reloaded =
                SettingsService::load(SettingsStore::new(fixture.temp.path().to_path_buf()))
                    .unwrap();
            assert_eq!(reloaded.snapshot(false).lock_on_sleep, enabled);
        }

        fixture.auto_lock.lock_now().unwrap();
        assert_eq!(
            set_lock_on_sleep_value(
                false,
                &fixture.auth,
                &fixture.settings,
                &fixture.startup,
                &fixture.operation_gate,
            )
            .unwrap_err()
            .code,
            "unauthorized"
        );
        assert!(fixture.settings.snapshot(false).lock_on_sleep);
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

    #[test]
    fn concurrent_unlock_and_recovery_calls_share_throttle_without_holding_gate_for_delay() {
        let fixture = CommandFixture::new();
        let key = fixture.auth.create_master_password(PASSWORD).unwrap();
        fixture.auth.lock();
        let before = std::fs::read(fixture.temp.path().join("profile.json")).unwrap();
        for _ in 0..3 {
            assert_eq!(
                unlock_and_arm(
                    "wrong",
                    &fixture.auth,
                    &fixture.auto_lock,
                    &fixture.operation_gate
                ),
                Err(AuthError::InvalidCredentials)
            );
        }
        let error = recover_master_password_value(
            "invalid key",
            "a replacement master password",
            &fixture.auth,
            &fixture.operation_gate,
        )
        .err()
        .unwrap();
        assert_eq!(error.code, "throttled");
        assert_eq!(error.retry_after_ms, Some(2000));

        let (sent, received) = mpsc::channel();
        let mut workers = Vec::new();
        for recovery in [false, true, false, true] {
            let auth = fixture.auth.clone();
            let gate = fixture.operation_gate.clone();
            let auto_lock = fixture.auto_lock.clone();
            let sent = sent.clone();
            let key = key.clone();
            workers.push(thread::spawn(move || {
                let result = if recovery {
                    recover_master_password_value(
                        &key,
                        "a replacement master password",
                        &auth,
                        &gate,
                    )
                    .map(|_| ())
                } else {
                    unlock_and_arm(PASSWORD, &auth, &auto_lock, &gate)
                        .map(|_| ())
                        .map_err(PublicIpcError::from)
                };
                sent.send(result).unwrap();
            }));
        }
        for _ in 0..4 {
            // A cooldown must return, not sleep with the operation gate held.
            let error = received
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .unwrap_err();
            assert_eq!(error.code, "throttled");
            assert!((1..=2000).contains(&error.retry_after_ms.unwrap()));
        }
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(fixture.auth.status(), AuthStatus::Locked);
        assert!(!fixture.auto_lock.is_armed_for_test());
        assert!(
            recovery_status_value(&fixture.auth, &fixture.operation_gate)
                .unwrap()
                .configured
        );
        let error = fixture
            .reset_authenticated(PASSWORD, "RESET KEYNEST")
            .unwrap_err();
        assert_eq!(error.code, "unauthorized");
        let error = reset_recovery(
            "RESET",
            &fixture.auth,
            &fixture.startup,
            &fixture.clipboard,
            &fixture.settings,
            &fixture.auto_lock,
            &fixture.operation_gate,
        )
        .unwrap_err();
        assert_eq!(error.code, "invalid-reset-confirmation");
        assert_eq!(
            std::fs::read(fixture.temp.path().join("profile.json")).unwrap(),
            before
        );
    }

    #[test]
    fn offline_recovery_preserves_real_encrypted_records_across_restart_and_rotation() {
        use zeroize::Zeroizing;

        let fixture = CommandFixture::new();
        let clipboard_before = "unrelated clipboard text";
        *fixture.clipboard_port.value.lock().unwrap() = clipboard_before.to_owned();
        let setup =
            create_master_password_for_recovery(PASSWORD, &fixture.auth, &fixture.operation_gate)
                .unwrap();
        let original_recovery = Zeroizing::new(setup.recovery_key.clone());
        let original_key = fixture
            .auth
            .require_vault_key(|key| Zeroizing::new(*key))
            .unwrap();
        let record_password = "fixture credential password";
        let record = create_vault_record_value(
            vault_input(record_password),
            &fixture.auth,
            &fixture.vault,
            &fixture.operation_gate,
        )
        .unwrap();
        let vault_path = fixture.temp.path().join("vault.enc");
        let profile_path = fixture.temp.path().join("profile.json");
        let original_vault = std::fs::read(&vault_path).unwrap();
        let original_profile = std::fs::read(&profile_path).unwrap();
        assert_eq!(
            *fixture.clipboard_port.value.lock().unwrap(),
            clipboard_before
        );
        fixture.auth.lock();

        // Restart the auth service from persisted wrappers, not the setup session.
        let auth = AuthService::load(
            ProfileStore::new(fixture.temp.path().to_path_buf()),
            KdfParams::testing(),
            Arc::new(crate::security::OsEntropy),
        );
        let wrong = "KN-R1-FFFF-FFFF-FFFF-FFFF-FFFF-FFFF-FFFF-FFFF";
        let error = recover_master_password_value(
            wrong,
            "replacement master password",
            &auth,
            &fixture.operation_gate,
        )
        .err()
        .unwrap();
        assert_eq!(error.code, "invalid-recovery-key");
        assert_eq!(auth.status(), AuthStatus::Locked);
        assert_eq!(std::fs::read(&profile_path).unwrap(), original_profile);
        assert_eq!(std::fs::read(&vault_path).unwrap(), original_vault);

        let recovered = recover_master_password_value(
            &original_recovery,
            "replacement master password",
            &auth,
            &fixture.operation_gate,
        )
        .unwrap();
        assert_eq!(recovered.status, AuthStatus::Unlocked);
        let replacement_recovery = Zeroizing::new(recovered.recovery_key.clone());
        assert!(replacement_recovery.as_str() != original_recovery.as_str());
        assert!(auth
            .require_vault_key(|key| key == original_key.as_ref())
            .unwrap());
        assert_eq!(std::fs::read(&vault_path).unwrap(), original_vault);
        assert_eq!(
            *fixture.clipboard_port.value.lock().unwrap(),
            clipboard_before
        );
        let vault = VaultService::new(
            fixture.temp.path().to_path_buf(),
            Arc::new(crate::security::OsEntropy),
        );
        let read =
            get_vault_record_value(&record.id, &auth, &vault, &fixture.operation_gate).unwrap();
        assert!(read.password == record_password);
        assert_eq!(std::fs::read(&vault_path).unwrap(), original_vault);

        let current_profile = std::fs::read(&profile_path).unwrap();
        for secret in [
            PASSWORD,
            "replacement master password",
            original_recovery.as_str(),
            replacement_recovery.as_str(),
            record_password,
        ] {
            for bytes in [&original_profile, &current_profile, &original_vault] {
                assert!(!bytes
                    .windows(secret.len())
                    .any(|window| window == secret.as_bytes()));
            }
        }
        // Status exposes configuration only, never a way to retrieve a saved key.
        let status =
            serde_json::to_value(recovery_status_value(&auth, &fixture.operation_gate).unwrap())
                .unwrap();
        assert_eq!(status, serde_json::json!({"configured": true}));
        auth.lock();
        drop(auth);
        let auth = AuthService::load(
            ProfileStore::new(fixture.temp.path().to_path_buf()),
            KdfParams::testing(),
            Arc::new(crate::security::OsEntropy),
        );
        assert_eq!(auth.unlock(PASSWORD), Err(AuthError::InvalidCredentials));
        auth.unlock("replacement master password").unwrap();
        auth.lock();
        let error = recover_master_password_value(
            &original_recovery,
            "another replacement password",
            &auth,
            &fixture.operation_gate,
        )
        .err()
        .unwrap();
        assert_eq!(error.code, "invalid-recovery-key");
        assert_eq!(std::fs::read(&profile_path).unwrap(), current_profile);
        let next = recover_master_password_value(
            &replacement_recovery,
            "another replacement password",
            &auth,
            &fixture.operation_gate,
        )
        .unwrap();
        assert!(next.recovery_key != *replacement_recovery);
        assert!(auth
            .require_vault_key(|key| key == original_key.as_ref())
            .unwrap());
        assert!(
            get_vault_record_value(&record.id, &auth, &vault, &fixture.operation_gate)
                .unwrap()
                .password
                == record_password
        );
        assert_eq!(std::fs::read(&vault_path).unwrap(), original_vault);
        assert_eq!(
            *fixture.clipboard_port.value.lock().unwrap(),
            clipboard_before
        );
    }

    #[test]
    fn master_password_recovery_waits_for_the_shared_security_operation_gate() {
        let fixture = CommandFixture::new();
        let recovery_key = fixture.auth.create_master_password(PASSWORD).unwrap();
        fixture.auth.lock();
        let guard = fixture.operation_gate.lock();
        let auth = fixture.auth.clone();
        let operation_gate = fixture.operation_gate.clone();
        let recovery_key = recovery_key.to_string();
        let (started_tx, started_rx) = mpsc::channel();
        let recovery = thread::spawn(move || {
            started_tx.send(()).unwrap();
            recover_master_password_value(
                &recovery_key,
                "new secure master password",
                &auth,
                &operation_gate,
            )
        });
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(!recovery.is_finished());

        drop(guard);
        let result = recovery.join().unwrap().unwrap();
        assert_eq!(result.status, AuthStatus::Unlocked);
        assert!(result.recovery_key.starts_with("KN-R1-"));
    }

    #[test]
    fn one_time_recovery_key_displays_pause_and_then_rearm_auto_lock() {
        let fixture = CommandFixture::new();
        let setup =
            create_master_password_for_recovery(PASSWORD, &fixture.auth, &fixture.operation_gate)
                .unwrap();
        assert_eq!(setup.status, AuthStatus::Unlocked);
        assert!(!fixture.auto_lock.is_armed_for_test());
        assert_eq!(
            finish_recovery_key_display(
                &fixture.auth,
                &fixture.auto_lock,
                &fixture.operation_gate,
            )
            .unwrap(),
            AuthStatus::Unlocked
        );
        assert!(fixture.auto_lock.is_armed_for_test());

        regenerate_recovery_key_value(
            PASSWORD,
            &fixture.auth,
            &fixture.auto_lock,
            &fixture.operation_gate,
        )
        .unwrap();
        assert!(!fixture.auto_lock.is_armed_for_test());
        finish_recovery_key_display(&fixture.auth, &fixture.auto_lock, &fixture.operation_gate)
            .unwrap();
        assert!(fixture.auto_lock.is_armed_for_test());
    }
}
