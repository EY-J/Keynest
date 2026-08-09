mod security;

use std::sync::Arc;

use security::{AuthError, AuthService, AuthStatus, KdfParams, OsEntropy, ProfileStore};
use serde::Serialize;
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
                message: "Type RESET exactly to confirm.",
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

#[tauri::command]
fn get_auth_status(auth: State<'_, AuthService>) -> AuthStatus {
    auth.status()
}

#[tauri::command]
async fn create_master_password(
    mut password: String,
    auth: State<'_, AuthService>,
) -> Result<AuthStatus, PublicAuthError> {
    let service = auth.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = service.create_master_password(&password);
        password.zeroize();
        result.map(|()| service.status()).map_err(Into::into)
    })
    .await
    .map_err(|_| PublicAuthError::internal())?
}

#[tauri::command]
async fn unlock(
    mut password: String,
    auth: State<'_, AuthService>,
) -> Result<AuthStatus, PublicAuthError> {
    let service = auth.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = service.unlock(&password);
        password.zeroize();
        result.map(|()| service.status()).map_err(Into::into)
    })
    .await
    .map_err(|_| PublicAuthError::internal())?
}

#[tauri::command]
fn lock(auth: State<'_, AuthService>) -> AuthStatus {
    auth.lock()
}

#[tauri::command]
fn reset_keynest(
    confirmation: String,
    auth: State<'_, AuthService>,
) -> Result<AuthStatus, PublicAuthError> {
    auth.reset_keynest(&confirmation)
        .map(|()| auth.status())
        .map_err(Into::into)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let kdf_params = KdfParams::production();
            let store = ProfileStore::new(app_data_dir, kdf_params);
            app.manage(AuthService::load(store, kdf_params, Arc::new(OsEntropy)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_auth_status,
            create_master_password,
            unlock,
            lock,
            reset_keynest
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
}
