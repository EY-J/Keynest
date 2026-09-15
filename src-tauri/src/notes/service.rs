use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::recently_deleted::{DeletedItem, DeletedItemSource, DeletedItemType};
use crate::security::EntropySource;

use super::{
    crypto::{decrypt, encrypt, FORMAT_VERSION},
    NoteInput, NoteRecord, NotesError,
};

const NOTES_FILENAME: &str = "notes.enc";
const SCHEMA_V2: &str = "CREATE TABLE notes (
    id TEXT PRIMARY KEY NOT NULL,
    format_version INTEGER NOT NULL,
    nonce BLOB NOT NULL,
    ciphertext BLOB NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    deleted_at_ms INTEGER
);
CREATE INDEX notes_updated_at_idx ON notes(updated_at_ms DESC, id ASC);
PRAGMA user_version = 2;";

#[derive(Clone)]
pub(crate) struct NotesService {
    app_data_dir: PathBuf,
    entropy: Arc<dyn EntropySource>,
}

struct StoredNote {
    id: String,
    format_version: i64,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
    created_at_ms: i64,
    updated_at_ms: i64,
    deleted_at_ms: Option<i64>,
}

impl NotesService {
    pub(crate) fn new(app_data_dir: PathBuf, entropy: Arc<dyn EntropySource>) -> Self {
        Self {
            app_data_dir,
            entropy,
        }
    }

    pub(crate) fn list(&self, key: &[u8; 32]) -> Result<Vec<NoteRecord>, NotesError> {
        let connection = self.open()?;
        let mut statement = connection
            .prepare("SELECT id, format_version, nonce, ciphertext, created_at_ms, updated_at_ms, deleted_at_ms FROM notes WHERE deleted_at_ms IS NULL ORDER BY updated_at_ms DESC, id ASC")
            .map_err(storage_error)?;
        let stored = statement
            .query_map([], stored_note)
            .map_err(storage_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage_error)?;
        stored
            .into_iter()
            .map(|note| self.decrypt_record(key, note))
            .collect()
    }

    pub(crate) fn create(
        &self,
        key: &[u8; 32],
        input: NoteInput,
    ) -> Result<NoteRecord, NotesError> {
        let input = input.normalized()?;
        let id = self.new_id()?;
        let now = timestamp()?;
        let encrypted = encrypt(&input, key, &id, self.entropy.as_ref())?;
        self.open()?.execute(
            "INSERT INTO notes (id, format_version, nonce, ciphertext, created_at_ms, updated_at_ms, deleted_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
            params![id, FORMAT_VERSION, encrypted.nonce, encrypted.ciphertext, now, now],
        ).map_err(storage_error)?;
        Ok(record_for(id, &input, now, now))
    }

    pub(crate) fn update(
        &self,
        key: &[u8; 32],
        id: &str,
        input: NoteInput,
    ) -> Result<NoteRecord, NotesError> {
        let input = input.normalized()?;
        let mut connection = self.open()?;
        let transaction = connection.transaction().map_err(storage_error)?;
        let stored = transaction.query_row(
            "SELECT id, format_version, nonce, ciphertext, created_at_ms, updated_at_ms, deleted_at_ms FROM notes WHERE id = ?1 AND deleted_at_ms IS NULL",
            params![id], stored_note,
        ).optional().map_err(storage_error)?.ok_or(NotesError::NotFound)?;
        let _existing = decrypt(
            stored.format_version,
            &stored.nonce,
            &stored.ciphertext,
            key,
            id,
        )?;
        let now = timestamp()?.max(
            stored
                .updated_at_ms
                .checked_add(1)
                .ok_or(NotesError::DataDamaged)?,
        );
        let encrypted = encrypt(&input, key, id, self.entropy.as_ref())?;
        let changed = transaction.execute(
            "UPDATE notes SET format_version = ?1, nonce = ?2, ciphertext = ?3, updated_at_ms = ?4 WHERE id = ?5 AND deleted_at_ms IS NULL",
            params![FORMAT_VERSION, encrypted.nonce, encrypted.ciphertext, now, id],
        ).map_err(storage_error)?;
        if changed != 1 {
            return Err(NotesError::NotFound);
        }
        transaction.commit().map_err(storage_error)?;
        Ok(record_for(id.to_owned(), &input, stored.created_at_ms, now))
    }

