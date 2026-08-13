use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use serde::Serialize;
use thiserror::Error;

use super::{
    crypto::{
        unwrap_vault_key, wrap_existing_vault_key, wrap_new_vault_key, CryptoError, EntropySource,
        KdfParams, VaultKey,
    },
    storage::{ProfileLoad, ProfileStore, StorageError, StoredProfile},
};

const MINIMUM_PASSWORD_CHARACTERS: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AuthStatus {
    SetupRequired,
    Locked,
    Unlocked,
    DataError,
}

#[derive(Clone)]
pub(crate) struct AuthService {
    inner: Arc<Mutex<AuthInner>>,
    store: ProfileStore,
    kdf_params: KdfParams,
    entropy: Arc<dyn EntropySource>,
}

struct AuthInner {
    state: AuthState,
    failed_attempts: u32,
    next_allowed_at: Option<Instant>,
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
        let state = match store.load() {
            Ok(ProfileLoad::Missing) => AuthState::SetupRequired,
            Ok(ProfileLoad::Valid(profile)) => AuthState::Locked(profile),
            Err(_) => AuthState::DataError,
        };
        Self {
            inner: Arc::new(Mutex::new(AuthInner {
                state,
                failed_attempts: 0,
                next_allowed_at: None,
            })),
            store,
            kdf_params,
            entropy,
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

    pub(crate) fn create_master_password(&self, password: &str) -> Result<(), AuthError> {
        if password.chars().count() < MINIMUM_PASSWORD_CHARACTERS {
            return Err(AuthError::PasswordTooShort);
        }

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
        let profile = StoredProfile::new(wrapped_key);
        if self.store.create(&profile).is_err() {
            inner.state = AuthState::DataError;
            return Err(AuthError::LocalDataFailure);
        }

        inner.failed_attempts = 0;
        inner.next_allowed_at = None;
        inner.state = AuthState::Unlocked { profile, vault_key };
        Ok(())
    }

    pub(crate) fn unlock(&self, password: &str) -> Result<(), AuthError> {
        self.unlock_at(password, Instant::now())
    }

    pub(crate) fn unlock_at(&self, password: &str, now: Instant) -> Result<(), AuthError> {
        let mut inner = self.lock_inner();
        if let Some(next_allowed_at) = inner.next_allowed_at {
            if next_allowed_at > now {
                let remaining = next_allowed_at.duration_since(now);
                return Err(AuthError::Throttled {
                    retry_after_ms: remaining.as_millis().max(1) as u64,
                });
            }
        }

        let profile = match &inner.state {
            AuthState::Locked(profile) => profile.clone(),
            AuthState::Unlocked { .. } => return Ok(()),
            AuthState::SetupRequired => return Err(AuthError::NotInitialized),
            AuthState::DataError => return Err(AuthError::DataDamaged),
        };

        match unwrap_vault_key(password, &profile.wrapped_key) {
            Ok(vault_key) => {
                inner.failed_attempts = 0;
                inner.next_allowed_at = None;
                inner.state = AuthState::Unlocked { profile, vault_key };
                Ok(())
            }
            Err(CryptoError::AuthenticationFailed) => {
                record_failed_attempt(&mut inner, now);
                Err(AuthError::InvalidCredentials)
            }
            Err(_) => {
                inner.state = AuthState::DataError;
                Err(AuthError::DataDamaged)
            }
        }
    }

    pub(crate) fn lock(&self) -> AuthStatus {
        let mut inner = self.lock_inner();
        let previous = std::mem::replace(&mut inner.state, AuthState::DataError);
        inner.state = match previous {
            AuthState::Unlocked { profile, .. } => AuthState::Locked(profile),
            other => other,
        };
        match inner.state {
            AuthState::SetupRequired => AuthStatus::SetupRequired,
            AuthState::Locked(_) => AuthStatus::Locked,
            AuthState::Unlocked { .. } => AuthStatus::Unlocked,
            AuthState::DataError => AuthStatus::DataError,
        }
    }

    pub(crate) fn change_master_password(
        &self,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), AuthError> {
        if new_password.chars().count() < MINIMUM_PASSWORD_CHARACTERS {
            return Err(AuthError::PasswordTooShort);
        }

        let mut inner = self.lock_inner();
        let (profile, vault_key) = match &mut inner.state {
            AuthState::Unlocked { profile, vault_key } => (profile, vault_key),
            AuthState::Locked(_) => return Err(AuthError::Unauthorized),
            AuthState::SetupRequired => return Err(AuthError::NotInitialized),
            AuthState::DataError => return Err(AuthError::DataDamaged),
        };

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

        let wrapped_key = wrap_existing_vault_key(
            new_password,
            vault_key.expose(),
            self.kdf_params,
            self.entropy.as_ref(),
        )
        .map_err(|_| AuthError::LocalDataFailure)?;
        let replacement = StoredProfile::new(wrapped_key);
        self.store.replace(&replacement)?;
        *profile = replacement;
        Ok(())
    }

    pub(crate) fn reset_keynest(&self, confirmation: &str) -> Result<(), AuthError> {
        if confirmation != "RESET" {
            return Err(AuthError::InvalidResetConfirmation);
        }

        let mut inner = self.lock_inner();
        if self.store.reset().is_err() {
            inner.state = AuthState::DataError;
            return Err(AuthError::LocalDataFailure);
        }
        inner.failed_attempts = 0;
        inner.next_allowed_at = None;
        inner.state = AuthState::SetupRequired;
        Ok(())
    }

    // This is the authorization boundary that future vault commands must use.
    #[allow(dead_code)]
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
    #[error("KeyNest already has a master password")]
    AlreadyInitialized,
    #[error("KeyNest needs a master password before it can be unlocked")]
    NotInitialized,
    #[error("the master password is incorrect")]
    InvalidCredentials,
    #[error("wait before trying again")]
    Throttled { retry_after_ms: u64 },
    #[error("type RESET exactly to confirm")]
    InvalidResetConfirmation,
    #[error("KeyNest is locked")]
    Unauthorized,
    #[error("the local encrypted profile is damaged")]
    DataDamaged,
    #[error("local KeyNest data could not be accessed")]
    LocalDataFailure,
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

fn record_failed_attempt(inner: &mut AuthInner, now: Instant) {
    inner.failed_attempts = inner.failed_attempts.saturating_add(1);
    if inner.failed_attempts >= 3 {
        let exponent = (inner.failed_attempts - 3).min(3);
        let seconds = (1_u64 << exponent).min(5);
        inner.next_allowed_at = Some(now + Duration::from_secs(seconds));
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    use super::*;
    use crate::security::{
        crypto::{CryptoError, EntropySource, KdfParams},
        storage::ProfileStore,
    };

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

    struct AuthFixture {
        _temp: tempfile::TempDir,
        service: AuthService,
    }

    impl AuthFixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let params = KdfParams::testing();
            let store = ProfileStore::new(temp.path().to_path_buf(), params);
            let service = AuthService::load(store, params, Arc::new(FixedEntropy));
            Self {
                _temp: temp,
                service,
            }
        }

        fn with_profile_bytes(bytes: &[u8]) -> Self {
            let temp = tempfile::tempdir().unwrap();
            std::fs::write(temp.path().join("profile.json"), bytes).unwrap();
            let params = KdfParams::testing();
            let store = ProfileStore::new(temp.path().to_path_buf(), params);
            let service = AuthService::load(store, params, Arc::new(FixedEntropy));
            Self {
                _temp: temp,
                service,
            }
        }
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
    fn reset_requires_exact_confirmation() {
        let fixture = AuthFixture::new();
        fixture
            .service
            .create_master_password("a secure master password")
            .unwrap();

        assert_eq!(
            fixture.service.reset_keynest("reset"),
            Err(AuthError::InvalidResetConfirmation),
        );
        assert_eq!(fixture.service.status(), AuthStatus::Unlocked);

        fixture.service.reset_keynest("RESET").unwrap();
        assert_eq!(fixture.service.status(), AuthStatus::SetupRequired);
    }

    #[test]
    fn third_and_later_failures_enforce_a_capped_delay() {
        let fixture = AuthFixture::new();
        fixture
            .service
            .create_master_password("a secure master password")
            .unwrap();
        fixture.service.lock();
        let start = Instant::now();

        assert_eq!(
            fixture.service.unlock_at("wrong master password", start),
            Err(AuthError::InvalidCredentials),
        );
        assert_eq!(
            fixture.service.unlock_at("wrong master password", start),
            Err(AuthError::InvalidCredentials),
        );
        assert_eq!(
            fixture.service.unlock_at("wrong master password", start),
            Err(AuthError::InvalidCredentials),
        );
        assert!(matches!(
            fixture.service.unlock_at("wrong master password", start),
            Err(AuthError::Throttled {
                retry_after_ms: 1_000
            })
        ));

        let after_one_second = start + Duration::from_secs(1);
        assert_eq!(
            fixture
                .service
                .unlock_at("wrong master password", after_one_second),
            Err(AuthError::InvalidCredentials),
        );
        assert!(matches!(
            fixture
                .service
                .unlock_at("wrong master password", after_one_second),
            Err(AuthError::Throttled {
                retry_after_ms: 2_000
            })
        ));

        let after_three_seconds = start + Duration::from_secs(3);
        assert_eq!(
            fixture
                .service
                .unlock_at("wrong master password", after_three_seconds),
            Err(AuthError::InvalidCredentials),
        );
        let after_seven_seconds = start + Duration::from_secs(7);
        assert_eq!(
            fixture
                .service
                .unlock_at("wrong master password", after_seven_seconds),
            Err(AuthError::InvalidCredentials),
        );
        assert!(matches!(
            fixture
                .service
                .unlock_at("wrong master password", after_seven_seconds),
            Err(AuthError::Throttled {
                retry_after_ms: 5_000
            })
        ));
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
}
