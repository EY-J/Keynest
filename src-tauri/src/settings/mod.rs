mod model;
mod service;
mod storage;

pub(crate) use model::{SettingsSnapshot, SettingsValues, ThemePreference};
pub(crate) use service::{SettingsError, SettingsService};
pub(crate) use storage::SettingsStore;

#[cfg(test)]
mod tests {
    use super::{storage::SettingsLoad, SettingsStore, SettingsValues, ThemePreference};

    fn assert_file_is_damaged(contents: &[u8]) {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("settings.json"), contents).unwrap();
        let store = SettingsStore::new(temp.path().to_path_buf());

        assert_eq!(store.load().unwrap(), SettingsLoad::Damaged);
    }

    #[test]
    fn missing_settings_use_secure_defaults() {
        let temp = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(temp.path().to_path_buf());

        assert_eq!(store.load().unwrap(), SettingsLoad::Missing);
        assert_eq!(SettingsValues::default().auto_lock_seconds, 300);
        assert_eq!(SettingsValues::default().clipboard_clear_seconds, 30);
        assert_eq!(SettingsValues::default().theme, ThemePreference::System);
    }

    #[test]
    fn invalid_security_values_are_damaged() {
        assert_file_is_damaged(
            br#"{"format_version":1,"auto_lock_seconds":0,"clipboard_clear_seconds":0,"theme":"dark"}"#,
        );
    }

    #[test]
    fn replacement_round_trips_allowed_values() {
        let temp = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(temp.path().to_path_buf());
        let values = SettingsValues {
            auto_lock_seconds: 900,
            clipboard_clear_seconds: 60,
            theme: ThemePreference::Light,
        };

        store.replace(values).unwrap();

        assert_eq!(store.load().unwrap(), SettingsLoad::Valid(values));
    }

    #[test]
    fn unknown_fields_are_damaged() {
        assert_file_is_damaged(
            br#"{"format_version":1,"auto_lock_seconds":300,"clipboard_clear_seconds":30,"theme":"system","unexpected":true}"#,
        );
    }

    #[test]
    fn unsupported_versions_are_damaged() {
        assert_file_is_damaged(
            br#"{"format_version":2,"auto_lock_seconds":300,"clipboard_clear_seconds":30,"theme":"system"}"#,
        );
    }

    #[test]
    fn unlisted_auto_lock_values_are_damaged() {
        assert_file_is_damaged(
            br#"{"format_version":1,"auto_lock_seconds":301,"clipboard_clear_seconds":30,"theme":"system"}"#,
        );
    }

    #[test]
    fn unlisted_clipboard_clear_values_are_damaged() {
        assert_file_is_damaged(
            br#"{"format_version":1,"auto_lock_seconds":300,"clipboard_clear_seconds":31,"theme":"system"}"#,
        );
    }

    #[test]
    fn unknown_themes_are_damaged() {
        assert_file_is_damaged(
            br#"{"format_version":1,"auto_lock_seconds":300,"clipboard_clear_seconds":30,"theme":"blue"}"#,
        );
    }
}
