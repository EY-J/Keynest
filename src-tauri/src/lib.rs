mod security;
mod settings;

use std::{sync::Arc, time::Duration};

use security::{
    AuthError, AuthService, AuthStatus, AutoLockService, ClipboardService, KdfParams,
    LockCoordinator, LockError, OsEntropy, ProfileStore, TauriClipboardPort, TauriLockEventSink,
};
use serde::Serialize;
use settings::{SettingsService, SettingsStore};
use tauri::{Manager, State};
use zeroize::Zeroize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicAuthError {
    code: &'static str,
    message: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_after_ms: Option<u64>,
}

impl PublicAuthError {
    fn internal() -> Self {
        Self {
            code: "internal-error",
            message: "KeyNest could not complete the security request.",
            retry_after_ms: None,
        }
    }
}

impl From<AuthError> for PublicAuthError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::PasswordTooShort => Self {
                code: "password-too-short",
                message: "Use at least 12 characters.",
                retry_after_ms: None,
            },
            AuthError::AlreadyInitialized => Self {
                code: "already-initialized",
                message: "KeyNest already has a master password.",
                retry_after_ms: None,
            },
            AuthError::NotInitialized => Self {
                code: "not-initialized",
                message: "Create a master password before unlocking KeyNest.",
                retry_after_ms: None,
            },
            AuthError::InvalidCredentials => Self {
                code: "invalid-credentials",
                message: "The master password is incorrect.",
                retry_after_ms: None,
            },
            AuthError::Throttled { retry_after_ms } => Self {
                code: "throttled",
                message: "Wait a moment before trying again.",
                retry_after_ms: Some(retry_after_ms),
            },
            AuthError::InvalidResetConfirmation => Self {
                code: "invalid-reset-confirmation",
                message: "Type RESET KEYNEST exactly to confirm.",
                retry_after_ms: None,
            },
            AuthError::Unauthorized => Self {
                code: "unauthorized",
                message: "KeyNest is locked.",
                retry_after_ms: None,
            },
            AuthError::DataDamaged => Self {
                code: "data-error",
                message: "KeyNest's encrypted local data is damaged or unsupported.",
                retry_after_ms: None,
            },
            AuthError::LocalDataFailure => Self {
                code: "local-data-error",
                message: "KeyNest could not access its encrypted local data.",
                retry_after_ms: None,
            },
        }
    }
}

impl From<LockError> for PublicAuthError {
    fn from(_: LockError) -> Self {
        Self::internal()
    }
}

#[tauri::command]
fn get_auth_status(auth: State<'_, AuthService>) -> AuthStatus {
    auth.status()
}

#[tauri::command]
async fn create_master_password(
    mut password: String,
    auth: State<'_, AuthService>,
    auto_lock: State<'_, AutoLockService>,
) -> Result<AuthStatus, PublicAuthError> {
    let service = auth.inner().clone();
    let auto_lock = auto_lock.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = service.create_master_password(&password);
        password.zeroize();
        result
            .map(|()| {
                auto_lock.arm();
                service.status()
            })
            .map_err(Into::into)
    })
    .await
    .map_err(|_| PublicAuthError::internal())?
}

#[tauri::command]
async fn unlock(
    mut password: String,
    auth: State<'_, AuthService>,
    auto_lock: State<'_, AutoLockService>,
) -> Result<AuthStatus, PublicAuthError> {
    let service = auth.inner().clone();
    let auto_lock = auto_lock.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = service.unlock(&password);
        password.zeroize();
        result
            .map(|()| {
                auto_lock.arm();
                service.status()
            })
            .map_err(Into::into)
    })
    .await
    .map_err(|_| PublicAuthError::internal())?
}

#[tauri::command]
async fn change_master_password(
    mut current_password: String,
    mut new_password: String,
    auth: State<'_, AuthService>,
) -> Result<AuthStatus, PublicAuthError> {
    let service = auth.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = service.change_master_password(&current_password, &new_password);
        current_password.zeroize();
        new_password.zeroize();
        result.map(|()| service.status()).map_err(Into::into)
    })
    .await
    .map_err(|_| PublicAuthError::internal())?
}

#[tauri::command]
fn lock(auto_lock: State<'_, AutoLockService>) -> Result<AuthStatus, PublicAuthError> {
    auto_lock.lock_now().map_err(Into::into)
}

#[tauri::command]
fn record_activity(
    auth: State<'_, AuthService>,
    auto_lock: State<'_, AutoLockService>,
) -> Result<(), PublicAuthError> {
    if auth.status() != AuthStatus::Unlocked {
        return Err(AuthError::Unauthorized.into());
    }
    auto_lock.record_activity();
    Ok(())
}

