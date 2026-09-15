use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use atomic_write_file::AtomicWriteFile;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;

use super::crypto::{PinWrappedVaultKey, WrappedVaultKey};

const PROFILE_FILENAME: &str = "profile.json";
const VAULT_FILENAME: &str = "vault.enc";
const LEGACY_FORMAT_VERSION: u32 = 1;
const RECOVERY_FORMAT_VERSION: u32 = 2;
const KDF_ALGORITHM: &str = "argon2id";
const KEY_WRAP_ALGORITHM: &str = "xchacha20poly1305";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoredProfile {
    pub format_version: u32,
    pub kdf_algorithm: String,
    pub key_wrap_algorithm: String,
    pub wrapped_key: WrappedVaultKey,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_wrapped_key: Option<WrappedVaultKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_wrapped_key: Option<PinWrappedVaultKey>,
}

impl StoredProfile {
    #[cfg(test)]
    pub(crate) fn new(wrapped_key: WrappedVaultKey) -> Self {
        Self {
            format_version: LEGACY_FORMAT_VERSION,
            kdf_algorithm: KDF_ALGORITHM.to_owned(),
            key_wrap_algorithm: KEY_WRAP_ALGORITHM.to_owned(),
            wrapped_key,
            recovery_wrapped_key: None,
            pin_wrapped_key: None,
        }
    }

    pub(crate) fn with_recovery(
        wrapped_key: WrappedVaultKey,
        recovery_wrapped_key: WrappedVaultKey,
    ) -> Self {
        Self {
            format_version: RECOVERY_FORMAT_VERSION,
            kdf_algorithm: KDF_ALGORITHM.to_owned(),
            key_wrap_algorithm: KEY_WRAP_ALGORITHM.to_owned(),
            wrapped_key,
            recovery_wrapped_key: Some(recovery_wrapped_key),
            pin_wrapped_key: None,
        }
    }

    pub(crate) fn replacing_master_wrapper(&self, wrapped_key: WrappedVaultKey) -> Self {
        Self {
            wrapped_key,
            ..self.clone()
        }
    }

    pub(crate) fn replacing_recovery_wrapper(&self, recovery_wrapped_key: WrappedVaultKey) -> Self {
        Self {
            format_version: RECOVERY_FORMAT_VERSION,
            recovery_wrapped_key: Some(recovery_wrapped_key),
            ..self.clone()
        }
    }

    pub(crate) fn replacing_pin_wrapper(&self, pin_wrapped_key: PinWrappedVaultKey) -> Self {
        Self {
            pin_wrapped_key: Some(pin_wrapped_key),
            ..self.clone()
        }
    }

    pub(crate) fn removing_pin_wrapper(&self) -> Self {
        Self {
            pin_wrapped_key: None,
            ..self.clone()
        }
    }

