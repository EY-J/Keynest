use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use thiserror::Error;

use crate::security::EntropySource;

const FILE_NAME: &str = "folders.db";
const SCHEMA: &str = "CREATE TABLE folders (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL COLLATE NOCASE UNIQUE,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);
CREATE TABLE folder_items (
  kind TEXT NOT NULL CHECK (kind IN ('credential', 'note')),
  item_id TEXT NOT NULL,
  folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
  PRIMARY KEY (kind, item_id)
);
CREATE INDEX folder_items_folder_idx ON folder_items(folder_id);";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Folder {
    pub id: String,
    pub name: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FolderItem {
    pub kind: String,
    pub item_id: String,
    pub folder_id: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub(crate) enum FolderError {
    #[error("folder name is invalid")]
    InvalidName,
    #[error("folder name already exists")]
    DuplicateName,
    #[error("folder was not found")]
    NotFound,
    #[error("folder storage is unavailable")]
    StorageUnavailable,
    #[error("folder data is damaged")]
    DataDamaged,
}

#[derive(Clone)]
pub(crate) struct FolderService {
    app_data_dir: PathBuf,
    entropy: Arc<dyn EntropySource>,
}

impl FolderService {
    pub(crate) fn new(app_data_dir: PathBuf, entropy: Arc<dyn EntropySource>) -> Self {
        Self {
            app_data_dir,
            entropy,
        }
    }

    fn open(&self) -> Result<Connection, FolderError> {
        fs::create_dir_all(&self.app_data_dir).map_err(|_| FolderError::StorageUnavailable)?;
        let mut connection =
            Connection::open(self.app_data_dir.join(FILE_NAME)).map_err(storage_error)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(storage_error)?;
        connection
            .execute_batch(
                "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;",
            )
            .map_err(storage_error)?;
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(storage_error)?;
        match version {
            0 => {
                let transaction = connection.transaction().map_err(storage_error)?;
                let objects: i64 = transaction
                    .query_row("SELECT COUNT(*) FROM sqlite_schema", [], |row| row.get(0))
                    .map_err(storage_error)?;
                if objects != 0 {
                    return Err(FolderError::DataDamaged);
                }
                transaction.execute_batch(SCHEMA).map_err(storage_error)?;
                transaction
                    .execute_batch("PRAGMA user_version = 1;")
                    .map_err(storage_error)?;
                transaction.commit().map_err(storage_error)?;
            }
            1 => {}
            _ => return Err(FolderError::DataDamaged),
        }
        Ok(connection)
    }

    pub(crate) fn list(&self) -> Result<Vec<Folder>, FolderError> {
        let connection = self.open()?;
        let mut statement = connection.prepare("SELECT id, name, created_at_ms, updated_at_ms FROM folders ORDER BY name COLLATE NOCASE, id").map_err(storage_error)?;
        let result = statement
            .query_map([], |row| {
                Ok(Folder {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_at_ms: row.get(2)?,
                    updated_at_ms: row.get(3)?,
                })
            })
            .map_err(storage_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage_error);
        result
    }

    pub(crate) fn create(&self, name: &str) -> Result<Folder, FolderError> {
        let name = valid_name(name)?;
        let mut bytes = [0_u8; 16];
        self.entropy
            .fill(&mut bytes)
            .map_err(|_| FolderError::StorageUnavailable)?;
        let id: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        let now = timestamp()?;
        let connection = self.open()?;
        connection.execute("INSERT INTO folders (id, name, created_at_ms, updated_at_ms) VALUES (?1, ?2, ?3, ?3)", params![id, name, now]).map_err(name_error)?;
        Ok(Folder {
            id,
            name,
            created_at_ms: now,
            updated_at_ms: now,
        })
    }

    pub(crate) fn rename(&self, id: &str, name: &str) -> Result<Folder, FolderError> {
        let name = valid_name(name)?;
        let connection = self.open()?;
        let created_at_ms: i64 = connection
            .query_row(
                "SELECT created_at_ms FROM folders WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)?
            .ok_or(FolderError::NotFound)?;
        let now = timestamp()?;
        let changed = connection
            .execute(
                "UPDATE folders SET name = ?1, updated_at_ms = ?2 WHERE id = ?3",
                params![name, now, id],
            )
            .map_err(name_error)?;
        if changed != 1 {
            return Err(FolderError::NotFound);
        }
        Ok(Folder {
            id: id.to_owned(),
            name,
            created_at_ms,
            updated_at_ms: now,
        })
    }

    pub(crate) fn delete(&self, id: &str) -> Result<(), FolderError> {
        let connection = self.open()?;
        if connection
            .execute("DELETE FROM folders WHERE id = ?1", [id])
            .map_err(storage_error)?
            == 0
        {
            return Err(FolderError::NotFound);
        }
        Ok(())
    }

    pub(crate) fn items(&self) -> Result<Vec<FolderItem>, FolderError> {
        let connection = self.open()?;
        let mut statement = connection
            .prepare("SELECT kind, item_id, folder_id FROM folder_items ORDER BY kind, item_id")
            .map_err(storage_error)?;
        let result = statement
            .query_map([], |row| {
                Ok(FolderItem {
                    kind: row.get(0)?,
                    item_id: row.get(1)?,
                    folder_id: row.get(2)?,
                })
            })
            .map_err(storage_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage_error);
        result
    }

    pub(crate) fn contents(&self, folder_id: &str) -> Result<Vec<FolderItem>, FolderError> {
        let connection = self.open()?;
        self.require_folder(&connection, folder_id)?;
        let mut statement = connection.prepare("SELECT kind, item_id, folder_id FROM folder_items WHERE folder_id = ?1 ORDER BY kind, item_id").map_err(storage_error)?;
        let result = statement
            .query_map([folder_id], |row| {
                Ok(FolderItem {
                    kind: row.get(0)?,
                    item_id: row.get(1)?,
                    folder_id: row.get(2)?,
                })
            })
            .map_err(storage_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage_error);
        result
    }

    pub(crate) fn assign(
        &self,
        kind: &str,
        item_id: &str,
        folder_id: Option<&str>,
    ) -> Result<(), FolderError> {
        if !matches!(kind, "credential" | "note") || item_id.is_empty() {
            return Err(FolderError::DataDamaged);
        }
        let connection = self.open()?;
        if let Some(folder_id) = folder_id {
            self.require_folder(&connection, folder_id)?;
            connection
                .execute(
                    "INSERT INTO folder_items (kind, item_id, folder_id) VALUES (?1, ?2, ?3)
                ON CONFLICT(kind, item_id) DO UPDATE SET folder_id = excluded.folder_id",
                    params![kind, item_id, folder_id],
                )
                .map_err(|error| {
                    if matches!(&error, rusqlite::Error::SqliteFailure(failure, _) if failure.code == rusqlite::ffi::ErrorCode::ConstraintViolation) {
                        FolderError::NotFound
                    } else { storage_error(error) }
                })?;
        } else {
            connection
                .execute(
                    "DELETE FROM folder_items WHERE kind = ?1 AND item_id = ?2",
                    params![kind, item_id],
                )
                .map_err(storage_error)?;
        }
        Ok(())
    }

    fn require_folder(&self, connection: &Connection, id: &str) -> Result<(), FolderError> {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM folders WHERE id = ?1)",
                [id],
                |row| row.get(0),
            )
            .map_err(storage_error)?;
        if exists {
            Ok(())
        } else {
            Err(FolderError::NotFound)
        }
    }
}

