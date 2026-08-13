use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::model::{SettingsValues, ThemePreference};

const SETTINGS_FILENAME: &str = "settings.json";
const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredSettings {
    format_version: u32,
    auto_lock_seconds: u64,
    clipboard_clear_seconds: u64,
    theme: ThemePreference,
}

impl StoredSettings {
    fn new(values: SettingsValues) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            auto_lock_seconds: values.auto_lock_seconds,
            clipboard_clear_seconds: values.clipboard_clear_seconds,
            theme: values.theme,
        }
    }

    fn values(self) -> Option<SettingsValues> {
        let values = SettingsValues {
            auto_lock_seconds: self.auto_lock_seconds,
            clipboard_clear_seconds: self.clipboard_clear_seconds,
            theme: self.theme,
        };

        (self.format_version == FORMAT_VERSION && values.is_valid()).then_some(values)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SettingsStore {
    app_data_dir: PathBuf,
}

impl SettingsStore {
    pub(crate) fn new(app_data_dir: PathBuf) -> Self {
        Self { app_data_dir }
    }

    pub(crate) fn load(&self) -> Result<SettingsLoad, SettingsStorageError> {
        let path = self.settings_path();
        if !path.try_exists().map_err(SettingsStorageError::Io)? {
            return Ok(SettingsLoad::Missing);
        }

        let bytes = fs::read(path).map_err(SettingsStorageError::Io)?;
        let stored: StoredSettings = match serde_json::from_slice(&bytes) {
            Ok(stored) => stored,
            Err(_) => return Ok(SettingsLoad::Damaged),
        };

        Ok(match stored.values() {
            Some(values) => SettingsLoad::Valid(values),
            None => SettingsLoad::Damaged,
        })
    }

    pub(crate) fn replace(&self, values: SettingsValues) -> Result<(), SettingsStorageError> {
        if !values.is_valid() {
            return Err(SettingsStorageError::InvalidValues);
        }

        fs::create_dir_all(&self.app_data_dir).map_err(SettingsStorageError::Io)?;
        let bytes = serde_json::to_vec_pretty(&StoredSettings::new(values))
            .map_err(SettingsStorageError::Serialization)?;
        let mut file =
            AtomicWriteFile::open(self.settings_path()).map_err(SettingsStorageError::Io)?;
        file.write_all(&bytes).map_err(SettingsStorageError::Io)?;
        file.sync_all().map_err(SettingsStorageError::Io)?;
        file.commit().map_err(SettingsStorageError::Io)
    }

    pub(crate) fn reset(&self) -> Result<(), SettingsStorageError> {
        match fs::remove_file(self.settings_path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(SettingsStorageError::Io(error)),
        }
    }

    fn settings_path(&self) -> PathBuf {
        self.app_data_dir.join(SETTINGS_FILENAME)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingsLoad {
    Missing,
    Valid(SettingsValues),
    Damaged,
}

#[derive(Debug, Error)]
pub(crate) enum SettingsStorageError {
    #[error("settings contain unsupported values")]
    InvalidValues,
    #[error("settings could not be serialized")]
    Serialization(serde_json::Error),
    #[error("settings could not be accessed")]
    Io(io::Error),
}
