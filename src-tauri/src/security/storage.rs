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

use super::crypto::{KdfParams, WrappedVaultKey};

const PROFILE_FILENAME: &str = "profile.json";
const VAULT_FILENAME: &str = "vault.enc";
const FORMAT_VERSION: u32 = 1;
const KDF_ALGORITHM: &str = "argon2id";
const KEY_WRAP_ALGORITHM: &str = "xchacha20poly1305";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoredProfile {
    pub format_version: u32,
    pub kdf_algorithm: String,
    pub key_wrap_algorithm: String,
    pub wrapped_key: WrappedVaultKey,
}

impl StoredProfile {
    pub(crate) fn new(wrapped_key: WrappedVaultKey) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            kdf_algorithm: KDF_ALGORITHM.to_owned(),
            key_wrap_algorithm: KEY_WRAP_ALGORITHM.to_owned(),
            wrapped_key,
        }
    }

    fn validate(&self, accepted_kdf: KdfParams) -> Result<(), StorageError> {
        if self.format_version != FORMAT_VERSION
            || self.kdf_algorithm != KDF_ALGORITHM
            || self.key_wrap_algorithm != KEY_WRAP_ALGORITHM
            || self.wrapped_key.params != accepted_kdf
            || decoded_length(&self.wrapped_key.salt) != Some(16)
            || decoded_length(&self.wrapped_key.nonce) != Some(24)
            || decoded_length(&self.wrapped_key.ciphertext) != Some(48)
        {
            return Err(StorageError::DamagedProfile);
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ProfileStore {
    app_data_dir: PathBuf,
    accepted_kdf: KdfParams,
    #[cfg(test)]
    fail_next_replace: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl ProfileStore {
    pub(crate) fn new(app_data_dir: PathBuf, accepted_kdf: KdfParams) -> Self {
        Self {
            app_data_dir,
            accepted_kdf,
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
        profile.validate(self.accepted_kdf)?;
        Ok(ProfileLoad::Valid(profile))
    }

    pub(crate) fn create(&self, profile: &StoredProfile) -> Result<(), StorageError> {
        profile.validate(self.accepted_kdf)?;
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
        profile.validate(self.accepted_kdf)?;
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
    Valid(StoredProfile),
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
    fn missing_profile_is_distinct_from_damaged_profile() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_path_buf(), KdfParams::testing());
        assert_eq!(store.load().unwrap(), ProfileLoad::Missing);

        std::fs::write(temp.path().join("profile.json"), b"not-json").unwrap();

        assert!(matches!(store.load(), Err(StorageError::DamagedProfile)));
    }

    #[test]
    fn profile_creation_never_writes_plaintext_secrets() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_path_buf(), KdfParams::testing());
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
        assert_eq!(store.load().unwrap(), ProfileLoad::Valid(profile));
    }

    #[test]
    fn weaker_kdf_metadata_is_rejected_as_damaged() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_path_buf(), KdfParams::production());
        let (mut profile, _) = profile_fixture("a secure master password");
        profile.wrapped_key.params = KdfParams::testing();
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
        let store = ProfileStore::new(temp.path().to_path_buf(), KdfParams::testing());
        std::fs::write(temp.path().join("profile.json"), b"profile").unwrap();
        std::fs::write(temp.path().join("vault.enc"), b"vault").unwrap();
        std::fs::write(temp.path().join("keep.txt"), b"keep").unwrap();

        store.reset().unwrap();

        assert!(!temp.path().join("profile.json").exists());
        assert!(!temp.path().join("vault.enc").exists());
        assert!(temp.path().join("keep.txt").exists());
    }

    #[test]
    fn failed_vault_deletion_preserves_profile_for_a_retry() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_path_buf(), KdfParams::testing());
        std::fs::write(temp.path().join("profile.json"), b"profile").unwrap();
        std::fs::create_dir(temp.path().join("vault.enc")).unwrap();

        assert!(matches!(store.reset(), Err(StorageError::Io(_))));
        assert!(temp.path().join("profile.json").exists());
    }

    #[test]
    fn password_change_atomic_replace_failure_preserves_original_profile_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_path_buf(), KdfParams::testing());
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
        assert_eq!(store.load().unwrap(), ProfileLoad::Valid(original));
    }
}