#[tauri::command]
fn reset_keynest(
    confirmation: String,
    auth: State<'_, AuthService>,
    auto_lock: State<'_, AutoLockService>,
) -> Result<AuthStatus, PublicAuthError> {
    auth.reset_keynest(&confirmation)
        .map(|()| {
            auto_lock.disarm();
            auth.status()
        })
        .map_err(Into::into)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let settings_store = SettingsStore::new(app_data_dir.clone());
            let settings = SettingsService::load(settings_store)?;
            let snapshot = settings.snapshot(false);
            app.manage(settings);
            let kdf_params = KdfParams::production();
            let store = ProfileStore::new(app_data_dir, kdf_params);
            let auth = AuthService::load(store, kdf_params, Arc::new(OsEntropy));
            app.manage(auth.clone());
            let clipboard = ClipboardService::new(
                Arc::new(TauriClipboardPort::new(app.handle().clone())),
                Duration::from_secs(snapshot.clipboard_clear_seconds),
            );
            app.manage(clipboard.clone());
            let coordinator = LockCoordinator::new(
                auth,
                clipboard,
                Arc::new(TauriLockEventSink::new(app.handle().clone())),
            );
            app.manage(coordinator.clone());
            app.manage(AutoLockService::new(
                Arc::new(coordinator),
                Duration::from_secs(snapshot.auto_lock_seconds),
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_auth_status,
            create_master_password,
            unlock,
            change_master_password,
            lock,
            record_activity,
            reset_keynest
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            tauri::RunEvent::Resumed => {
                let _ = app.state::<AutoLockService>().lock_now();
            }
            tauri::RunEvent::ExitRequested { api, code, .. } => {
                let clipboard = app.state::<ClipboardService>();
                if clipboard.begin_process_exit_cleanup() {
                    api.prevent_exit();
                    let app = app.clone();
                    let finish = Arc::new(move || app.exit(code.unwrap_or(0)));
                    let _ =
                        clipboard.start_process_exit_cleanup(Duration::from_millis(250), finish);
                }
            }
            _ => {}
        });
}

#[cfg(test)]
mod command_tests {
    use super::{
        security::{AuthError, AuthStatus},
        PublicAuthError,
    };

    #[test]
    fn auth_status_uses_kebab_case_ipc_values() {
        assert_eq!(
            serde_json::to_string(&AuthStatus::SetupRequired).unwrap(),
            "\"setup-required\""
        );
        assert_eq!(
            serde_json::to_string(&AuthStatus::DataError).unwrap(),
            "\"data-error\""
        );
    }

    #[test]
    fn invalid_credentials_expose_only_a_generic_message() {
        let public = PublicAuthError::from(AuthError::InvalidCredentials);

        assert_eq!(public.code, "invalid-credentials");
        assert_eq!(public.message, "The master password is incorrect.");
        assert_eq!(public.retry_after_ms, None);
    }

    #[test]
    fn throttled_error_preserves_the_safe_retry_delay() {
        let public = PublicAuthError::from(AuthError::Throttled {
            retry_after_ms: 2_000,
        });

        assert_eq!(public.code, "throttled");
        assert_eq!(public.retry_after_ms, Some(2_000));
    }

    #[test]
    fn password_change_public_error_serializes_password_too_short_safely() {
        let public = PublicAuthError::from(AuthError::PasswordTooShort);
        let serialized = serde_json::to_value(public).unwrap();

        assert_eq!(serialized["code"], "password-too-short");
        assert_eq!(serialized["message"], "Use at least 12 characters.");
        assert!(serialized.get("retryAfterMs").is_none());
    }

    #[test]
    fn password_change_public_error_serializes_invalid_credentials_safely() {
        let public = PublicAuthError::from(AuthError::InvalidCredentials);
        let serialized = serde_json::to_value(public).unwrap();

        assert_eq!(serialized["code"], "invalid-credentials");
        assert_eq!(serialized["message"], "The master password is incorrect.");
        assert!(serialized.get("retryAfterMs").is_none());
    }

    #[test]
    fn password_change_public_error_serializes_unauthorized_safely() {
        let public = PublicAuthError::from(AuthError::Unauthorized);
        let serialized = serde_json::to_value(public).unwrap();

        assert_eq!(serialized["code"], "unauthorized");
        assert_eq!(serialized["message"], "KeyNest is locked.");
        assert!(serialized.get("retryAfterMs").is_none());
    }

    #[test]
    fn reset_confirmation_public_error_names_the_exact_required_phrase() {
        let public = PublicAuthError::from(AuthError::InvalidResetConfirmation);

        assert_eq!(public.code, "invalid-reset-confirmation");
        assert_eq!(public.message, "Type RESET KEYNEST exactly to confirm.");
    }
}