    pub(crate) fn delete(&self, key: &[u8; 32], id: &str) -> Result<(), NotesError> {
        let mut connection = self.open()?;
        let transaction = connection.transaction().map_err(storage_error)?;
        let stored = transaction.query_row(
            "SELECT id, format_version, nonce, ciphertext, created_at_ms, updated_at_ms, deleted_at_ms FROM notes WHERE id = ?1 AND deleted_at_ms IS NULL",
            params![id], stored_note,
        ).optional().map_err(storage_error)?.ok_or(NotesError::NotFound)?;
        let _existing = decrypt(
            stored.format_version,
            &stored.nonce,
            &stored.ciphertext,
            key,
            id,
        )?;
        let deleted_at_ms = timestamp()?;
        if transaction
            .execute(
                "UPDATE notes SET deleted_at_ms = ?1 WHERE id = ?2 AND deleted_at_ms IS NULL",
                params![deleted_at_ms, id],
            )
            .map_err(storage_error)?
            != 1
        {
            return Err(NotesError::DataDamaged);
        }
        transaction.commit().map_err(storage_error)
    }

    pub(crate) fn list_deleted(&self, key: &[u8; 32]) -> Result<Vec<DeletedItem>, NotesError> {
        let connection = self.open()?;
        let mut statement = connection
            .prepare("SELECT id, format_version, nonce, ciphertext, created_at_ms, updated_at_ms, deleted_at_ms FROM notes WHERE deleted_at_ms IS NOT NULL ORDER BY deleted_at_ms DESC, id ASC")
            .map_err(storage_error)?;
        let stored = statement
            .query_map([], stored_note)
            .map_err(storage_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage_error)?;
        stored
            .into_iter()
            .map(|note| {
                let deleted_at_ms = note.deleted_at_ms.ok_or(NotesError::DataDamaged)?;
                let input = decrypt(
                    note.format_version,
                    &note.nonce,
                    &note.ciphertext,
                    key,
                    &note.id,
                )?;
                Ok(DeletedItem {
                    id: note.id,
                    item_type: DeletedItemType::Note,
                    title: input.title.clone(),
                    deleted_at_ms,
                    original_source: DeletedItemSource::Notes,
                })
            })
            .collect()
    }

    pub(crate) fn restore(&self, key: &[u8; 32], id: &str) -> Result<(), NotesError> {
        let mut connection = self.open()?;
        let transaction = connection.transaction().map_err(storage_error)?;
        let stored = transaction.query_row(
            "SELECT id, format_version, nonce, ciphertext, created_at_ms, updated_at_ms, deleted_at_ms FROM notes WHERE id = ?1 AND deleted_at_ms IS NOT NULL",
            params![id], stored_note,
        ).optional().map_err(storage_error)?.ok_or(NotesError::NotFound)?;
        let _existing = decrypt(
            stored.format_version,
            &stored.nonce,
            &stored.ciphertext,
            key,
            id,
        )?;
        if transaction
            .execute(
                "UPDATE notes SET deleted_at_ms = NULL WHERE id = ?1 AND deleted_at_ms IS NOT NULL",
                params![id],
            )
            .map_err(storage_error)?
            != 1
        {
            return Err(NotesError::NotFound);
        }
        transaction.commit().map_err(storage_error)
    }

    pub(crate) fn permanently_delete(&self, key: &[u8; 32], id: &str) -> Result<(), NotesError> {
        let mut connection = self.open()?;
        let transaction = connection.transaction().map_err(storage_error)?;
        let stored = transaction.query_row(
            "SELECT id, format_version, nonce, ciphertext, created_at_ms, updated_at_ms, deleted_at_ms FROM notes WHERE id = ?1 AND deleted_at_ms IS NOT NULL",
            params![id], stored_note,
        ).optional().map_err(storage_error)?.ok_or(NotesError::NotFound)?;
        let _existing = decrypt(
            stored.format_version,
            &stored.nonce,
            &stored.ciphertext,
            key,
            id,
        )?;
        if transaction
            .execute(
                "DELETE FROM notes WHERE id = ?1 AND deleted_at_ms IS NOT NULL",
                params![id],
            )
            .map_err(storage_error)?
            != 1
        {
            return Err(NotesError::DataDamaged);
        }
        transaction.commit().map_err(storage_error)
    }

