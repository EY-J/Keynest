use std::sync::{Arc, Mutex, MutexGuard};

use thiserror::Error;

use super::{
    model::{
        SettingsSnapshot, SettingsValues, ThemePreference, AUTO_LOCK_OPTIONS,
        CLIPBOARD_CLEAR_OPTIONS,
    },
    storage::{SettingsLoad, SettingsStorageError, SettingsStore},
};

const DAMAGED_SETTINGS_WARNING: &str =
    "KeyNest restored secure settings defaults because the saved preferences were invalid.";

#[derive(Clone)]
pub(crate) struct SettingsService {
    inner: Arc<Mutex<SettingsInner>>,
    store: SettingsStore,
}

struct SettingsInner {
    values: SettingsValues,
    warning: Option<String>,
}

impl SettingsService {
    pub(crate) fn load(store: SettingsStore) -> Result<Self, SettingsError> {
        let (values, warning) = match store.load()? {
            SettingsLoad::Missing => (SettingsValues::default(), None),
            SettingsLoad::Valid(values) => (values, None),
            SettingsLoad::Damaged => (
                SettingsValues::default(),
                Some(DAMAGED_SETTINGS_WARNING.to_owned()),
            ),
        };

        Ok(Self {
            inner: Arc::new(Mutex::new(SettingsInner { values, warning })),
            store,
        })
    }

    pub(crate) fn snapshot(&self, launch_at_startup: bool) -> SettingsSnapshot {
        let inner = self.lock_inner();
        SettingsSnapshot {
            auto_lock_seconds: inner.values.auto_lock_seconds,
            clipboard_clear_seconds: inner.values.clipboard_clear_seconds,
            theme: inner.values.theme,
            launch_at_startup,
            warning: inner.warning.clone(),
        }
    }

    pub(crate) fn set_auto_lock_seconds(&self, seconds: u64) -> Result<(), SettingsError> {
        if !AUTO_LOCK_OPTIONS.contains(&seconds) {
            return Err(SettingsError::InvalidAutoLockSeconds);
        }

        self.replace_values(|values| values.auto_lock_seconds = seconds)
    }

    pub(crate) fn set_clipboard_clear_seconds(&self, seconds: u64) -> Result<(), SettingsError> {
        if !CLIPBOARD_CLEAR_OPTIONS.contains(&seconds) {
            return Err(SettingsError::InvalidClipboardClearSeconds);
        }

        self.replace_values(|values| values.clipboard_clear_seconds = seconds)
    }

    pub(crate) fn set_theme(&self, theme: ThemePreference) -> Result<(), SettingsError> {
        self.replace_values(|values| values.theme = theme)
    }

    pub(crate) fn reset(&self) -> Result<(), SettingsError> {
        let mut inner = self.lock_inner();
        self.store.reset()?;
        inner.values = SettingsValues::default();
        inner.warning = None;
        Ok(())
    }

    fn replace_values(
        &self,
        update: impl FnOnce(&mut SettingsValues),
    ) -> Result<(), SettingsError> {
        let mut inner = self.lock_inner();
        let mut values = inner.values;
        update(&mut values);
        self.store.replace(values)?;
        inner.values = values;
        Ok(())
    }

    fn lock_inner(&self) -> MutexGuard<'_, SettingsInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Debug, Error)]
pub(crate) enum SettingsError {
    #[error("auto-lock duration is not supported")]
    InvalidAutoLockSeconds,
    #[error("clipboard-clear duration is not supported")]
    InvalidClipboardClearSeconds,
    #[error(transparent)]
    Storage(#[from] SettingsStorageError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{storage::SettingsLoad, SettingsStore, SettingsValues, ThemePreference};

    const DAMAGED_WARNING: &str =
        "KeyNest restored secure settings defaults because the saved preferences were invalid.";

    #[test]
    fn missing_settings_load_as_defaults_without_a_warning() {
        let temp = tempfile::tempdir().unwrap();
        let service = SettingsService::load(SettingsStore::new(temp.path().to_path_buf())).unwrap();

        let snapshot = service.snapshot(false);

        assert_eq!(snapshot.auto_lock_seconds, 300);
        assert_eq!(snapshot.clipboard_clear_seconds, 30);
        assert_eq!(snapshot.theme, ThemePreference::System);
        assert!(!snapshot.launch_at_startup);
        assert_eq!(snapshot.warning, None);
    }

    #[test]
    fn damaged_settings_load_as_defaults_with_a_warning() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("settings.json"), b"not-json").unwrap();
        let service = SettingsService::load(SettingsStore::new(temp.path().to_path_buf())).unwrap();

        assert_eq!(
            service.snapshot(false).warning.as_deref(),
            Some(DAMAGED_WARNING)
        );
    }

    #[test]
    fn valid_mutations_persist_before_updating_memory() {
        let temp = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(temp.path().to_path_buf());
        let service = SettingsService::load(store.clone()).unwrap();

        service.set_auto_lock_seconds(900).unwrap();
        service.set_clipboard_clear_seconds(60).unwrap();
        service.set_theme(ThemePreference::Light).unwrap();

        let expected = SettingsValues {
            auto_lock_seconds: 900,
            clipboard_clear_seconds: 60,
            theme: ThemePreference::Light,
        };
        assert_eq!(store.load().unwrap(), SettingsLoad::Valid(expected));
        let snapshot = service.snapshot(true);
        assert_eq!(snapshot.auto_lock_seconds, expected.auto_lock_seconds);
        assert_eq!(
            snapshot.clipboard_clear_seconds,
            expected.clipboard_clear_seconds
        );
        assert_eq!(snapshot.theme, expected.theme);
        assert!(snapshot.launch_at_startup);
    }

    #[test]
    fn invalid_mutations_leave_settings_unchanged() {
        let temp = tempfile::tempdir().unwrap();
        let service = SettingsService::load(SettingsStore::new(temp.path().to_path_buf())).unwrap();

        assert!(matches!(
            service.set_auto_lock_seconds(301),
            Err(SettingsError::InvalidAutoLockSeconds)
        ));
        assert!(matches!(
            service.set_clipboard_clear_seconds(31),
            Err(SettingsError::InvalidClipboardClearSeconds)
        ));

        let snapshot = service.snapshot(false);
        assert_eq!(snapshot.auto_lock_seconds, 300);
        assert_eq!(snapshot.clipboard_clear_seconds, 30);
    }

    #[test]
    fn failed_persistence_leaves_prior_memory_unchanged() {
        let temp = tempfile::tempdir().unwrap();
        let app_data_dir = temp.path().join("app-data");
        let service = SettingsService::load(SettingsStore::new(app_data_dir.clone())).unwrap();
        std::fs::write(&app_data_dir, b"blocks directory creation").unwrap();

        assert!(matches!(
            service.set_theme(ThemePreference::Dark),
            Err(SettingsError::Storage(_))
        ));
        assert_eq!(service.snapshot(false).theme, ThemePreference::System);
    }

    #[test]
    fn reset_removes_saved_settings_and_restores_defaults() {
        let temp = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(temp.path().to_path_buf());
        let service = SettingsService::load(store.clone()).unwrap();
        service.set_auto_lock_seconds(900).unwrap();

        service.reset().unwrap();

        assert_eq!(store.load().unwrap(), SettingsLoad::Missing);
        assert_eq!(service.snapshot(false).auto_lock_seconds, 300);
        assert_eq!(service.snapshot(false).warning, None);
    }
}
