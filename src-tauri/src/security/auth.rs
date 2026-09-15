use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use thiserror::Error;
use zeroize::Zeroizing;

use super::{
    crypto::{
        generate_recovery_key, unwrap_pin_vault_key, unwrap_recovery_vault_key, unwrap_vault_key,
        wrap_existing_vault_key, wrap_new_vault_key, wrap_pin_vault_key, wrap_recovery_vault_key,
        CryptoError, EntropySource, KdfParams, VaultKey,
    },
    device_protection::{DeviceProtector, OsDeviceProtector},
    password_policy::validate_master_password,
    storage::{ProfileLoad, ProfileStore, StorageError, StoredProfile},
};

const RESET_CONFIRMATION: &str = "RESET KEYNEST";
const DEVICE_SECRET_LENGTH: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AuthStatus {
    SetupRequired,
    Locked,
    Unlocked,
    DataError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LockOutcome {
    pub status: AuthStatus,
    pub transitioned: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecoveryStatus {
    pub configured: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PinStatus {
    pub configured: bool,
    pub unlock_available: bool,
}

#[derive(Clone)]
pub(crate) struct AuthService {
    inner: Arc<Mutex<AuthInner>>,
    store: ProfileStore,
    kdf_params: KdfParams,
    entropy: Arc<dyn EntropySource>,
    device_protector: Arc<dyn DeviceProtector>,
}

struct AuthInner {
    state: AuthState,
    failed_attempts: u32,
    last_failed_at: Option<Instant>,
    pin_failed_attempts: u8,
    pin_last_failed_at: Option<Instant>,
    pin_disabled: bool,
}

enum AuthState {
    SetupRequired,
    Locked(StoredProfile),
    Unlocked {
        profile: StoredProfile,
        vault_key: VaultKey,
    },
    DataError,
}

impl AuthService {
    pub(crate) fn load(
        store: ProfileStore,
        kdf_params: KdfParams,
        entropy: Arc<dyn EntropySource>,
    ) -> Self {
        Self::load_with_device_protector(store, kdf_params, entropy, Arc::new(OsDeviceProtector))
    }

    pub(crate) fn load_with_device_protector(
        store: ProfileStore,
        kdf_params: KdfParams,
        entropy: Arc<dyn EntropySource>,
        device_protector: Arc<dyn DeviceProtector>,
    ) -> Self {
        let state = match store.load() {
            Ok(ProfileLoad::Missing) => AuthState::SetupRequired,
            Ok(ProfileLoad::Valid(profile)) => AuthState::Locked(*profile),
            Err(_) => AuthState::DataError,
        };
        Self {
            inner: Arc::new(Mutex::new(AuthInner {
                state,
                failed_attempts: 0,
                last_failed_at: None,
                pin_failed_attempts: 0,
                pin_last_failed_at: None,
                pin_disabled: false,
            })),
            store,
            kdf_params,
            entropy,
            device_protector,
        }
    }

    pub(crate) fn status(&self) -> AuthStatus {
        match &self.lock_inner().state {
            AuthState::SetupRequired => AuthStatus::SetupRequired,
            AuthState::Locked(_) => AuthStatus::Locked,
            AuthState::Unlocked { .. } => AuthStatus::Unlocked,
            AuthState::DataError => AuthStatus::DataError,
        }
    }

    pub(crate) fn create_master_password(
        &self,
        password: &str,
    ) -> Result<Zeroizing<String>, AuthError> {
        validate_master_password(password)?;

        let mut inner = self.lock_inner();
        match inner.state {
            AuthState::SetupRequired => {}
            AuthState::DataError => return Err(AuthError::DataDamaged),
            AuthState::Locked(_) | AuthState::Unlocked { .. } => {
                return Err(AuthError::AlreadyInitialized)
            }
        }

        let (wrapped_key, vault_key) =
            wrap_new_vault_key(password, self.kdf_params, self.entropy.as_ref())
                .map_err(|_| AuthError::LocalDataFailure)?;
        let recovery_key = generate_recovery_key(self.entropy.as_ref())
            .map_err(|_| AuthError::LocalDataFailure)?;
        let recovery_wrapped_key = wrap_recovery_vault_key(
            &recovery_key,
            vault_key.expose(),
            self.kdf_params,
            self.entropy.as_ref(),
        )
        .map_err(|_| AuthError::LocalDataFailure)?;
        let profile = StoredProfile::with_recovery(wrapped_key, recovery_wrapped_key);
        if self.store.create(&profile).is_err() {
            inner.state = AuthState::DataError;
            return Err(AuthError::LocalDataFailure);
        }

        inner.failed_attempts = 0;
        inner.last_failed_at = None;
        inner.state = AuthState::Unlocked { profile, vault_key };
        Ok(recovery_key)
    }

    pub(crate) fn recovery_status(&self) -> Result<RecoveryStatus, AuthError> {
        let inner = self.lock_inner();
        let configured = match &inner.state {
            AuthState::Locked(profile) | AuthState::Unlocked { profile, .. } => {
                profile.recovery_wrapped_key.is_some()
            }
            AuthState::SetupRequired => false,
            AuthState::DataError => return Err(AuthError::DataDamaged),
        };
        Ok(RecoveryStatus { configured })
    }

    pub(crate) fn pin_status(&self) -> Result<PinStatus, AuthError> {
        let inner = self.lock_inner();
        let configured = match &inner.state {
            AuthState::Locked(profile) | AuthState::Unlocked { profile, .. } => {
                profile.pin_wrapped_key.is_some()
            }
            AuthState::SetupRequired => false,
            AuthState::DataError => return Err(AuthError::DataDamaged),
        };
        Ok(PinStatus {
            configured,
            unlock_available: configured && !inner.pin_disabled,
        })
    }

    pub(crate) fn recover_master_password(
        &self,
        recovery_key: &str,
        new_password: &str,
    ) -> Result<Zeroizing<String>, AuthError> {
        self.recover_master_password_with_clock(recovery_key, new_password, Instant::now)
    }

    fn recover_master_password_with_clock(
        &self,
        recovery_key: &str,
        new_password: &str,
        now: impl Fn() -> Instant,
    ) -> Result<Zeroizing<String>, AuthError> {
        validate_master_password(new_password)?;

        let mut inner = self.lock_inner();
        check_attempt_delay(&inner, now())?;
        let profile = match &inner.state {
            AuthState::Locked(profile) => profile.clone(),
            AuthState::Unlocked { .. } => return Err(AuthError::Unauthorized),
            AuthState::SetupRequired => return Err(AuthError::NotInitialized),
            AuthState::DataError => return Err(AuthError::DataDamaged),
        };
        let recovery_wrapped_key = profile
            .recovery_wrapped_key
            .as_ref()
            .ok_or(AuthError::RecoveryNotConfigured)?;
        let vault_key = match unwrap_recovery_vault_key(recovery_key, recovery_wrapped_key) {
            Ok(key) => key,
            Err(CryptoError::AuthenticationFailed | CryptoError::InvalidRecoveryKey) => {
                return Err(record_failed_attempt(
                    &mut inner,
                    now(),
                    AuthError::InvalidRecoveryKey,
                ));
            }
            Err(_) => {
                inner.state = AuthState::DataError;
                return Err(AuthError::DataDamaged);
            }
        };

        let wrapped_key = wrap_existing_vault_key(
            new_password,
            vault_key.expose(),
            self.kdf_params,
            self.entropy.as_ref(),
        )
        .map_err(|_| AuthError::LocalDataFailure)?;
        let replacement_recovery_key = generate_recovery_key(self.entropy.as_ref())
            .map_err(|_| AuthError::LocalDataFailure)?;
        let replacement_recovery_wrapped_key = wrap_recovery_vault_key(
            &replacement_recovery_key,
            vault_key.expose(),
            self.kdf_params,
            self.entropy.as_ref(),
        )
        .map_err(|_| AuthError::LocalDataFailure)?;
        let replacement =
            StoredProfile::with_recovery(wrapped_key, replacement_recovery_wrapped_key);
        self.store.replace(&replacement)?;
        inner.failed_attempts = 0;
        inner.last_failed_at = None;
        inner.state = AuthState::Unlocked {
            profile: replacement,
            vault_key,
        };
        Ok(replacement_recovery_key)
    }

    pub(crate) fn regenerate_recovery_key(
        &self,
        current_password: &str,
    ) -> Result<Zeroizing<String>, AuthError> {
        let mut inner = self.lock_inner();
        let (profile, vault_key) = match &mut inner.state {
            AuthState::Unlocked { profile, vault_key } => (profile, vault_key),
            AuthState::Locked(_) => return Err(AuthError::Unauthorized),
            AuthState::SetupRequired => return Err(AuthError::NotInitialized),
            AuthState::DataError => return Err(AuthError::DataDamaged),
        };
        verify_master_password(current_password, profile, vault_key)?;

        let recovery_key = generate_recovery_key(self.entropy.as_ref())
            .map_err(|_| AuthError::LocalDataFailure)?;
        let recovery_wrapped_key = wrap_recovery_vault_key(
            &recovery_key,
            vault_key.expose(),
            self.kdf_params,
            self.entropy.as_ref(),
        )
        .map_err(|_| AuthError::LocalDataFailure)?;
        let replacement = profile.replacing_recovery_wrapper(recovery_wrapped_key);
        self.store.replace(&replacement)?;
        *profile = replacement;
        Ok(recovery_key)
    }

    pub(crate) fn unlock(&self, password: &str) -> Result<(), AuthError> {
        self.unlock_with_clock(password, Instant::now)
    }

    #[cfg(test)]
    pub(crate) fn unlock_at(&self, password: &str, now: Instant) -> Result<(), AuthError> {
        self.unlock_with_clock(password, || now)
    }

    fn unlock_with_clock(
        &self,
        password: &str,
        now: impl Fn() -> Instant,
    ) -> Result<(), AuthError> {
        let mut inner = self.lock_inner();
        check_attempt_delay(&inner, now())?;

        let profile = match &inner.state {
            AuthState::Locked(profile) => profile.clone(),
            AuthState::Unlocked { .. } => return Ok(()),
            AuthState::SetupRequired => return Err(AuthError::NotInitialized),
            AuthState::DataError => return Err(AuthError::DataDamaged),
        };

        match unwrap_vault_key(password, &profile.wrapped_key) {
            Ok(vault_key) => {
                inner.failed_attempts = 0;
                inner.last_failed_at = None;
                reset_pin_attempts(&mut inner);
                inner.state = AuthState::Unlocked { profile, vault_key };
                Ok(())
            }
            Err(CryptoError::AuthenticationFailed) => Err(record_failed_attempt(
                &mut inner,
                now(),
                AuthError::InvalidCredentials,
            )),
            Err(_) => {
                inner.state = AuthState::DataError;
                Err(AuthError::DataDamaged)
            }
        }
    }

    pub(crate) fn unlock_with_pin(&self, pin: &str) -> Result<(), AuthError> {
        self.unlock_with_pin_at(pin, Instant::now())
    }

    fn unlock_with_pin_at(&self, pin: &str, now: Instant) -> Result<(), AuthError> {
        validate_pin(pin)?;
        let mut inner = self.lock_inner();
        if inner.pin_disabled {
            return Err(AuthError::PinRequiresMasterPassword);
        }
        check_pin_attempt_delay(&inner, now)?;
        let profile = match &inner.state {
            AuthState::Locked(profile) => profile.clone(),
            AuthState::Unlocked { .. } => return Ok(()),
            AuthState::SetupRequired => return Err(AuthError::NotInitialized),
            AuthState::DataError => return Err(AuthError::DataDamaged),
        };
        let wrapped = profile
            .pin_wrapped_key
            .as_ref()
            .ok_or(AuthError::PinNotConfigured)?;
        let protected = STANDARD
            .decode(&wrapped.protected_device_secret)
            .map_err(|_| AuthError::InvalidPin)?;
        let device_secret = match self.device_protector.unprotect(&protected) {
            Ok(secret) => secret,
            Err(_) => return Err(record_failed_pin_attempt(&mut inner, now)),
        };
        match unwrap_pin_vault_key(pin, &device_secret, wrapped) {
            Ok(vault_key) => {
                reset_pin_attempts(&mut inner);
                inner.state = AuthState::Unlocked { profile, vault_key };
                Ok(())
            }
            Err(CryptoError::AuthenticationFailed | CryptoError::InvalidMetadata) => {
                Err(record_failed_pin_attempt(&mut inner, now))
            }
            Err(_) => Err(AuthError::LocalDataFailure),
        }
    }

    pub(crate) fn setup_pin(
        &self,
        current_password: &str,
        pin: &str,
        confirmation: &str,
    ) -> Result<(), AuthError> {
        self.replace_pin(current_password, pin, confirmation, false)
    }

    pub(crate) fn change_pin(
        &self,
        current_password: &str,
        pin: &str,
        confirmation: &str,
    ) -> Result<(), AuthError> {
        self.replace_pin(current_password, pin, confirmation, true)
    }

    fn replace_pin(
        &self,
        current_password: &str,
        pin: &str,
        confirmation: &str,
        require_existing: bool,
    ) -> Result<(), AuthError> {
        validate_pin(pin)?;
        if !constant_time_eq(pin.as_bytes(), confirmation.as_bytes()) {
            return Err(AuthError::PinConfirmationMismatch);
        }
        let mut inner = self.lock_inner();
        let (profile, vault_key) = match &mut inner.state {
            AuthState::Unlocked { profile, vault_key } => (profile, vault_key),
            AuthState::Locked(_) => return Err(AuthError::Unauthorized),
            AuthState::SetupRequired => return Err(AuthError::NotInitialized),
            AuthState::DataError => return Err(AuthError::DataDamaged),
        };
        if require_existing && profile.pin_wrapped_key.is_none() {
            return Err(AuthError::PinNotConfigured);
        }
        if !require_existing && profile.pin_wrapped_key.is_some() {
            return Err(AuthError::PinAlreadyConfigured);
        }
        verify_master_password(current_password, profile, vault_key)?;

        let mut device_secret = Zeroizing::new([0_u8; DEVICE_SECRET_LENGTH]);
        self.entropy
            .fill(device_secret.as_mut())
            .map_err(|_| AuthError::LocalDataFailure)?;
        let protected_device_secret = self
            .device_protector
            .protect(device_secret.as_ref())
            .map_err(|_| AuthError::DeviceProtectionUnavailable)?;
        let wrapped = wrap_pin_vault_key(
            pin,
            device_secret.as_ref(),
            &protected_device_secret,
            vault_key.expose(),
            self.kdf_params,
            self.entropy.as_ref(),
        )
        .map_err(|_| AuthError::LocalDataFailure)?;
        let replacement = profile.replacing_pin_wrapper(wrapped);
        self.store.replace(&replacement)?;
        *profile = replacement;
        reset_pin_attempts(&mut inner);
        Ok(())
    }

    pub(crate) fn remove_pin(&self, current_password: &str) -> Result<(), AuthError> {
        let mut inner = self.lock_inner();
        let (profile, vault_key) = match &mut inner.state {
            AuthState::Unlocked { profile, vault_key } => (profile, vault_key),
            AuthState::Locked(_) => return Err(AuthError::Unauthorized),
            AuthState::SetupRequired => return Err(AuthError::NotInitialized),
            AuthState::DataError => return Err(AuthError::DataDamaged),
        };
        if profile.pin_wrapped_key.is_none() {
            return Err(AuthError::PinNotConfigured);
        }
        verify_master_password(current_password, profile, vault_key)?;
        let replacement = profile.removing_pin_wrapper();
        self.store.replace(&replacement)?;
        *profile = replacement;
        reset_pin_attempts(&mut inner);
        Ok(())
    }

    pub(crate) fn lock(&self) -> LockOutcome {
        let mut inner = self.lock_inner();
        let previous = std::mem::replace(&mut inner.state, AuthState::DataError);
        let transitioned = matches!(&previous, AuthState::Unlocked { .. });
        inner.state = match previous {
            AuthState::Unlocked { profile, .. } => {
                reset_pin_attempts(&mut inner);
                AuthState::Locked(profile)
            }
            other => other,
        };
        let status = match inner.state {
            AuthState::SetupRequired => AuthStatus::SetupRequired,
            AuthState::Locked(_) => AuthStatus::Locked,
            AuthState::Unlocked { .. } => AuthStatus::Unlocked,
            AuthState::DataError => AuthStatus::DataError,
        };
        LockOutcome {
            status,
            transitioned,
        }
    }

    pub(crate) fn change_master_password(
        &self,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), AuthError> {
        validate_master_password(new_password)?;

        let mut inner = self.lock_inner();
        let (profile, vault_key) = match &mut inner.state {
            AuthState::Unlocked { profile, vault_key } => (profile, vault_key),
            AuthState::Locked(_) => return Err(AuthError::Unauthorized),
            AuthState::SetupRequired => return Err(AuthError::NotInitialized),
            AuthState::DataError => return Err(AuthError::DataDamaged),
        };

        verify_master_password(current_password, profile, vault_key)?;

        let wrapped_key = wrap_existing_vault_key(
            new_password,
            vault_key.expose(),
            self.kdf_params,
            self.entropy.as_ref(),
        )
        .map_err(|_| AuthError::LocalDataFailure)?;
        let replacement = profile.replacing_master_wrapper(wrapped_key);
        self.store.replace(&replacement)?;
        *profile = replacement;
        Ok(())
    }

    pub(crate) fn validate_reset_confirmation(&self, confirmation: &str) -> Result<(), AuthError> {
        if confirmation != RESET_CONFIRMATION {
            return Err(AuthError::InvalidResetConfirmation);
        }

        Ok(())
    }

    pub(crate) fn validate_authenticated_reset(
        &self,
        current_password: &str,
        confirmation: &str,
    ) -> Result<(), AuthError> {
        let inner = self.lock_inner();
        let (profile, vault_key) = match &inner.state {
            AuthState::Unlocked { profile, vault_key } => (profile, vault_key),
            AuthState::Locked(_) => return Err(AuthError::Unauthorized),
            AuthState::SetupRequired => return Err(AuthError::NotInitialized),
            AuthState::DataError => return Err(AuthError::DataDamaged),
        };
        self.validate_reset_confirmation(confirmation)?;

        let verified_key = match unwrap_vault_key(current_password, &profile.wrapped_key) {
            Ok(key) => key,
            Err(CryptoError::AuthenticationFailed) => {
                return Err(AuthError::InvalidCredentials);
            }
            Err(_) => return Err(AuthError::DataDamaged),
        };
        if verified_key.expose() != vault_key.expose() {
            return Err(AuthError::DataDamaged);
        }

        Ok(())
    }

    pub(crate) fn finish_reset(&self) -> Result<(), AuthError> {
        let mut inner = self.lock_inner();
        self.store.reset()?;
        inner.failed_attempts = 0;
        inner.last_failed_at = None;
        reset_pin_attempts(&mut inner);
        inner.state = AuthState::SetupRequired;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn reset_keynest(&self, confirmation: &str) -> Result<(), AuthError> {
        self.validate_reset_confirmation(confirmation)?;
        self.finish_reset()
    }

    // Keep the vault key inside the authenticated Rust boundary.
    pub(crate) fn require_vault_key<T>(
        &self,
        operation: impl FnOnce(&[u8; 32]) -> T,
    ) -> Result<T, AuthError> {
        let inner = self.lock_inner();
        match &inner.state {
            AuthState::Unlocked { vault_key, .. } => Ok(operation(vault_key.expose())),
            _ => Err(AuthError::Unauthorized),
        }
    }

    fn lock_inner(&self) -> MutexGuard<'_, AuthInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub(crate) enum AuthError {
    #[error("the master password must contain at least 12 characters")]
    PasswordTooShort,
    #[error("Master Password is too weak.")]
    PasswordTooWeak,
    #[error("KeyNest already has a master password")]
    AlreadyInitialized,
    #[error("KeyNest needs a master password before it can be unlocked")]
    NotInitialized,
    #[error("the master password is incorrect")]
    InvalidCredentials,
    #[error("recovery is not configured for this profile")]
    RecoveryNotConfigured,
    #[error("the recovery key is incorrect")]
    InvalidRecoveryKey,
    #[error("wait before trying again")]
    Throttled { retry_after_ms: u64 },
    #[error("the device PIN must contain exactly six digits")]
    InvalidPinFormat,
    #[error("the device PIN confirmation does not match")]
    PinConfirmationMismatch,
    #[error("the device PIN is incorrect")]
    InvalidPin,
    #[error("device PIN unlock is not configured")]
    PinNotConfigured,
    #[error("device PIN unlock is already configured")]
    PinAlreadyConfigured,
    #[error("wait before trying the device PIN again")]
    PinThrottled { retry_after_ms: u64 },
    #[error("use the Master Password for this locked session")]
    PinRequiresMasterPassword,
    #[error("Windows device protection is unavailable")]
    DeviceProtectionUnavailable,
    #[error("type RESET KEYNEST exactly to confirm")]
    InvalidResetConfirmation,
    #[error("KeyNest is locked")]
    Unauthorized,
    #[error("the local encrypted profile is damaged")]
    DataDamaged,
    #[error("local KeyNest data could not be accessed")]
    LocalDataFailure,
}

fn verify_master_password(
    current_password: &str,
    profile: &StoredProfile,
    vault_key: &VaultKey,
) -> Result<(), AuthError> {
    let verified_key = match unwrap_vault_key(current_password, &profile.wrapped_key) {
        Ok(key) => key,
        Err(CryptoError::AuthenticationFailed) => return Err(AuthError::InvalidCredentials),
        Err(_) => return Err(AuthError::DataDamaged),
    };
    if verified_key.expose() != vault_key.expose() {
        return Err(AuthError::DataDamaged);
    }
    Ok(())
}

fn validate_pin(pin: &str) -> Result<(), AuthError> {
    if pin.len() == 6 && pin.bytes().all(|byte| byte.is_ascii_digit()) {
        Ok(())
    } else {
        Err(AuthError::InvalidPinFormat)
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        difference |= usize::from(
            left.get(index).copied().unwrap_or(0) ^ right.get(index).copied().unwrap_or(0),
        );
    }
    difference == 0
}

impl From<StorageError> for AuthError {
    fn from(error: StorageError) -> Self {
        match error {
            StorageError::DamagedProfile => AuthError::DataDamaged,
            StorageError::AlreadyExists | StorageError::Serialization(_) | StorageError::Io(_) => {
                AuthError::LocalDataFailure
            }
        }
    }
}

fn attempt_delay(failed_attempts: u32) -> Duration {
    Duration::from_secs(match failed_attempts {
        0..=3 => 0,
        4 => 2,
        5 => 5,
        6 => 10,
        _ => 30,
    })
}

fn check_attempt_delay(inner: &AuthInner, now: Instant) -> Result<(), AuthError> {
    if let Some(last_failed_at) = inner.last_failed_at {
        let remaining = attempt_delay(inner.failed_attempts)
            .saturating_sub(now.saturating_duration_since(last_failed_at));
        if !remaining.is_zero() {
            // Round up so sub-millisecond remainder does not enable an early retry.
            return Err(AuthError::Throttled {
                retry_after_ms: remaining.as_nanos().div_ceil(1_000_000) as u64,
            });
        }
    }
    Ok(())
}

fn record_failed_attempt(inner: &mut AuthInner, now: Instant, error: AuthError) -> AuthError {
    inner.failed_attempts = inner.failed_attempts.saturating_add(1);
    // Sample after verification and after acquiring the auth lock: neither KDF
    // time nor queued requests may consume the cooldown. No sleep/deadline addition.
    inner.last_failed_at = Some(now);
    check_attempt_delay(inner, now).err().unwrap_or(error)
}

fn check_pin_attempt_delay(inner: &AuthInner, now: Instant) -> Result<(), AuthError> {
    if let Some(last_failed_at) = inner.pin_last_failed_at {
        let delay = if inner.pin_failed_attempts == 4 {
            Duration::from_secs(2)
        } else {
            Duration::ZERO
        };
        let remaining = delay.saturating_sub(now.saturating_duration_since(last_failed_at));
        if !remaining.is_zero() {
            return Err(AuthError::PinThrottled {
                retry_after_ms: remaining.as_nanos().div_ceil(1_000_000) as u64,
            });
        }
    }
    Ok(())
}

fn record_failed_pin_attempt(inner: &mut AuthInner, now: Instant) -> AuthError {
    inner.pin_failed_attempts = inner.pin_failed_attempts.saturating_add(1);
    inner.pin_last_failed_at = Some(now);
    if inner.pin_failed_attempts >= 5 {
        inner.pin_disabled = true;
        return AuthError::PinRequiresMasterPassword;
    }
    check_pin_attempt_delay(inner, now)
        .err()
        .unwrap_or(AuthError::InvalidPin)
}

fn reset_pin_attempts(inner: &mut AuthInner) {
    inner.pin_failed_attempts = 0;
    inner.pin_last_failed_at = None;
    inner.pin_disabled = false;
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            atomic::{AtomicBool, AtomicU8, Ordering},
            Arc,
        },
        time::{Duration, Instant},
    };

    use super::*;
    use crate::security::{
        crypto::{CryptoError, EntropySource, KdfParams},
        device_protection::{DeviceProtectionError, DeviceProtector},
        storage::ProfileStore,
    };

    struct FakeDeviceProtector(u8);

    impl DeviceProtector for FakeDeviceProtector {
        fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, DeviceProtectionError> {
            let mut protected = Vec::with_capacity(plaintext.len() + 2);
            protected.extend_from_slice(&[0x4b, self.0]);
            protected.extend(plaintext.iter().map(|byte| byte ^ self.0));
            Ok(protected)
        }

        fn unprotect(
            &self,
            ciphertext: &[u8],
        ) -> Result<Zeroizing<Vec<u8>>, DeviceProtectionError> {
            if ciphertext.get(..2) != Some([0x4b, self.0].as_slice()) {
                return Err(DeviceProtectionError::Unavailable);
            }
            Ok(Zeroizing::new(
                ciphertext[2..].iter().map(|byte| byte ^ self.0).collect(),
            ))
        }
    }

    struct FixedEntropy;

    impl EntropySource for FixedEntropy {
        fn fill(&self, destination: &mut [u8]) -> Result<(), CryptoError> {
            let length = destination.len() as u8;
            for (index, byte) in destination.iter_mut().enumerate() {
                *byte = length.wrapping_add(index as u8);
            }
            Ok(())
        }
    }

    struct AlternateEntropy;

    impl EntropySource for AlternateEntropy {
        fn fill(&self, destination: &mut [u8]) -> Result<(), CryptoError> {
            let length = destination.len() as u8;
            for (index, byte) in destination.iter_mut().enumerate() {
                *byte = length.wrapping_add(index as u8).wrapping_add(1);
            }
            Ok(())
        }
    }

    struct CountingEntropy(AtomicU8);

    impl CountingEntropy {
        fn new() -> Self {
            Self(AtomicU8::new(1))
        }
    }

    impl EntropySource for CountingEntropy {
        fn fill(&self, destination: &mut [u8]) -> Result<(), CryptoError> {
            let seed = self.0.fetch_add(1, Ordering::SeqCst);
            for (index, byte) in destination.iter_mut().enumerate() {
                *byte = seed.wrapping_add(index as u8);
            }
            Ok(())
        }
    }

    struct SwitchableEntropy {
        fail: AtomicBool,
    }

    impl SwitchableEntropy {
        fn working() -> Self {
            Self {
                fail: AtomicBool::new(false),
            }
        }

        fn fail(&self) {
            self.fail.store(true, Ordering::SeqCst);
        }
    }

    impl EntropySource for SwitchableEntropy {
        fn fill(&self, destination: &mut [u8]) -> Result<(), CryptoError> {
            if self.fail.load(Ordering::SeqCst) {
                return Err(CryptoError::EntropyUnavailable);
            }

            FixedEntropy.fill(destination)
        }
    }

    struct AuthFixture {
        temp: tempfile::TempDir,
        service: AuthService,
    }

    impl AuthFixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let params = KdfParams::testing();
            let store = ProfileStore::new(temp.path().to_path_buf());
            let service = AuthService::load_with_device_protector(
                store,
                params,
                Arc::new(FixedEntropy),
                Arc::new(FakeDeviceProtector(0x5a)),
            );
            Self { temp, service }
        }

        fn with_profile_bytes(bytes: &[u8]) -> Self {
            let temp = tempfile::tempdir().unwrap();
            std::fs::write(temp.path().join("profile.json"), bytes).unwrap();
            let params = KdfParams::testing();
            let store = ProfileStore::new(temp.path().to_path_buf());
            let service = AuthService::load_with_device_protector(
                store,
                params,
                Arc::new(FixedEntropy),
                Arc::new(FakeDeviceProtector(0x5a)),
            );
            Self { temp, service }
        }

        fn create_unlocked_with_vault(&self) {
            self.service
                .create_master_password("a secure master password")
                .unwrap();
            std::fs::write(self.temp.path().join("vault.enc"), b"encrypted vault").unwrap();
        }

        fn assert_security_files_exist(&self) {
            assert!(self.temp.path().join("profile.json").is_file());
            assert!(self.temp.path().join("vault.enc").is_file());
        }
    }

    #[test]
    fn stored_kdf_survives_new_defaults_and_password_change_only_rewraps_master_key() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_path_buf());
        let old_defaults = KdfParams::production();
        let new_defaults = KdfParams {
            iterations: 4,
            ..old_defaults
        };
        let service = AuthService::load(store.clone(), old_defaults, Arc::new(FixedEntropy));
        let recovery_key = service
            .create_master_password("a secure master password")
            .unwrap();
        let setup_profile = match store.load().unwrap() {
            ProfileLoad::Valid(profile) => profile,
            ProfileLoad::Missing => panic!("setup must persist a profile"),
        };
        assert_eq!(setup_profile.wrapped_key.params, old_defaults);
        assert_eq!(
            setup_profile.recovery_wrapped_key.as_ref().unwrap().params,
            old_defaults
        );
        let original_key = service
            .require_vault_key(|key| Zeroizing::new(*key))
            .unwrap();
        let original_bytes = std::fs::read(store.profile_path()).unwrap();
        service.lock();
        drop(service);

        let reloaded = AuthService::load(store.clone(), new_defaults, Arc::new(FixedEntropy));
        assert_eq!(reloaded.status(), AuthStatus::Locked);
        reloaded.unlock("a secure master password").unwrap();
        assert!(reloaded
            .require_vault_key(|key| key == original_key.as_ref())
            .unwrap());
        assert_eq!(std::fs::read(store.profile_path()).unwrap(), original_bytes);
        reloaded
            .change_master_password("a secure master password", "a replacement secure password")
            .unwrap();
        let changed = match store.load().unwrap() {
            ProfileLoad::Valid(profile) => profile,
            ProfileLoad::Missing => panic!("password change must retain the profile"),
        };
        assert_eq!(changed.wrapped_key.params, new_defaults);
        assert_eq!(
            changed.recovery_wrapped_key,
            setup_profile.recovery_wrapped_key
        );
        assert!(reloaded
            .require_vault_key(|key| key == original_key.as_ref())
            .unwrap());
        reloaded.lock();
        drop(reloaded);
        // Also model loading a newer wrapper with older creation defaults.
        let reloaded = AuthService::load(store, old_defaults, Arc::new(FixedEntropy));
        reloaded.unlock("a replacement secure password").unwrap();
        assert!(reloaded
            .require_vault_key(|key| key == original_key.as_ref())
            .unwrap());
        // The untouched recovery wrapper still uses its own stored KDF parameters.
        assert!(
            unwrap_recovery_vault_key(
                &recovery_key,
                changed.recovery_wrapped_key.as_ref().unwrap()
            )
            .unwrap()
            .expose()
                == original_key.as_ref()
        );
    }

    #[test]
    fn existing_weak_password_remains_unlockable_after_policy_upgrade() {
        let temp = tempfile::tempdir().unwrap();
        let params = KdfParams::testing();
        let store = ProfileStore::new(temp.path().to_path_buf());
        // Model a pre-policy profile without using the newly restricted setup API.
        let password = "123456789012";
        let (wrapped, _) = wrap_new_vault_key(password, params, &FixedEntropy).unwrap();
        store.create(&StoredProfile::new(wrapped)).unwrap();
        let before = std::fs::read(temp.path().join("profile.json")).unwrap();
        let service = AuthService::load(store, params, Arc::new(FixedEntropy));
        service.unlock(password).unwrap();
        assert_eq!(service.status(), AuthStatus::Unlocked);
        assert_eq!(
            std::fs::read(temp.path().join("profile.json")).unwrap(),
            before
        );
    }

    #[test]
    fn first_run_create_lock_and_unlock_round_trip() {
        let fixture = AuthFixture::new();
        assert_eq!(fixture.service.status(), AuthStatus::SetupRequired);

        fixture
            .service
            .create_master_password("a secure master password")
            .unwrap();
        assert_eq!(fixture.service.status(), AuthStatus::Unlocked);

        fixture.service.lock();
        assert_eq!(fixture.service.status(), AuthStatus::Locked);
        assert_eq!(
            fixture.service.unlock("wrong master password"),
            Err(AuthError::InvalidCredentials),
        );
        assert_eq!(fixture.service.status(), AuthStatus::Locked);

        fixture.service.unlock("a secure master password").unwrap();
        assert_eq!(fixture.service.status(), AuthStatus::Unlocked);
    }

    #[test]
    fn lock_outcome_is_true_only_for_the_unlocked_to_locked_transition() {
        let fixture = AuthFixture::new();
        assert_eq!(
            fixture.service.lock(),
            LockOutcome {
                status: AuthStatus::SetupRequired,
                transitioned: false,
            }
        );
        fixture
            .service
            .create_master_password("a secure master password")
            .unwrap();

        assert_eq!(
            fixture.service.lock(),
            LockOutcome {
                status: AuthStatus::Locked,
                transitioned: true,
            }
        );
        assert_eq!(
            fixture.service.lock(),
            LockOutcome {
                status: AuthStatus::Locked,
                transitioned: false,
            }
        );
    }

    #[test]
    fn short_password_is_rejected_by_rust() {
        let fixture = AuthFixture::new();

        assert_eq!(
            fixture.service.create_master_password("too short"),
            Err(AuthError::PasswordTooShort),
        );
        assert_eq!(fixture.service.status(), AuthStatus::SetupRequired);
    }

    #[test]
    fn damaged_profile_loads_fail_closed() {
        let fixture = AuthFixture::with_profile_bytes(b"not-json");

        assert_eq!(fixture.service.status(), AuthStatus::DataError);
        assert_eq!(
            fixture.service.require_vault_key(|_| ()),
            Err(AuthError::Unauthorized),
        );
    }

    #[test]
    fn reset_confirmation_rejects_every_non_exact_phrase_without_mutation() {
        let fixture = AuthFixture::new();
        fixture.create_unlocked_with_vault();

        for confirmation in [
            "RESET",
            "reset keynest",
            "Reset KeyNest",
            " RESET KEYNEST",
            "RESET KEYNEST ",
            "DELETE KEYNEST",
            "",
        ] {
            assert_eq!(
                fixture.service.validate_reset_confirmation(confirmation),
                Err(AuthError::InvalidResetConfirmation),
                "unexpectedly accepted {confirmation:?}",
            );
            assert_eq!(fixture.service.status(), AuthStatus::Unlocked);
            fixture.assert_security_files_exist();
        }
    }

    #[test]
    fn reset_confirmation_validation_does_not_delete_or_change_state() {
        let fixture = AuthFixture::new();
        fixture.create_unlocked_with_vault();
        fixture.service.lock();

        fixture
            .service
            .validate_reset_confirmation("RESET KEYNEST")
            .unwrap();

        assert_eq!(fixture.service.status(), AuthStatus::Locked);
        fixture.assert_security_files_exist();
    }

    #[test]
    fn authenticated_reset_wrong_password_preserves_files_and_state() {
        let fixture = AuthFixture::new();
        fixture.create_unlocked_with_vault();

        assert_eq!(
            fixture
                .service
                .validate_authenticated_reset("wrong master password", "RESET KEYNEST"),
            Err(AuthError::InvalidCredentials),
        );

        assert_eq!(fixture.service.status(), AuthStatus::Unlocked);
        fixture.assert_security_files_exist();
    }

    #[test]
    fn authenticated_reset_wrong_confirmation_preserves_files_and_state() {
        let fixture = AuthFixture::new();
        fixture.create_unlocked_with_vault();

        assert_eq!(
            fixture
                .service
                .validate_authenticated_reset("a secure master password", "RESET"),
            Err(AuthError::InvalidResetConfirmation),
        );

        assert_eq!(fixture.service.status(), AuthStatus::Unlocked);
        fixture.assert_security_files_exist();
    }

    #[test]
    fn authenticated_reset_while_locked_is_unauthorized_without_mutation() {
        let fixture = AuthFixture::new();
        fixture.create_unlocked_with_vault();
        fixture.service.lock();

        assert_eq!(
            fixture
                .service
                .validate_authenticated_reset("a secure master password", "RESET KEYNEST"),
            Err(AuthError::Unauthorized),
        );

        assert_eq!(fixture.service.status(), AuthStatus::Locked);
        fixture.assert_security_files_exist();
    }

    #[test]
    fn authenticated_reset_validation_does_not_delete_or_change_state() {
        let fixture = AuthFixture::new();
        fixture.create_unlocked_with_vault();
        let live_key = fixture.service.require_vault_key(|key| *key).unwrap();

        fixture
            .service
            .validate_authenticated_reset("a secure master password", "RESET KEYNEST")
            .unwrap();

        assert_eq!(fixture.service.status(), AuthStatus::Unlocked);
        assert_eq!(
            fixture.service.require_vault_key(|key| *key).unwrap(),
            live_key
        );
        fixture.assert_security_files_exist();
    }

    #[test]
    fn authenticated_reset_rejects_a_live_session_key_mismatch_without_mutation() {
        let fixture = AuthFixture::new();
        fixture.create_unlocked_with_vault();
        let profile_bytes = std::fs::read(fixture.temp.path().join("profile.json")).unwrap();
        let (_, mismatched_live_key) = wrap_new_vault_key(
            "unrelated secure master password",
            KdfParams::testing(),
            &AlternateEntropy,
        )
        .unwrap();
        {
            let mut inner = fixture.service.lock_inner();
            let profile = match &inner.state {
                AuthState::Unlocked { profile, .. } => profile.clone(),
                _ => panic!("fixture must be unlocked"),
            };
            inner.state = AuthState::Unlocked {
                profile,
                vault_key: mismatched_live_key,
            };
        }

        assert_eq!(
            fixture
                .service
                .validate_authenticated_reset("a secure master password", "RESET KEYNEST"),
            Err(AuthError::DataDamaged),
        );

        assert_eq!(fixture.service.status(), AuthStatus::Unlocked);
        assert_eq!(
            std::fs::read(fixture.temp.path().join("profile.json")).unwrap(),
            profile_bytes
        );
        fixture.assert_security_files_exist();
    }

    #[test]
    fn finish_reset_deletes_security_files_and_changes_state_only_on_success() {
        let fixture = AuthFixture::new();
        fixture.create_unlocked_with_vault();

        fixture.service.finish_reset().unwrap();

        assert_eq!(fixture.service.status(), AuthStatus::SetupRequired);
        assert!(!fixture.temp.path().join("profile.json").exists());
        assert!(!fixture.temp.path().join("vault.enc").exists());
    }

    #[test]
    fn failed_finish_reset_preserves_the_in_memory_retry_path() {
        let fixture = AuthFixture::new();
        fixture
            .service
            .create_master_password("a secure master password")
            .unwrap();
        std::fs::create_dir(fixture.temp.path().join("vault.enc")).unwrap();
        let live_key = fixture.service.require_vault_key(|key| *key).unwrap();

        assert_eq!(
            fixture.service.finish_reset(),
            Err(AuthError::LocalDataFailure)
        );
        assert_eq!(fixture.service.status(), AuthStatus::Unlocked);
        assert_eq!(
            fixture.service.require_vault_key(|key| *key).unwrap(),
            live_key
        );
        assert!(fixture.temp.path().join("profile.json").is_file());

        std::fs::remove_dir(fixture.temp.path().join("vault.enc")).unwrap();
        fixture.service.finish_reset().unwrap();
        assert_eq!(fixture.service.status(), AuthStatus::SetupRequired);
    }

    #[test]
    fn reset_compatibility_wrapper_requires_reset_keynest() {
        let fixture = AuthFixture::new();
        fixture.create_unlocked_with_vault();

        assert_eq!(
            fixture.service.reset_keynest("RESET"),
            Err(AuthError::InvalidResetConfirmation),
        );
        assert_eq!(fixture.service.status(), AuthStatus::Unlocked);
        fixture.assert_security_files_exist();

        fixture.service.reset_keynest("RESET KEYNEST").unwrap();
        assert_eq!(fixture.service.status(), AuthStatus::SetupRequired);
    }

    #[test]
    fn unlock_and_recovery_enforce_the_same_capped_schedule_and_reset_on_success() {
        for recovery in [false, true] {
            let fixture = AuthFixture::new();
            let key = fixture
                .service
                .create_master_password("a secure master password")
                .unwrap();
            std::fs::write(
                fixture.temp.path().join("vault.enc"),
                b"unchanged encrypted fixture",
            )
            .unwrap();
            fixture.service.lock();
            let profile = std::fs::read(fixture.service.store.profile_path()).unwrap();
            let mut now = Instant::now();
            for (index, seconds) in [0, 0, 0, 2, 5, 10, 30, 30].into_iter().enumerate() {
                let result = if recovery {
                    fixture
                        .service
                        .recover_master_password_with_clock(
                            "KN-R1-FFFF-FFFF-FFFF-FFFF-FFFF-FFFF-FFFF-FFFF",
                            "a replacement master password",
                            || now,
                        )
                        .map(|_| ())
                } else {
                    fixture.service.unlock_at("incorrect password", now)
                };
                let expected = if seconds == 0 {
                    if recovery {
                        AuthError::InvalidRecoveryKey
                    } else {
                        AuthError::InvalidCredentials
                    }
                } else {
                    AuthError::Throttled {
                        retry_after_ms: seconds * 1000,
                    }
                };
                assert_eq!(result, Err(expected));
                if seconds > 0 {
                    let count = fixture.service.lock_inner().failed_attempts;
                    assert_eq!(
                        fixture.service.unlock_at("a secure master password", now),
                        Err(AuthError::Throttled {
                            retry_after_ms: seconds * 1000
                        })
                    );
                    assert_eq!(fixture.service.lock_inner().failed_attempts, count);
                }
                assert_eq!(
                    fixture.service.lock_inner().failed_attempts,
                    index as u32 + 1
                );
                now += Duration::from_secs(seconds);
            }
            assert_eq!(
                std::fs::read(fixture.service.store.profile_path()).unwrap(),
                profile
            );
            assert_eq!(
                std::fs::read(fixture.temp.path().join("vault.enc")).unwrap(),
                b"unchanged encrypted fixture"
            );
            if recovery {
                assert!(fixture
                    .service
                    .recover_master_password_with_clock(
                        &key,
                        "a replacement master password",
                        || now,
                    )
                    .is_ok());
            } else {
                fixture
                    .service
                    .unlock_at("a secure master password", now)
                    .unwrap();
            }
            assert_eq!(fixture.service.lock_inner().failed_attempts, 0);
            assert!(fixture.service.lock_inner().last_failed_at.is_none());
            fixture.service.lock();
            assert_eq!(
                fixture.service.unlock_at("incorrect password", now),
                Err(AuthError::InvalidCredentials)
            );
        }
    }

    #[test]
    fn alternating_flows_and_invalid_reset_do_not_bypass_the_shared_cooldown() {
        let fixture = AuthFixture::new();
        let key = fixture
            .service
            .create_master_password("a secure master password")
            .unwrap();
        fixture.service.lock();
        let before = std::fs::read(fixture.service.store.profile_path()).unwrap();
        let now = Instant::now();
        for _ in 0..3 {
            assert_eq!(
                fixture.service.unlock_at("wrong", now),
                Err(AuthError::InvalidCredentials)
            );
        }
        assert!(matches!(
            fixture.service.recover_master_password_with_clock(
                "malformed key",
                "a replacement master password",
                || now,
            ),
            Err(AuthError::Throttled {
                retry_after_ms: 2000
            })
        ));
        assert!(matches!(
            fixture.service.recover_master_password_with_clock(
                &key,
                "a replacement master password",
                || now,
            ),
            Err(AuthError::Throttled {
                retry_after_ms: 2000
            })
        ));
        assert_eq!(
            fixture.service.reset_keynest("RESET"),
            Err(AuthError::InvalidResetConfirmation)
        );
        assert_eq!(
            fixture
                .service
                .validate_authenticated_reset("a secure master password", "RESET KEYNEST"),
            Err(AuthError::Unauthorized)
        );
        fixture.service.lock();
        assert!(fixture.service.recovery_status().unwrap().configured);
        assert_eq!(
            fixture.service.unlock_at("a secure master password", now),
            Err(AuthError::Throttled {
                retry_after_ms: 2000
            })
        );
        assert_eq!(
            std::fs::read(fixture.service.store.profile_path()).unwrap(),
            before
        );
        assert_eq!(fixture.service.lock_inner().failed_attempts, 4);
        fixture
            .service
            .unlock_at("a secure master password", now + Duration::from_secs(2))
            .unwrap();
    }

    #[test]
    fn cooldown_starts_after_verification_and_uses_fresh_clock_samples() {
        use std::cell::Cell;
        let fixture = AuthFixture::new();
        fixture
            .service
            .create_master_password("a secure master password")
            .unwrap();
        fixture.service.lock();
        let start = Instant::now();
        for _ in 0..3 {
            let _ = fixture.service.unlock_at("wrong", start);
        }
        let samples = Cell::new(0);
        let result = fixture.service.unlock_with_clock("wrong", || {
            let sample = samples.get();
            samples.set(sample + 1);
            start + Duration::from_secs(if sample == 0 { 0 } else { 15 })
        });
        assert_eq!(samples.get(), 2);
        assert_eq!(
            result,
            Err(AuthError::Throttled {
                retry_after_ms: 2000
            })
        );
        assert_eq!(
            fixture
                .service
                .unlock_at("a secure master password", start + Duration::from_secs(15)),
            Err(AuthError::Throttled {
                retry_after_ms: 2000
            })
        );
        fixture
            .service
            .unlock_at("a secure master password", start + Duration::from_secs(17))
            .unwrap();
    }

    #[test]
    fn saturated_counters_and_submillisecond_remainders_stay_bounded() {
        let fixture = AuthFixture::new();
        let now = Instant::now();
        let mut inner = fixture.service.lock_inner();
        inner.failed_attempts = u32::MAX;
        assert_eq!(
            record_failed_attempt(&mut inner, now, AuthError::InvalidCredentials),
            AuthError::Throttled {
                retry_after_ms: 30_000
            }
        );
        assert_eq!(inner.failed_attempts, u32::MAX);
        assert_eq!(
            check_attempt_delay(&inner, now + Duration::from_micros(29_999_999)),
            Err(AuthError::Throttled { retry_after_ms: 1 })
        );
        assert_eq!(
            check_attempt_delay(&inner, now + Duration::from_secs(30)),
            Ok(())
        );
        assert_eq!(
            check_attempt_delay(&inner, now - Duration::from_secs(1)),
            Err(AuthError::Throttled {
                retry_after_ms: 30_000
            })
        );
    }

    #[test]
    fn password_change_success_preserves_vault_key_and_unlocked_session() {
        let fixture = AuthFixture::new();
        fixture
            .service
            .create_master_password("old secure master password")
            .unwrap();
        let before = fixture.service.require_vault_key(|key| *key).unwrap();

        fixture
            .service
            .change_master_password("old secure master password", "new secure master password")
            .unwrap();

        assert_eq!(fixture.service.status(), AuthStatus::Unlocked);
        assert_eq!(
            fixture.service.require_vault_key(|key| *key).unwrap(),
            before
        );
        fixture.service.lock();
        assert_eq!(
            fixture.service.unlock("old secure master password"),
            Err(AuthError::InvalidCredentials)
        );
        fixture
            .service
            .unlock("new secure master password")
            .unwrap();
    }

    #[test]
    fn password_change_short_new_password_is_rejected_without_mutation() {
        let fixture = AuthFixture::new();
        fixture
            .service
            .create_master_password("old secure master password")
            .unwrap();
        let before_key = fixture.service.require_vault_key(|key| *key).unwrap();
        let before_profile = std::fs::read(fixture.service.store.profile_path()).unwrap();

        assert_eq!(
            fixture
                .service
                .change_master_password("old secure master password", "too short"),
            Err(AuthError::PasswordTooShort)
        );

        assert_eq!(fixture.service.status(), AuthStatus::Unlocked);
        assert_eq!(
            fixture.service.require_vault_key(|key| *key).unwrap(),
            before_key
        );
        assert_eq!(
            std::fs::read(fixture.service.store.profile_path()).unwrap(),
            before_profile
        );
        fixture.service.lock();
        fixture
            .service
            .unlock("old secure master password")
            .unwrap();
    }

    #[test]
    fn password_change_incorrect_current_password_is_rejected_without_mutation() {
        let fixture = AuthFixture::new();
        fixture
            .service
            .create_master_password("old secure master password")
            .unwrap();
        let before_key = fixture.service.require_vault_key(|key| *key).unwrap();
        let before_profile = std::fs::read(fixture.service.store.profile_path()).unwrap();

        assert_eq!(
            fixture.service.change_master_password(
                "wrong secure master password",
                "new secure master password"
            ),
            Err(AuthError::InvalidCredentials)
        );

        assert_eq!(fixture.service.status(), AuthStatus::Unlocked);
        assert_eq!(
            fixture.service.require_vault_key(|key| *key).unwrap(),
            before_key
        );
        assert_eq!(
            std::fs::read(fixture.service.store.profile_path()).unwrap(),
            before_profile
        );
        fixture.service.lock();
        fixture
            .service
            .unlock("old secure master password")
            .unwrap();
    }

    #[test]
    fn password_change_locked_call_is_unauthorized() {
        let fixture = AuthFixture::new();
        fixture
            .service
            .create_master_password("old secure master password")
            .unwrap();
        fixture.service.lock();

        assert_eq!(
            fixture
                .service
                .change_master_password("old secure master password", "new secure master password"),
            Err(AuthError::Unauthorized)
        );

        fixture
            .service
            .unlock("old secure master password")
            .unwrap();
    }

    #[test]
    fn password_change_replace_failure_preserves_disk_password_and_unlocked_session() {
        let fixture = AuthFixture::new();
        fixture
            .service
            .create_master_password("old secure master password")
            .unwrap();
        let before_key = fixture.service.require_vault_key(|key| *key).unwrap();
        let before_profile = std::fs::read(fixture.service.store.profile_path()).unwrap();
        fixture.service.store.fail_next_replace_for_test();

        assert_eq!(
            fixture
                .service
                .change_master_password("old secure master password", "new secure master password"),
            Err(AuthError::LocalDataFailure)
        );

        assert_eq!(fixture.service.status(), AuthStatus::Unlocked);
        assert_eq!(
            fixture.service.require_vault_key(|key| *key).unwrap(),
            before_key
        );
        assert_eq!(
            std::fs::read(fixture.service.store.profile_path()).unwrap(),
            before_profile
        );
        fixture.service.lock();
        fixture
            .service
            .unlock("old secure master password")
            .unwrap();
    }

    #[test]
    fn password_change_entropy_failure_preserves_disk_password_key_and_unlocked_session() {
        let temp = tempfile::tempdir().unwrap();
        let params = KdfParams::testing();
        let store = ProfileStore::new(temp.path().to_path_buf());
        let entropy = Arc::new(SwitchableEntropy::working());
        let service = AuthService::load(store, params, entropy.clone());
        service
            .create_master_password("old secure master password")
            .unwrap();
        let before_key = service.require_vault_key(|key| *key).unwrap();
        let before_profile = std::fs::read(service.store.profile_path()).unwrap();
        entropy.fail();

        assert_eq!(
            service
                .change_master_password("old secure master password", "new secure master password"),
            Err(AuthError::LocalDataFailure)
        );

        assert_eq!(service.status(), AuthStatus::Unlocked);
        assert_eq!(service.require_vault_key(|key| *key).unwrap(), before_key);
        assert_eq!(
            std::fs::read(service.store.profile_path()).unwrap(),
            before_profile
        );
        service.lock();
        service.unlock("old secure master password").unwrap();
        assert_eq!(service.require_vault_key(|key| *key).unwrap(), before_key);
    }

    #[test]
    fn password_change_live_key_mismatch_fails_closed_without_storage_or_session_mutation() {
        let fixture = AuthFixture::new();
        fixture
            .service
            .create_master_password("old secure master password")
            .unwrap();
        let before_profile = std::fs::read(fixture.service.store.profile_path()).unwrap();
        let (_, mismatched_live_key) = wrap_new_vault_key(
            "unrelated secure master password",
            KdfParams::testing(),
            &AlternateEntropy,
        )
        .unwrap();
        let mismatched_key_bytes = *mismatched_live_key.expose();
        {
            let mut inner = fixture.service.lock_inner();
            let profile = match &inner.state {
                AuthState::Unlocked { profile, .. } => profile.clone(),
                _ => panic!("fixture must be unlocked"),
            };
            inner.state = AuthState::Unlocked {
                profile,
                vault_key: mismatched_live_key,
            };
        }

        assert_eq!(
            fixture
                .service
                .change_master_password("old secure master password", "new secure master password"),
            Err(AuthError::DataDamaged)
        );

        assert_eq!(fixture.service.status(), AuthStatus::Unlocked);
        assert_eq!(
            fixture.service.require_vault_key(|key| *key).unwrap(),
            mismatched_key_bytes
        );
        assert_eq!(
            std::fs::read(fixture.service.store.profile_path()).unwrap(),
            before_profile
        );
        fixture.service.lock();
        fixture
            .service
            .unlock("old secure master password")
            .unwrap();
        assert_ne!(
            fixture.service.require_vault_key(|key| *key).unwrap(),
            mismatched_key_bytes
        );
    }

    #[test]
    fn fresh_setup_persists_two_wrappers_for_the_exact_same_vault_key_only() {
        use crate::security::crypto::unwrap_recovery_vault_key;

        let temp = tempfile::tempdir().unwrap();
        let params = KdfParams::testing();
        let store = ProfileStore::new(temp.path().to_path_buf());
        let service = AuthService::load(store.clone(), params, Arc::new(CountingEntropy::new()));
        let recovery_key = service
            .create_master_password("a secure master password")
            .unwrap();
        let live_key = service.require_vault_key(|key| *key).unwrap();
        let ProfileLoad::Valid(profile) = store.load().unwrap() else {
            panic!("profile must exist")
        };
        let password_key =
            unwrap_vault_key("a secure master password", &profile.wrapped_key).unwrap();
        let recovery_wrapped = profile.recovery_wrapped_key.as_ref().unwrap();
        let recovered_key = unwrap_recovery_vault_key(&recovery_key, recovery_wrapped).unwrap();

        assert_eq!(profile.format_version, 2);
        assert_ne!(profile.wrapped_key.salt, recovery_wrapped.salt);
        assert_ne!(profile.wrapped_key.nonce, recovery_wrapped.nonce);
        assert_eq!(password_key.expose(), &live_key);
        assert_eq!(recovered_key.expose(), &live_key);
        assert!(service.recovery_status().unwrap().configured);
        let profile_bytes = std::fs::read(store.profile_path()).unwrap();
        assert!(!profile_bytes
            .windows(recovery_key.len())
            .any(|window| window == recovery_key.as_bytes()));
    }

    #[test]
    fn legacy_profile_unlocks_and_reports_recovery_not_configured() {
        let temp = tempfile::tempdir().unwrap();
        let params = KdfParams::testing();
        let store = ProfileStore::new(temp.path().to_path_buf());
        let (wrapped, expected_key) =
            wrap_new_vault_key("legacy master password", params, &CountingEntropy::new()).unwrap();
        store.create(&StoredProfile::new(wrapped)).unwrap();
        std::fs::write(temp.path().join("vault.enc"), b"legacy encrypted vault").unwrap();
        let service = AuthService::load(store, params, Arc::new(CountingEntropy::new()));

        assert!(!service.recovery_status().unwrap().configured);
        assert_eq!(
            service.recover_master_password(
                "KN-R1-FFFF-FFFF-FFFF-FFFF-FFFF-FFFF-FFFF-FFFF",
                "replacement master password"
            ),
            Err(AuthError::RecoveryNotConfigured)
        );
        service.unlock("legacy master password").unwrap();
        assert_eq!(
            service.require_vault_key(|key| *key).unwrap(),
            *expected_key.expose()
        );
        assert_eq!(
            std::fs::read(temp.path().join("vault.enc")).unwrap(),
            b"legacy encrypted vault"
        );
        let recovery_key = service
            .regenerate_recovery_key("legacy master password")
            .unwrap();
        assert!(recovery_key.starts_with("KN-R1-"));
        assert_eq!(
            service.require_vault_key(|key| *key).unwrap(),
            *expected_key.expose()
        );
        assert!(service.recovery_status().unwrap().configured);
        assert_eq!(
            std::fs::read(temp.path().join("vault.enc")).unwrap(),
            b"legacy encrypted vault"
        );
    }

    #[test]
    fn password_change_preserves_recovery_wrapper_vault_key_and_vault_bytes() {
        use crate::security::crypto::unwrap_recovery_vault_key;

        let temp = tempfile::tempdir().unwrap();
        let params = KdfParams::testing();
        let store = ProfileStore::new(temp.path().to_path_buf());
        let service = AuthService::load(store.clone(), params, Arc::new(CountingEntropy::new()));
        let recovery_key = service
            .create_master_password("old secure master password")
            .unwrap();
        let original_key = service.require_vault_key(|key| *key).unwrap();
        std::fs::write(
            temp.path().join("vault.enc"),
            b"unchanged encrypted records",
        )
        .unwrap();
        let ProfileLoad::Valid(before) = store.load().unwrap() else {
            panic!("profile must exist")
        };

        service
            .change_master_password("old secure master password", "new secure master password")
            .unwrap();
        let ProfileLoad::Valid(after) = store.load().unwrap() else {
            panic!("profile must exist")
        };

        assert_eq!(before.recovery_wrapped_key, after.recovery_wrapped_key);
        assert_eq!(service.require_vault_key(|key| *key).unwrap(), original_key);
        assert_eq!(
            unwrap_recovery_vault_key(&recovery_key, after.recovery_wrapped_key.as_ref().unwrap())
                .unwrap()
                .expose(),
            &original_key
        );
        assert_eq!(
            std::fs::read(temp.path().join("vault.enc")).unwrap(),
            b"unchanged encrypted records"
        );
    }

    #[test]
    fn recovery_rotates_credentials_preserves_vault_key_and_never_touches_vault_records() {
        let temp = tempfile::tempdir().unwrap();
        let params = KdfParams::testing();
        let store = ProfileStore::new(temp.path().to_path_buf());
        let service = AuthService::load(store, params, Arc::new(CountingEntropy::new()));
        let old_recovery_key = service
            .create_master_password("old secure master password")
            .unwrap();
        let original_key = service.require_vault_key(|key| *key).unwrap();
        let vault_bytes = b"ciphertext remains byte-for-byte identical";
        std::fs::write(temp.path().join("vault.enc"), vault_bytes).unwrap();
        service.lock();

        let new_recovery_key = service
            .recover_master_password(&old_recovery_key, "new secure master password")
            .unwrap();
        assert_ne!(new_recovery_key.as_str(), old_recovery_key.as_str());
        assert_eq!(service.require_vault_key(|key| *key).unwrap(), original_key);
        assert_eq!(
            std::fs::read(temp.path().join("vault.enc")).unwrap(),
            vault_bytes
        );
        service.lock();
        assert_eq!(
            service.unlock("old secure master password"),
            Err(AuthError::InvalidCredentials)
        );
        service.unlock("new secure master password").unwrap();
        service.lock();
        assert_eq!(
            service.recover_master_password(&old_recovery_key, "another secure master password"),
            Err(AuthError::InvalidRecoveryKey)
        );
        service
            .recover_master_password(&new_recovery_key, "another secure master password")
            .unwrap();
        assert_eq!(service.require_vault_key(|key| *key).unwrap(), original_key);
    }

    #[test]
    fn wrong_recovery_and_failed_atomic_write_preserve_profile_and_retry_state() {
        let temp = tempfile::tempdir().unwrap();
        let params = KdfParams::testing();
        let store = ProfileStore::new(temp.path().to_path_buf());
        let service = AuthService::load(store.clone(), params, Arc::new(CountingEntropy::new()));
        let recovery_key = service
            .create_master_password("old secure master password")
            .unwrap();
        service.lock();
        let before = std::fs::read(store.profile_path()).unwrap();

        assert_eq!(
            service.recover_master_password(
                "KN-R1-FFFF-FFFF-FFFF-FFFF-FFFF-FFFF-FFFF-FFFF",
                "new secure master password"
            ),
            Err(AuthError::InvalidRecoveryKey)
        );
        assert_eq!(std::fs::read(store.profile_path()).unwrap(), before);

        store.fail_next_replace_for_test();
        assert_eq!(
            service.recover_master_password(&recovery_key, "new secure master password"),
            Err(AuthError::LocalDataFailure)
        );
        assert_eq!(service.status(), AuthStatus::Locked);
        assert_eq!(std::fs::read(store.profile_path()).unwrap(), before);
        service.unlock("old secure master password").unwrap();
        service.lock();
        // A failed commit must not consume or rotate the user's only recovery key.
        let replacement = service
            .recover_master_password(&recovery_key, "new secure master password")
            .unwrap();
        assert!(replacement.as_str() != recovery_key.as_str());
        service.lock();
        service.unlock("new secure master password").unwrap();
    }

    #[test]
    fn recovery_regeneration_requires_current_password_and_invalidates_previous_key() {
        let temp = tempfile::tempdir().unwrap();
        let params = KdfParams::testing();
        let store = ProfileStore::new(temp.path().to_path_buf());
        let service = AuthService::load(store.clone(), params, Arc::new(CountingEntropy::new()));
        let old_key = service
            .create_master_password("a secure master password")
            .unwrap();
        let original_vault_key = service.require_vault_key(|key| *key).unwrap();
        let before = std::fs::read(store.profile_path()).unwrap();
        assert_eq!(
            service.regenerate_recovery_key("wrong master password"),
            Err(AuthError::InvalidCredentials)
        );
        assert_eq!(std::fs::read(store.profile_path()).unwrap(), before);

        let new_key = service
            .regenerate_recovery_key("a secure master password")
            .unwrap();
        assert_ne!(new_key.as_str(), old_key.as_str());
        assert_eq!(
            service.require_vault_key(|key| *key).unwrap(),
            original_vault_key
        );
        service.lock();
        assert_eq!(
            service.recover_master_password(&old_key, "replacement master password"),
            Err(AuthError::InvalidRecoveryKey)
        );
        service
            .recover_master_password(&new_key, "replacement master password")
            .unwrap();
        assert_eq!(
            service.require_vault_key(|key| *key).unwrap(),
            original_vault_key
        );
    }

    #[test]
    fn pin_setup_wraps_the_existing_vault_key_without_persisting_the_pin() {
        let fixture = AuthFixture::new();
        fixture.create_unlocked_with_vault();
        let original_key = fixture.service.require_vault_key(|key| *key).unwrap();

        fixture
            .service
            .setup_pin("a secure master password", "123456", "123456")
            .unwrap();
        assert_eq!(
            fixture.service.pin_status().unwrap(),
            PinStatus {
                configured: true,
                unlock_available: true
            }
        );
        let profile_bytes = std::fs::read(fixture.temp.path().join("profile.json")).unwrap();
        assert!(!profile_bytes.windows(6).any(|window| window == b"123456"));

        fixture.service.lock();
        fixture.service.unlock_with_pin("123456").unwrap();
        assert_eq!(
            fixture.service.require_vault_key(|key| *key).unwrap(),
            original_key
        );
        fixture
            .service
            .change_master_password("a secure master password", "a replacement secure password")
            .unwrap();
        assert!(fixture.service.pin_status().unwrap().configured);
        fixture.service.lock();
        fixture.service.unlock_with_pin("123456").unwrap();
        fixture.service.lock();
        fixture
            .service
            .unlock("a replacement secure password")
            .unwrap();
        assert_eq!(
            fixture.service.require_vault_key(|key| *key).unwrap(),
            original_key
        );
    }

    #[test]
    fn pin_management_requires_the_master_password_and_updates_atomically() {
        let fixture = AuthFixture::new();
        fixture.create_unlocked_with_vault();
        assert_eq!(
            fixture.service.setup_pin("wrong", "123456", "123456"),
            Err(AuthError::InvalidCredentials)
        );
        assert!(!fixture.service.pin_status().unwrap().configured);
        assert_eq!(
            fixture
                .service
                .setup_pin("a secure master password", "123456", "654321"),
            Err(AuthError::PinConfirmationMismatch)
        );
        fixture.service.store.fail_next_replace_for_test();
        assert_eq!(
            fixture
                .service
                .setup_pin("a secure master password", "123456", "123456"),
            Err(AuthError::LocalDataFailure)
        );
        assert!(!fixture.service.pin_status().unwrap().configured);
        fixture
            .service
            .setup_pin("a secure master password", "123456", "123456")
            .unwrap();
        assert_eq!(
            fixture.service.change_pin("wrong", "654321", "654321"),
            Err(AuthError::InvalidCredentials)
        );
        let before_failed_change = std::fs::read(fixture.temp.path().join("profile.json")).unwrap();
        fixture.service.store.fail_next_replace_for_test();
        assert_eq!(
            fixture
                .service
                .change_pin("a secure master password", "654321", "654321"),
            Err(AuthError::LocalDataFailure)
        );
        assert_eq!(
            std::fs::read(fixture.temp.path().join("profile.json")).unwrap(),
            before_failed_change
        );
        fixture.service.lock();
        fixture.service.unlock_with_pin("123456").unwrap();
        fixture
            .service
            .change_pin("a secure master password", "654321", "654321")
            .unwrap();
        fixture.service.lock();
        assert_eq!(
            fixture.service.unlock_with_pin("123456"),
            Err(AuthError::InvalidPin)
        );
        fixture.service.unlock_with_pin("654321").unwrap();

        let before_failed_remove = std::fs::read(fixture.temp.path().join("profile.json")).unwrap();
        assert_eq!(
            fixture.service.remove_pin("wrong"),
            Err(AuthError::InvalidCredentials)
        );
        assert_eq!(
            std::fs::read(fixture.temp.path().join("profile.json")).unwrap(),
            before_failed_remove
        );
        fixture.service.store.fail_next_replace_for_test();
        assert_eq!(
            fixture.service.remove_pin("a secure master password"),
            Err(AuthError::LocalDataFailure)
        );
        assert!(fixture.service.pin_status().unwrap().configured);
        assert_eq!(
            std::fs::read(fixture.temp.path().join("profile.json")).unwrap(),
            before_failed_remove
        );
        fixture
            .service
            .remove_pin("a secure master password")
            .unwrap();
        fixture.service.lock();
        assert_eq!(
            fixture.service.unlock_with_pin("654321"),
            Err(AuthError::PinNotConfigured)
        );
        fixture.service.unlock("a secure master password").unwrap();
    }

    #[test]
    fn five_bad_pin_attempts_require_master_password_for_only_that_locked_session() {
        let fixture = AuthFixture::new();
        fixture.create_unlocked_with_vault();
        fixture
            .service
            .setup_pin("a secure master password", "123456", "123456")
            .unwrap();
        fixture.service.lock();
        let start = Instant::now();
        for _ in 0..3 {
            assert_eq!(
                fixture.service.unlock_with_pin_at("000000", start),
                Err(AuthError::InvalidPin)
            );
        }
        assert_eq!(
            fixture.service.unlock_with_pin_at("000000", start),
            Err(AuthError::PinThrottled {
                retry_after_ms: 2_000
            })
        );
        assert!(matches!(
            fixture.service.unlock_with_pin_at("000000", start),
            Err(AuthError::PinThrottled { .. })
        ));
        assert_eq!(
            fixture
                .service
                .unlock_with_pin_at("000000", start + Duration::from_secs(2)),
            Err(AuthError::PinRequiresMasterPassword)
        );
        assert_eq!(
            fixture
                .service
                .unlock_with_pin_at("123456", start + Duration::from_secs(2)),
            Err(AuthError::PinRequiresMasterPassword)
        );
        fixture.service.unlock("a secure master password").unwrap();
        fixture.service.lock();
        fixture.service.unlock_with_pin("123456").unwrap();
    }

    #[test]
    fn copied_profile_cannot_use_pin_under_a_different_device_protector() {
        let fixture = AuthFixture::new();
        fixture.create_unlocked_with_vault();
        fixture
            .service
            .setup_pin("a secure master password", "123456", "123456")
            .unwrap();
        let copied = tempfile::tempdir().unwrap();
        std::fs::write(
            copied.path().join("profile.json"),
            std::fs::read(fixture.temp.path().join("profile.json")).unwrap(),
        )
        .unwrap();
        let copied_service = AuthService::load_with_device_protector(
            ProfileStore::new(copied.path().to_path_buf()),
            KdfParams::testing(),
            Arc::new(FixedEntropy),
            Arc::new(FakeDeviceProtector(0xa5)),
        );
        assert_eq!(
            copied_service.unlock_with_pin("123456"),
            Err(AuthError::InvalidPin)
        );
        copied_service.unlock("a secure master password").unwrap();
    }

    #[test]
    fn tampered_pin_wrapper_fails_without_blocking_master_password_unlock() {
        let fixture = AuthFixture::new();
        fixture.create_unlocked_with_vault();
        fixture
            .service
            .setup_pin("a secure master password", "123456", "123456")
            .unwrap();
        let path = fixture.temp.path().join("profile.json");
        let mut json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let encoded = json["pin_wrapped_key"]["ciphertext"].as_str().unwrap();
        let mut ciphertext = STANDARD.decode(encoded).unwrap();
        ciphertext[0] ^= 1;
        json["pin_wrapped_key"]["ciphertext"] = STANDARD.encode(ciphertext).into();
        std::fs::write(&path, serde_json::to_vec_pretty(&json).unwrap()).unwrap();

        let service = AuthService::load_with_device_protector(
            ProfileStore::new(fixture.temp.path().to_path_buf()),
            KdfParams::testing(),
            Arc::new(FixedEntropy),
            Arc::new(FakeDeviceProtector(0x5a)),
        );
        assert_eq!(
            service.unlock_with_pin("123456"),
            Err(AuthError::InvalidPin)
        );
        service.unlock("a secure master password").unwrap();
    }

    #[test]
    fn recovery_clears_device_pin_enrollment_but_preserves_the_vault_key() {
        let fixture = AuthFixture::new();
        let recovery_key = fixture
            .service
            .create_master_password("a secure master password")
            .unwrap();
        let original_key = fixture.service.require_vault_key(|key| *key).unwrap();
        fixture
            .service
            .setup_pin("a secure master password", "123456", "123456")
            .unwrap();
        fixture.service.lock();
        fixture
            .service
            .recover_master_password(&recovery_key, "a replacement secure password")
            .unwrap();
        assert!(!fixture.service.pin_status().unwrap().configured);
        assert_eq!(
            fixture.service.require_vault_key(|key| *key).unwrap(),
            original_key
        );
    }
}