    fn validate(&self) -> Result<(), StorageError> {
        let supported_shape = match self.format_version {
            LEGACY_FORMAT_VERSION => self.recovery_wrapped_key.is_none(),
            RECOVERY_FORMAT_VERSION => true,
            _ => false,
        };
        if !supported_shape
            || self.kdf_algorithm != KDF_ALGORITHM
            || self.key_wrap_algorithm != KEY_WRAP_ALGORITHM
            || !valid_wrapped_key(&self.wrapped_key)
            || self
                .recovery_wrapped_key
                .as_ref()
                .is_some_and(|wrapped| !valid_wrapped_key(wrapped))
            || self
                .pin_wrapped_key
                .as_ref()
                .is_some_and(|wrapped| !valid_pin_wrapped_key(wrapped))
        {
            return Err(StorageError::DamagedProfile);
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ProfileStore {
    app_data_dir: PathBuf,
    #[cfg(test)]
    fail_next_replace: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl ProfileStore {
    pub(crate) fn new(app_data_dir: PathBuf) -> Self {
        Self {
            app_data_dir,
            #[cfg(test)]
            fail_next_replace: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub(crate) fn load(&self) -> Result<ProfileLoad, StorageError> {
        let path = self.profile_path();
        if !path.try_exists().map_err(StorageError::Io)? {
            return Ok(ProfileLoad::Missing);
        }

        let bytes = fs::read(path).map_err(StorageError::Io)?;
        let profile: StoredProfile =
            serde_json::from_slice(&bytes).map_err(|_| StorageError::DamagedProfile)?;
        profile.validate()?;
        Ok(ProfileLoad::Valid(Box::new(profile)))
    }

    pub(crate) fn create(&self, profile: &StoredProfile) -> Result<(), StorageError> {
        profile.validate()?;
        fs::create_dir_all(&self.app_data_dir).map_err(StorageError::Io)?;

        let bytes = serde_json::to_vec_pretty(profile).map_err(StorageError::Serialization)?;
        let mut temporary = NamedTempFile::new_in(&self.app_data_dir).map_err(StorageError::Io)?;
        temporary.write_all(&bytes).map_err(StorageError::Io)?;
        temporary.flush().map_err(StorageError::Io)?;
        temporary.as_file().sync_all().map_err(StorageError::Io)?;
        temporary
            .persist_noclobber(self.profile_path())
            .map_err(|error| {
                if error.error.kind() == io::ErrorKind::AlreadyExists {
                    StorageError::AlreadyExists
                } else {
                    StorageError::Io(error.error)
                }
            })?;
        Ok(())
    }

    pub(crate) fn replace(&self, profile: &StoredProfile) -> Result<(), StorageError> {
        profile.validate()?;
        fs::create_dir_all(&self.app_data_dir).map_err(StorageError::Io)?;

        let bytes = serde_json::to_vec_pretty(profile).map_err(StorageError::Serialization)?;
        let mut file = AtomicWriteFile::open(self.profile_path()).map_err(StorageError::Io)?;
        file.write_all(&bytes).map_err(StorageError::Io)?;
        file.sync_all().map_err(StorageError::Io)?;

        #[cfg(test)]
        if self
            .fail_next_replace
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err(StorageError::Io(io::Error::other(
                "injected profile replacement failure",
            )));
        }

        file.commit().map_err(StorageError::Io)
    }

    #[cfg(test)]
    pub(crate) fn fail_next_replace_for_test(&self) {
        self.fail_next_replace
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub(crate) fn reset(&self) -> Result<(), StorageError> {
        remove_if_present(&self.app_data_dir.join("notes.enc-journal"))?;
        remove_if_present(&self.app_data_dir.join("notes.enc-wal"))?;
        remove_if_present(&self.app_data_dir.join("notes.enc-shm"))?;
        remove_if_present(&self.app_data_dir.join("notes.enc"))?;
        remove_if_present(&self.app_data_dir.join("vault.enc-journal"))?;
        remove_if_present(&self.app_data_dir.join("vault.enc-wal"))?;
        remove_if_present(&self.app_data_dir.join("vault.enc-shm"))?;
        remove_if_present(&self.vault_path())?;
        remove_if_present(&self.profile_path())?;
        Ok(())
    }

    pub(crate) fn profile_path(&self) -> PathBuf {
        self.app_data_dir.join(PROFILE_FILENAME)
    }

    fn vault_path(&self) -> PathBuf {
        self.app_data_dir.join(VAULT_FILENAME)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProfileLoad {
    Missing,
    Valid(Box<StoredProfile>),
}

#[derive(Debug, Error)]
pub(crate) enum StorageError {
    #[error("the encrypted profile is damaged or unsupported")]
    DamagedProfile,
    #[error("an encrypted profile already exists")]
    AlreadyExists,
    #[error("the encrypted profile could not be serialized")]
    Serialization(serde_json::Error),
    #[error("local KeyNest data could not be accessed")]
    Io(io::Error),
}

fn decoded_length(value: &str) -> Option<usize> {
    STANDARD.decode(value).ok().map(|bytes| bytes.len())
}

fn valid_wrapped_key(wrapped: &WrappedVaultKey) -> bool {
    wrapped.params.validate().is_ok()
        && decoded_length(&wrapped.salt) == Some(16)
        && decoded_length(&wrapped.nonce) == Some(24)
        && decoded_length(&wrapped.ciphertext) == Some(48)
}

fn valid_pin_wrapped_key(wrapped: &PinWrappedVaultKey) -> bool {
    wrapped.version == 1
        && wrapped.params.validate().is_ok()
        && decoded_length(&wrapped.protected_device_secret)
            .is_some_and(|length| (1..=4096).contains(&length))
        && decoded_length(&wrapped.salt) == Some(16)
        && decoded_length(&wrapped.nonce) == Some(24)
        && decoded_length(&wrapped.ciphertext) == Some(48)
}

fn remove_if_present(path: &Path) -> Result<(), StorageError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(StorageError::Io(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::crypto::{wrap_new_vault_key, CryptoError, EntropySource, KdfParams};

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

    fn profile_fixture(password: &str) -> (StoredProfile, Vec<u8>) {
        let (wrapped, vault_key) =
            wrap_new_vault_key(password, KdfParams::testing(), &FixedEntropy).unwrap();
        (
            StoredProfile::new(wrapped),
            vault_key.expose_for_test().to_vec(),
        )
    }

    #[test]
    fn malformed_or_excessive_kdf_metadata_in_either_wrapper_fails_closed() {
        use serde_json::json;
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_path_buf());
        let (legacy, _) = profile_fixture("fixture password");
        let profile = StoredProfile::with_recovery(legacy.wrapped_key.clone(), legacy.wrapped_key);
        let original = serde_json::to_value(profile).unwrap();
        for wrapper in ["wrapped_key", "recovery_wrapped_key"] {
            for invalid in [
                json!(null),
                json!({"memory_kib": 65_536, "iterations": 3}),
                json!({"memory_kib": "65536", "iterations": 3, "parallelism": 4}),
                json!({"memory_kib": -1, "iterations": 3, "parallelism": 4}),
                json!({"memory_kib": 65536.5, "iterations": 3, "parallelism": 4}),
                json!({"memory_kib": 65_536, "iterations": 3, "parallelism": 4, "unknown": 1}),
                json!({"memory_kib": 0, "iterations": 3, "parallelism": 4}),
                json!({"memory_kib": 65_536, "iterations": 0, "parallelism": 4}),
                json!({"memory_kib": 65_536, "iterations": 3, "parallelism": 0}),
                json!({"memory_kib": u32::MAX, "iterations": 3, "parallelism": 4}),
                json!({"memory_kib": 65_536, "iterations": u32::MAX, "parallelism": 4}),
                json!({"memory_kib": 65_536, "iterations": 3, "parallelism": u32::MAX}),
                json!({"memory_kib": 131_072, "iterations": 4, "parallelism": 4}),
                json!({"memory_kib": 4294967296_u64, "iterations": 3, "parallelism": 4}),
            ] {
                let mut damaged = original.clone();
                damaged[wrapper]["params"] = invalid;
                let bytes = serde_json::to_vec(&damaged).unwrap();
                std::fs::write(store.profile_path(), &bytes).unwrap();
                assert!(matches!(store.load(), Err(StorageError::DamagedProfile)));
                assert_eq!(std::fs::read(store.profile_path()).unwrap(), bytes);
            }
        }
    }

    #[test]
    fn invalid_kdf_cannot_be_created_or_replace_an_existing_profile() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_path_buf());
        let (profile, _) = profile_fixture("fixture password");
        let mut invalid = profile.clone();
        invalid.wrapped_key.params.memory_kib = u32::MAX;
        assert!(matches!(
            store.create(&invalid),
            Err(StorageError::DamagedProfile)
        ));
        assert!(!store.profile_path().exists());
        store.create(&profile).unwrap();
        let original = std::fs::read(store.profile_path()).unwrap();
        assert!(matches!(
            store.replace(&invalid),
            Err(StorageError::DamagedProfile)
        ));
        assert_eq!(std::fs::read(store.profile_path()).unwrap(), original);
    }

    #[test]
    fn missing_profile_is_distinct_from_damaged_profile() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_path_buf());
        assert_eq!(store.load().unwrap(), ProfileLoad::Missing);

        std::fs::write(temp.path().join("profile.json"), b"not-json").unwrap();

        assert!(matches!(store.load(), Err(StorageError::DamagedProfile)));
    }

    #[test]
    fn profile_creation_never_writes_plaintext_secrets() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_path_buf());
        let password = "a secure master password";
        let (profile, plaintext_vault_key) = profile_fixture(password);

        store.create(&profile).unwrap();

        let bytes = std::fs::read(store.profile_path()).unwrap();
        assert!(!bytes
            .windows(password.len())
            .any(|window| window == password.as_bytes()));
        assert!(!bytes
            .windows(plaintext_vault_key.len())
            .any(|window| window == plaintext_vault_key));
        assert_eq!(store.load().unwrap(), ProfileLoad::Valid(Box::new(profile)));
    }

    #[test]
    fn weaker_kdf_metadata_is_rejected_as_damaged() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_path_buf());
        let (mut profile, _) = profile_fixture("a secure master password");
        profile.wrapped_key.params = KdfParams {
            memory_kib: 65_535,
            ..KdfParams::production()
        };
        std::fs::write(
            temp.path().join("profile.json"),
            serde_json::to_vec(&profile).unwrap(),
        )
        .unwrap();

        assert!(matches!(store.load(), Err(StorageError::DamagedProfile)));
    }

