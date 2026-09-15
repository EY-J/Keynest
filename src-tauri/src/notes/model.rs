use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

const MAX_TITLE_LENGTH: usize = 200;
const MAX_CONTENT_LENGTH: usize = 1_000_000;
const MAX_TAG_COUNT: usize = 20;
const MAX_TAG_LENGTH: usize = 50;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NoteInput {
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub favorite: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NoteRecord {
    pub id: String,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub(crate) enum NotesError {
    #[error("note title is invalid")]
    InvalidTitle,
    #[error("note content is invalid")]
    InvalidContent,
    #[error("note tags are invalid")]
    InvalidTags,
    #[error("note was not found")]
    NotFound,
    #[error("notes data is damaged")]
    DataDamaged,
    #[error("secure random data is unavailable")]
    EntropyUnavailable,
    #[error("notes storage is unavailable")]
    StorageUnavailable,
}

impl NoteInput {
    pub(super) fn normalized(mut self) -> Result<Self, NotesError> {
        replace_with_trimmed(&mut self.title);
        if self.title.is_empty() || self.title.chars().count() > MAX_TITLE_LENGTH {
            return Err(NotesError::InvalidTitle);
        }
        if self.content.chars().count() > MAX_CONTENT_LENGTH {
            return Err(NotesError::InvalidContent);
        }
        if self.tags.len() > MAX_TAG_COUNT {
            return Err(NotesError::InvalidTags);
        }
        for tag in &mut self.tags {
            replace_with_trimmed(tag);
            if tag.is_empty() || tag.chars().count() > MAX_TAG_LENGTH {
                return Err(NotesError::InvalidTags);
            }
        }
        let mut index = 0;
        while index < self.tags.len() {
            let duplicate = self.tags[..index]
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&self.tags[index]));
            if duplicate {
                Zeroizing::new(self.tags.remove(index));
            } else {
                index += 1;
            }
        }
        Ok(self)
    }
}

fn replace_with_trimmed(value: &mut String) {
    let trimmed = value.trim().to_owned();
    value.zeroize();
    *value = trimmed;
}

impl fmt::Debug for NoteInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NoteInput")
            .field("note", &"[REDACTED]")
            .finish()
    }
}

impl fmt::Debug for NoteRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NoteRecord")
            .field("id", &self.id)
            .field("note", &"[REDACTED]")
            .field("created_at_ms", &self.created_at_ms)
            .field("updated_at_ms", &self.updated_at_ms)
            .finish()
    }
}
