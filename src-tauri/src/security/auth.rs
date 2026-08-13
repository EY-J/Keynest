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
const RESET_CONFIRMATION: &str = "RESET KEYNEST";

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
        inner.next_allowed_at = None;
        inner.state = AuthState::SetupRequired;
        Ok(())
    }

    pub(crate) fn reset_keynest(&self, confirmation: &str) -> Result<(), AuthError> {
        self.validate_reset_confirmation(confirmation)?;
        self.finish_reset()
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
    #[error("type RESET KEYNEST exactly to confirm")]
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
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
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
            let store = ProfileStore::new(temp.path().to_path_buf(), params);
            let service = AuthService::load(store, params, Arc::new(FixedEntropy));
            Self { temp, service }
        }

        fn with_profile_bytes(bytes: &[u8]) -> Self {
            let temp = tempfile::tempdir().unwrap();
            std::fs::write(temp.path().join("profile.json"), bytes).unwrap();
            let params = KdfParams::testing();
            let store = ProfileStore::new(temp.path().to_path_buf(), params);
            let service = AuthService::load(store, params, Arc::new(FixedEntropy));
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

    #[test]
    fn password_change_entropy_failure_preserves_disk_password_key_and_unlocked_session() {
        let temp = tempfile::tempdir().unwrap();
        let params = KdfParams::testing();
        let store = ProfileStore::new(temp.path().to_path_buf(), params);
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
}