    #[test]
    fn reset_deletes_only_keynest_owned_security_files() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_path_buf());
        std::fs::write(temp.path().join("profile.json"), b"profile").unwrap();
        std::fs::write(temp.path().join("vault.enc"), b"vault").unwrap();
        std::fs::write(temp.path().join("vault.enc-journal"), b"journal").unwrap();
        std::fs::write(temp.path().join("vault.enc-wal"), b"wal").unwrap();
        std::fs::write(temp.path().join("vault.enc-shm"), b"shm").unwrap();
        std::fs::write(temp.path().join("notes.enc"), b"notes").unwrap();
        std::fs::write(temp.path().join("notes.enc-journal"), b"journal").unwrap();
        std::fs::write(temp.path().join("notes.enc-wal"), b"wal").unwrap();
        std::fs::write(temp.path().join("notes.enc-shm"), b"shm").unwrap();
        std::fs::write(temp.path().join("keep.txt"), b"keep").unwrap();

        store.reset().unwrap();

        assert!(!temp.path().join("profile.json").exists());
        assert!(!temp.path().join("vault.enc").exists());
        assert!(!temp.path().join("vault.enc-journal").exists());
        assert!(!temp.path().join("vault.enc-wal").exists());
        assert!(!temp.path().join("vault.enc-shm").exists());
        assert!(!temp.path().join("notes.enc").exists());
        assert!(!temp.path().join("notes.enc-journal").exists());
        assert!(!temp.path().join("notes.enc-wal").exists());
        assert!(!temp.path().join("notes.enc-shm").exists());
        assert!(temp.path().join("keep.txt").exists());
    }

    #[test]
    fn failed_vault_deletion_preserves_profile_for_a_retry() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_path_buf());
        std::fs::write(temp.path().join("profile.json"), b"profile").unwrap();
        std::fs::create_dir(temp.path().join("vault.enc")).unwrap();

        assert!(matches!(store.reset(), Err(StorageError::Io(_))));
        assert!(temp.path().join("profile.json").exists());
    }

    #[test]
    fn password_change_atomic_replace_failure_preserves_original_profile_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_path_buf());
        let (original, _) = profile_fixture("old secure master password");
        let (replacement, _) = profile_fixture("new secure master password");
        store.create(&original).unwrap();
        let original_bytes = std::fs::read(store.profile_path()).unwrap();
        store.fail_next_replace_for_test();

        assert!(matches!(
            store.replace(&replacement),
            Err(StorageError::Io(_))
        ));
        assert_eq!(std::fs::read(store.profile_path()).unwrap(), original_bytes);
        assert_eq!(
            store.load().unwrap(),
            ProfileLoad::Valid(Box::new(original))
        );
    }

    #[test]
    fn legacy_v1_profile_without_recovery_metadata_still_loads() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_path_buf());
        let (profile, _) = profile_fixture("a secure master password");
        let mut json = serde_json::to_value(&profile).unwrap();
        assert!(json.get("recovery_wrapped_key").is_none());
        json.as_object_mut().unwrap().remove("recovery_wrapped_key");
        std::fs::write(
            store.profile_path(),
            serde_json::to_vec_pretty(&json).unwrap(),
        )
        .unwrap();

        assert_eq!(store.load().unwrap(), ProfileLoad::Valid(Box::new(profile)));
    }

    #[test]
    fn recovery_metadata_must_match_the_version_and_validate_completely() {
        use crate::security::crypto::{generate_recovery_key, wrap_recovery_vault_key};

        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_path_buf());
        let (legacy, vault_key) = profile_fixture("a secure master password");
        let recovery_key = generate_recovery_key(&FixedEntropy).unwrap();
        let recovery_wrapped = wrap_recovery_vault_key(
            &recovery_key,
            vault_key.as_slice().try_into().unwrap(),
            KdfParams::testing(),
            &FixedEntropy,
        )
        .unwrap();

        let mut invalid_v1 = legacy.clone();
        invalid_v1.recovery_wrapped_key = Some(recovery_wrapped.clone());
        std::fs::write(
            store.profile_path(),
            serde_json::to_vec(&invalid_v1).unwrap(),
        )
        .unwrap();
        assert!(matches!(store.load(), Err(StorageError::DamagedProfile)));

        let mut damaged_v2 = StoredProfile::with_recovery(legacy.wrapped_key, recovery_wrapped);
        damaged_v2.recovery_wrapped_key.as_mut().unwrap().nonce = "not-base64".to_owned();
        std::fs::write(
            store.profile_path(),
            serde_json::to_vec(&damaged_v2).unwrap(),
        )
        .unwrap();
        assert!(matches!(store.load(), Err(StorageError::DamagedProfile)));
    }
}
