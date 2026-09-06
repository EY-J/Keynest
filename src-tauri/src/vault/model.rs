use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

const MAX_NAME_LENGTH: usize = 200;
const MAX_USERNAME_LENGTH: usize = 500;
const MAX_PASSWORD_LENGTH: usize = 4_096;
const MAX_WEBSITE_LENGTH: usize = 2_048;
const MAX_TAG_COUNT: usize = 20;
const MAX_TAG_LENGTH: usize = 50;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VaultRecordInput {
    pub name: String,
    pub username: String,
    pub password: String,
    pub website: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VaultRecordSummary {
    pub id: String,
    pub name: String,
    pub username: String,
    pub website: Option<String>,
    pub tags: Vec<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, PartialEq, Eq, Serialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VaultRecord {
    pub id: String,
    pub name: String,
    pub username: String,
    pub password: String,
    pub website: Option<String>,
    pub tags: Vec<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl From<&VaultRecord> for VaultRecordSummary {
    fn from(record: &VaultRecord) -> Self {
        Self {
            id: record.id.clone(),
            name: record.name.clone(),
            username: record.username.clone(),
            website: record.website.clone(),
            tags: record.tags.clone(),
            created_at_ms: record.created_at_ms,
            updated_at_ms: record.updated_at_ms,
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub(crate) enum VaultError {
    #[error("credential name is invalid")]
    InvalidName,
    #[error("credential username is invalid")]
    InvalidUsername,
    #[error("credential password is invalid")]
    InvalidPassword,
    #[error("credential website is invalid")]
    InvalidWebsite,
    #[error("credential tags are invalid")]
    InvalidTags,
    #[error("credential was not found")]
    NotFound,
    #[error("vault data is damaged")]
    DataDamaged,
    #[error("secure random data is unavailable")]
    EntropyUnavailable,
    #[error("vault storage is unavailable")]
    StorageUnavailable,
}

impl VaultRecordInput {
    pub(super) fn normalized(mut self) -> Result<Self, VaultError> {
        trim_and_replace(&mut self.name);
        if self.name.is_empty() || character_count(&self.name) > MAX_NAME_LENGTH {
            return Err(VaultError::InvalidName);
        }

        trim_and_replace(&mut self.username);
        if self.username.is_empty() || character_count(&self.username) > MAX_USERNAME_LENGTH {
            return Err(VaultError::InvalidUsername);
        }

        if self.password.trim().is_empty() || character_count(&self.password) > MAX_PASSWORD_LENGTH
        {
            return Err(VaultError::InvalidPassword);
        }

        if let Some(website) = self.website.as_mut() {
            trim_and_replace(website);
            if character_count(website) > MAX_WEBSITE_LENGTH {
                return Err(VaultError::InvalidWebsite);
            }
        }
        if self.website.as_deref() == Some("") {
            self.website.zeroize();
            self.website = None;
        }

        if self.tags.len() > MAX_TAG_COUNT {
            return Err(VaultError::InvalidTags);
        }
        for tag in &mut self.tags {
            trim_and_replace(tag);
            if tag.is_empty() || character_count(tag) > MAX_TAG_LENGTH {
                return Err(VaultError::InvalidTags);
            }
        }
        let mut index = 0;
        while index < self.tags.len() {
            let is_duplicate = self.tags[..index]
                .iter()
                .any(|existing| case_insensitive_eq(existing, &self.tags[index]));
            if is_duplicate {
                Zeroizing::new(self.tags.remove(index));
            } else {
                index += 1;
            }
        }

        Ok(self)
    }
}

impl fmt::Debug for VaultRecordInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VaultRecordInput")
            .field("credential", &"[REDACTED]")
            .finish()
    }
}

impl fmt::Debug for VaultRecordSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VaultRecordSummary")
            .field("credential", &"[REDACTED]")
            .finish()
    }
}

impl fmt::Debug for VaultRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VaultRecord")
            .field("id", &self.id)
            .field("credential", &"[REDACTED]")
            .field("created_at_ms", &self.created_at_ms)
            .field("updated_at_ms", &self.updated_at_ms)
            .finish()
    }
}

fn trim_and_replace(value: &mut String) {
    let trimmed = value.trim().to_owned();
    value.zeroize();
    *value = trimmed;
}

fn case_insensitive_eq(left: &str, right: &str) -> bool {
    let left = zeroize::Zeroizing::new(left.to_lowercase());
    let right = zeroize::Zeroizing::new(right.to_lowercase());
    left == right
}

fn character_count(value: &str) -> usize {
    value.chars().count()
}

#[cfg(test)]
mod logging_tests {
    use super::*;
    #[test]
    fn decrypted_summary_debug_omits_all_user_fields() {
        let secret = "sentinel-decrypted-metadata";
        let summary = VaultRecordSummary {
            id: secret.into(),
            name: secret.into(),
            username: secret.into(),
            website: Some(secret.into()),
            tags: vec![secret.into()],
            created_at_ms: 0,
            updated_at_ms: 0,
        };
        assert!(!format!("{summary:?}").contains(secret));
    }
}
