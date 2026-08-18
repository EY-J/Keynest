use serde::{Deserialize, Serialize};

pub(crate) const AUTO_LOCK_OPTIONS: [u64; 4] = [60, 300, 900, 1800];
pub(crate) const CLIPBOARD_CLEAR_OPTIONS: [u64; 3] = [10, 30, 60];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ThemePreference {
    #[default]
    System,
    Dark,
    Light,
}

impl ThemePreference {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SettingsValues {
    pub auto_lock_seconds: u64,
    pub clipboard_clear_seconds: u64,
    pub theme: ThemePreference,
}

impl Default for SettingsValues {
    fn default() -> Self {
        Self {
            auto_lock_seconds: 300,
            clipboard_clear_seconds: 30,
            theme: ThemePreference::System,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsSnapshot {
    pub auto_lock_seconds: u64,
    pub clipboard_clear_seconds: u64,
    pub theme: ThemePreference,
    pub launch_at_startup: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

impl SettingsValues {
    pub(super) fn is_valid(self) -> bool {
        AUTO_LOCK_OPTIONS.contains(&self.auto_lock_seconds)
            && CLIPBOARD_CLEAR_OPTIONS.contains(&self.clipboard_clear_seconds)
    }
}