fn valid_name(name: &str) -> Result<String, FolderError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 || name.chars().any(char::is_control) {
        return Err(FolderError::InvalidName);
    }
    Ok(name.to_owned())
}

fn timestamp() -> Result<i64, FolderError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| FolderError::StorageUnavailable)?
        .as_millis()
        .try_into()
        .map_err(|_| FolderError::StorageUnavailable)
}

fn name_error(error: rusqlite::Error) -> FolderError {
    if let rusqlite::Error::SqliteFailure(failure, _) = &error {
        if failure.code == rusqlite::ffi::ErrorCode::ConstraintViolation {
            return FolderError::DuplicateName;
        }
    }
    storage_error(error)
}

fn storage_error(error: rusqlite::Error) -> FolderError {
    match error {
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(
                failure.code,
                rusqlite::ffi::ErrorCode::DatabaseCorrupt
                    | rusqlite::ffi::ErrorCode::NotADatabase
                    | rusqlite::ffi::ErrorCode::SchemaChanged
            ) =>
        {
            FolderError::DataDamaged
        }
        _ => FolderError::StorageUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::CryptoError;
    use crate::{
        notes::{NoteInput, NotesService},
        vault::{VaultRecordInput, VaultService},
    };
    use std::sync::{Arc, Mutex};

    struct Entropy(Mutex<u8>);
    impl EntropySource for Entropy {
        fn fill(&self, destination: &mut [u8]) -> Result<(), CryptoError> {
            let mut next = self.0.lock().unwrap();
            for byte in destination {
                *byte = *next;
                *next = next.wrapping_add(1);
            }
            Ok(())
        }
    }

    #[test]
    fn folders_organize_existing_records_without_changing_encrypted_stores() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().to_path_buf();
        let entropy = Arc::new(Entropy(Mutex::new(1)));
        let folders = FolderService::new(path.clone(), entropy.clone());
        let vault = VaultService::new(path.clone(), entropy.clone());
        let notes = NotesService::new(path.clone(), entropy);
        let key = [7_u8; 32];
        let credential = vault
            .create(
                &key,
                VaultRecordInput {
                    name: "Account".into(),
                    username: "user".into(),
                    password: "secret".into(),
                    website: None,
                    allowed_login_hosts: vec![],
                    tags: vec![],
                },
            )
            .unwrap();
        let note = notes
            .create(
                &key,
                NoteInput {
                    title: "Note".into(),
                    content: "private".into(),
                    tags: vec![],
                    favorite: true,
                },
            )
            .unwrap();
        let vault_bytes = fs::read(path.join("vault.enc")).unwrap();
        let notes_bytes = fs::read(path.join("notes.enc")).unwrap();

        assert!(folders.list().unwrap().is_empty());
        assert!(folders.items().unwrap().is_empty());
        let work = folders.create(" Work ").unwrap();
        assert_eq!(work.name, "Work");
        assert_eq!(folders.create("work"), Err(FolderError::DuplicateName));
        assert_eq!(folders.create(" "), Err(FolderError::InvalidName));
        let personal = folders.create("Personal").unwrap();
        assert_eq!(
            folders.rename(&personal.id, "Personal notes").unwrap().name,
            "Personal notes"
        );
        assert_eq!(
            folders.rename(&personal.id, "WORK"),
            Err(FolderError::DuplicateName)
        );
        folders
            .assign("credential", &credential.id, Some(&work.id))
            .unwrap();
        folders.assign("note", &note.id, Some(&work.id)).unwrap();
        assert_eq!(folders.contents(&work.id).unwrap().len(), 2);
        folders
            .assign("credential", &credential.id, Some(&personal.id))
            .unwrap();
        folders
            .assign("note", &note.id, Some(&personal.id))
            .unwrap();
        assert!(folders.contents(&work.id).unwrap().is_empty());
        folders.assign("credential", &credential.id, None).unwrap();
        folders.assign("note", &note.id, None).unwrap();
        assert!(folders.items().unwrap().is_empty());
        assert_eq!(
            folders.assign("note", &note.id, Some("missing")),
            Err(FolderError::NotFound)
        );
        assert_eq!(folders.contents("missing"), Err(FolderError::NotFound));
        folders.delete(&work.id).unwrap();
        folders
            .assign("credential", &credential.id, Some(&personal.id))
            .unwrap();
        folders
            .assign("note", &note.id, Some(&personal.id))
            .unwrap();
        folders.delete(&personal.id).unwrap();
        assert!(folders.items().unwrap().is_empty());
        assert_eq!(folders.delete(&personal.id), Err(FolderError::NotFound));
        assert_eq!(vault.list(&key).unwrap(), vec![credential.clone()]);
        assert_eq!(notes.list(&key).unwrap(), vec![note.clone()]);
        assert_eq!(fs::read(path.join("vault.enc")).unwrap(), vault_bytes);
        assert_eq!(fs::read(path.join("notes.enc")).unwrap(), notes_bytes);

        let recovered = folders.create("Recovered").unwrap();
        folders
            .assign("credential", &credential.id, Some(&recovered.id))
            .unwrap();
        folders
            .assign("note", &note.id, Some(&recovered.id))
            .unwrap();
        vault.delete(&key, &credential.id).unwrap();
        notes.delete(&key, &note.id).unwrap();
        assert!(vault.list(&key).unwrap().is_empty());
        assert!(notes.list(&key).unwrap().is_empty());
        vault.restore(&key, &credential.id).unwrap();
        notes.restore(&key, &note.id).unwrap();
        assert_eq!(folders.contents(&recovered.id).unwrap().len(), 2);
        assert_eq!(vault.list(&key).unwrap().len(), 1);
        assert_eq!(notes.list(&key).unwrap().len(), 1);
    }
}