    pub(crate) fn purge_deleted_before(
        &self,
        key: &[u8; 32],
        cutoff_ms: i64,
    ) -> Result<(), NotesError> {
        let items = self.list_deleted(key)?;
        for item in items
            .into_iter()
            .filter(|item| item.deleted_at_ms < cutoff_ms)
        {
            self.permanently_delete(key, &item.id)?;
        }
        Ok(())
    }

    fn open(&self) -> Result<Connection, NotesError> {
        fs::create_dir_all(&self.app_data_dir).map_err(|_| NotesError::StorageUnavailable)?;
        let mut connection =
            Connection::open(self.app_data_dir.join(NOTES_FILENAME)).map_err(storage_error)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(storage_error)?;
        connection
            .execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;")
            .map_err(storage_error)?;
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(storage_error)?;
        match version {
            0 => {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(storage_error)?;
                let objects: i64 = transaction
                    .query_row("SELECT COUNT(*) FROM sqlite_schema", [], |row| row.get(0))
                    .map_err(storage_error)?;
                if objects != 0 {
                    return Err(NotesError::DataDamaged);
                }
                transaction
                    .execute_batch(SCHEMA_V2)
                    .map_err(storage_error)?;
                transaction.commit().map_err(storage_error)?;
            }
            1 => {
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(storage_error)?;
                validate_schema(&transaction, 6)?;
                transaction.execute_batch("ALTER TABLE notes ADD COLUMN deleted_at_ms INTEGER; PRAGMA user_version = 2;").map_err(storage_error)?;
                validate_schema(&transaction, 7)?;
                transaction.commit().map_err(storage_error)?;
            }
            2 => {}
            _ => return Err(NotesError::DataDamaged),
        }
        validate_schema(&connection, 7)?;
        Ok(connection)
    }

    fn new_id(&self) -> Result<String, NotesError> {
        let mut bytes = [0_u8; 16];
        self.entropy
            .fill(&mut bytes)
            .map_err(|_| NotesError::EntropyUnavailable)?;
        Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    fn decrypt_record(&self, key: &[u8; 32], stored: StoredNote) -> Result<NoteRecord, NotesError> {
        let input = decrypt(
            stored.format_version,
            &stored.nonce,
            &stored.ciphertext,
            key,
            &stored.id,
        )?;
        Ok(record_for(
            stored.id,
            &input,
            stored.created_at_ms,
            stored.updated_at_ms,
        ))
    }
}

fn validate_schema(connection: &Connection, expected_columns: i64) -> Result<(), NotesError> {
    let columns: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('notes')",
            [],
            |row| row.get(0),
        )
        .map_err(storage_error)?;
    let index: i64 = connection
        .query_row("SELECT COUNT(*) FROM sqlite_schema WHERE type = 'index' AND name = 'notes_updated_at_idx'", [], |row| row.get(0))
        .map_err(storage_error)?;
    if columns == expected_columns && index == 1 {
        Ok(())
    } else {
        Err(NotesError::DataDamaged)
    }
}

fn stored_note(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredNote> {
    Ok(StoredNote {
        id: row.get(0)?,
        format_version: row.get(1)?,
        nonce: row.get(2)?,
        ciphertext: row.get(3)?,
        created_at_ms: row.get(4)?,
        updated_at_ms: row.get(5)?,
        deleted_at_ms: row.get(6)?,
    })
}

fn record_for(id: String, input: &NoteInput, created_at_ms: i64, updated_at_ms: i64) -> NoteRecord {
    NoteRecord {
        id,
        title: input.title.clone(),
        content: input.content.clone(),
        tags: input.tags.clone(),
        favorite: input.favorite,
        created_at_ms,
        updated_at_ms,
    }
}

fn timestamp() -> Result<i64, NotesError> {
    let milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| NotesError::StorageUnavailable)?
        .as_millis();
    i64::try_from(milliseconds).map_err(|_| NotesError::StorageUnavailable)
}

fn storage_error(_: rusqlite::Error) -> NotesError {
    NotesError::StorageUnavailable
}
