use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use zeroize::Zeroizing;

use crate::security::EntropySource;

use super::{
    crypto::{decrypt, encrypt, FORMAT_VERSION},
    VaultError, VaultRecord, VaultRecordInput, VaultRecordSummary,
};

const VAULT_FILENAME: &str = "vault.enc";
const CREATE_VAULT_TABLE_SQL: &str = "CREATE TABLE vault_records (
    id TEXT PRIMARY KEY NOT NULL,
    format_version INTEGER NOT NULL,
    nonce BLOB NOT NULL,
    ciphertext BLOB NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
)";
const CREATE_VAULT_INDEX_SQL: &str = "CREATE INDEX vault_records_updated_at_idx
    ON vault_records(updated_at_ms DESC, id ASC)";

#[derive(Clone)]
pub(crate) struct VaultService {
    app_data_dir: PathBuf,
    entropy: Arc<dyn EntropySource>,
}

struct StoredRecord {
    id: String,
    format_version: i64,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl VaultService {
    pub(crate) fn new(app_data_dir: PathBuf, entropy: Arc<dyn EntropySource>) -> Self {
        Self {
            app_data_dir,
            entropy,
        }
    }

    pub(crate) fn list(&self, vault_key: &[u8; 32]) -> Result<Vec<VaultRecordSummary>, VaultError> {
        let connection = self.open()?;
        let mut statement = connection
            .prepare(
                "SELECT id, format_version, nonce, ciphertext, created_at_ms, updated_at_ms
                 FROM vault_records ORDER BY updated_at_ms DESC, id ASC",
            )
            .map_err(storage_error)?;
        let records = statement
            .query_map([], stored_record)
            .map_err(storage_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage_error)?;

        records
            .into_iter()
            .map(|record| {
                let input = self.decrypt_input(vault_key, &record)?;
                Ok(summary_for(
                    &record.id,
                    &input,
                    record.created_at_ms,
                    record.updated_at_ms,
                ))
            })
            .collect()
    }

    pub(crate) fn create(
        &self,
        vault_key: &[u8; 32],
        input: VaultRecordInput,
    ) -> Result<VaultRecordSummary, VaultError> {
        let input = input.normalized()?;
        let id = self.new_id()?;
        let now = timestamp()?;
        let encrypted = encrypt(&input, vault_key, &id, self.entropy.as_ref())?;
        let connection = self.open()?;
        connection
            .execute(
                "INSERT INTO vault_records
                 (id, format_version, nonce, ciphertext, created_at_ms, updated_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    id,
                    FORMAT_VERSION,
                    encrypted.nonce,
                    encrypted.ciphertext,
                    now,
                    now
                ],
            )
            .map_err(storage_error)?;

        Ok(summary_for(&id, &input, now, now))
    }

    pub(crate) fn get(&self, vault_key: &[u8; 32], id: &str) -> Result<VaultRecord, VaultError> {
        let connection = self.open()?;
        let record = connection
            .query_row(
                "SELECT id, format_version, nonce, ciphertext, created_at_ms, updated_at_ms
                 FROM vault_records WHERE id = ?1",
                params![id],
                stored_record,
            )
            .optional()
            .map_err(storage_error)?
            .ok_or(VaultError::NotFound)?;
        let input = self.decrypt_input(vault_key, &record)?;
        Ok(record_for(
            record.id,
            &input,
            record.created_at_ms,
            record.updated_at_ms,
        ))
    }

    pub(crate) fn update(
        &self,
        vault_key: &[u8; 32],
        id: &str,
        input: VaultRecordInput,
    ) -> Result<VaultRecordSummary, VaultError> {
        let input = input.normalized()?;
        let mut connection = self.open()?;
        let transaction = connection.transaction().map_err(storage_error)?;
        let stored = transaction
            .query_row(
                "SELECT id, format_version, nonce, ciphertext, created_at_ms, updated_at_ms
                 FROM vault_records WHERE id = ?1",
                params![id],
                stored_record,
            )
            .optional()
            .map_err(storage_error)?
            .ok_or(VaultError::NotFound)?;
        let _existing = self.decrypt_input(vault_key, &stored)?;
        let updated_at_ms = stored
            .updated_at_ms
            .checked_add(1)
            .ok_or(VaultError::DataDamaged)?;
        let now = timestamp()?.max(updated_at_ms);
        let encrypted = encrypt(&input, vault_key, id, self.entropy.as_ref())?;
        let changed = transaction
            .execute(
                "UPDATE vault_records
                 SET format_version = ?1, nonce = ?2, ciphertext = ?3, updated_at_ms = ?4
                 WHERE id = ?5",
                params![
                    FORMAT_VERSION,
                    encrypted.nonce,
                    encrypted.ciphertext,
                    now,
                    id
                ],
            )
            .map_err(storage_error)?;
        if changed != 1 {
            return Err(VaultError::NotFound);
        }
        transaction.commit().map_err(storage_error)?;

        Ok(summary_for(id, &input, stored.created_at_ms, now))
    }

    pub(crate) fn delete(&self, vault_key: &[u8; 32], id: &str) -> Result<(), VaultError> {
        let mut connection = self.open()?;
        let transaction = connection.transaction().map_err(storage_error)?;
        let stored = transaction
            .query_row(
                "SELECT id, format_version, nonce, ciphertext, created_at_ms, updated_at_ms
                 FROM vault_records WHERE id = ?1",
                params![id],
                stored_record,
            )
            .optional()
            .map_err(storage_error)?
            .ok_or(VaultError::NotFound)?;
        let _record = self.decrypt_input(vault_key, &stored)?;
        let changed = transaction
            .execute("DELETE FROM vault_records WHERE id = ?1", params![id])
            .map_err(storage_error)?;
        if changed != 1 {
            return Err(VaultError::DataDamaged);
        }
        transaction.commit().map_err(storage_error)
    }

    pub(crate) fn password_for_copy(
        &self,
        vault_key: &[u8; 32],
        id: &str,
    ) -> Result<Zeroizing<String>, VaultError> {
        let connection = self.open()?;
        let stored = connection
            .query_row(
                "SELECT id, format_version, nonce, ciphertext, created_at_ms, updated_at_ms
                 FROM vault_records WHERE id = ?1",
                params![id],
                stored_record,
            )
            .optional()
            .map_err(storage_error)?
            .ok_or(VaultError::NotFound)?;
        let input = self.decrypt_input(vault_key, &stored)?;
        Ok(Zeroizing::new(input.password.clone()))
    }

    fn open(&self) -> Result<Connection, VaultError> {
        self.open_with_initialization_hook(|| Ok(()))
    }

    fn open_with_initialization_hook(
        &self,
        initialization_hook: impl FnOnce() -> Result<(), VaultError>,
    ) -> Result<Connection, VaultError> {
        fs::create_dir_all(&self.app_data_dir).map_err(|_| VaultError::StorageUnavailable)?;
        let mut connection =
            Connection::open(self.app_data_dir.join(VAULT_FILENAME)).map_err(storage_error)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(storage_error)?;
        connection
            .execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;")
            .map_err(storage_error)?;

        let schema_version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(storage_error)?;
        match schema_version {
            0 => initialize_v1_schema(&mut connection, initialization_hook)?,
            1 => {}
            _ => return Err(VaultError::DataDamaged),
        }
        validate_v1_schema(&connection)?;
        Ok(connection)
    }

    #[cfg(test)]
    pub(super) fn force_mid_schema_initialization_failure_for_test(
        &self,
    ) -> Result<(), VaultError> {
        self.open_with_initialization_hook(|| Err(VaultError::StorageUnavailable))
            .map(drop)
    }

    fn new_id(&self) -> Result<String, VaultError> {
        let mut bytes = [0_u8; 16];
        self.entropy
            .fill(&mut bytes)
            .map_err(|_| VaultError::EntropyUnavailable)?;
        Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    fn decrypt_input(
        &self,
        vault_key: &[u8; 32],
        stored: &StoredRecord,
    ) -> Result<VaultRecordInput, VaultError> {
        decrypt(
            stored.format_version,
            &stored.nonce,
            &stored.ciphertext,
            vault_key,
            &stored.id,
        )
    }
}

fn initialize_v1_schema(
    connection: &mut Connection,
    initialization_hook: impl FnOnce() -> Result<(), VaultError>,
) -> Result<(), VaultError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage_error)?;
    let schema_version: i64 = transaction
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(storage_error)?;
    match schema_version {
        0 => {
            require_empty_v0_schema(&transaction)?;
            transaction
                .execute_batch(CREATE_VAULT_TABLE_SQL)
                .map_err(storage_error)?;
            initialization_hook()?;
            transaction
                .execute_batch(CREATE_VAULT_INDEX_SQL)
                .map_err(storage_error)?;
            transaction
                .execute_batch("PRAGMA user_version = 1;")
                .map_err(storage_error)?;
            validate_v1_schema(&transaction)?;
        }
        1 => validate_v1_schema(&transaction)?,
        _ => return Err(VaultError::DataDamaged),
    }
    transaction.commit().map_err(storage_error)
}

fn record_for(
    id: String,
    input: &VaultRecordInput,
    created_at_ms: i64,
    updated_at_ms: i64,
) -> VaultRecord {
    VaultRecord {
        id,
        name: input.name.clone(),
        username: input.username.clone(),
        password: input.password.clone(),
        website: input.website.clone(),
        category: input.category.clone(),
        tags: input.tags.clone(),
        created_at_ms,
        updated_at_ms,
    }
}

fn summary_for(
    id: &str,
    input: &VaultRecordInput,
    created_at_ms: i64,
    updated_at_ms: i64,
) -> VaultRecordSummary {
    VaultRecordSummary {
        id: id.to_owned(),
        name: input.name.clone(),
        username: input.username.clone(),
        website: input.website.clone(),
        category: input.category.clone(),
        tags: input.tags.clone(),
        created_at_ms,
        updated_at_ms,
    }
}

fn stored_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredRecord> {
    Ok(StoredRecord {
        id: row.get(0)?,
        format_version: row.get(1)?,
        nonce: row.get(2)?,
        ciphertext: row.get(3)?,
        created_at_ms: row.get(4)?,
        updated_at_ms: row.get(5)?,
    })
}

fn require_empty_v0_schema(connection: &Connection) -> Result<(), VaultError> {
    let object_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM sqlite_schema", [], |row| row.get(0))
        .map_err(storage_error)?;
    if object_count == 0 {
        Ok(())
    } else {
        Err(VaultError::DataDamaged)
    }
}

fn timestamp() -> Result<i64, VaultError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| VaultError::StorageUnavailable)?
        .as_millis()
        .try_into()
        .map_err(|_| VaultError::StorageUnavailable)
}

fn storage_error(error: rusqlite::Error) -> VaultError {
    use rusqlite::ffi::ErrorCode;

    match error {
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(
                failure.code,
                ErrorCode::DatabaseCorrupt
                    | ErrorCode::NotADatabase
                    | ErrorCode::TypeMismatch
                    | ErrorCode::SchemaChanged
                    | ErrorCode::ConstraintViolation
            ) =>
        {
            VaultError::DataDamaged
        }
        rusqlite::Error::FromSqlConversionFailure(..)
        | rusqlite::Error::IntegralValueOutOfRange(..)
        | rusqlite::Error::Utf8Error(..)
        | rusqlite::Error::InvalidColumnType(..) => VaultError::DataDamaged,
        _ => VaultError::StorageUnavailable,
    }
}

fn validate_v1_schema(connection: &Connection) -> Result<(), VaultError> {
    let mut objects = connection
        .prepare("SELECT type, name, tbl_name, sql FROM sqlite_schema ORDER BY type, name")
        .map_err(storage_error)?;
    let objects = objects
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(storage_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage_error)?;
    if objects.len() != 3
        || !schema_object_matches(
            &objects[0],
            "index",
            "sqlite_autoindex_vault_records_1",
            "vault_records",
            None,
        )
        || !schema_object_matches(
            &objects[1],
            "index",
            "vault_records_updated_at_idx",
            "vault_records",
            Some(CREATE_VAULT_INDEX_SQL),
        )
        || !schema_object_matches(
            &objects[2],
            "table",
            "vault_records",
            "vault_records",
            Some(CREATE_VAULT_TABLE_SQL),
        )
    {
        return Err(VaultError::DataDamaged);
    }

    let mut columns = connection
        .prepare("PRAGMA table_xinfo(vault_records)")
        .map_err(storage_error)?;
    let columns = columns
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })
        .map_err(storage_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage_error)?;
    let expected_columns = [
        (0, "id", "TEXT", 1, 1, 0),
        (1, "format_version", "INTEGER", 1, 0, 0),
        (2, "nonce", "BLOB", 1, 0, 0),
        (3, "ciphertext", "BLOB", 1, 0, 0),
        (4, "created_at_ms", "INTEGER", 1, 0, 0),
        (5, "updated_at_ms", "INTEGER", 1, 0, 0),
    ];
    if columns.len() != expected_columns.len()
        || !columns
            .iter()
            .zip(expected_columns)
            .all(|(actual, expected)| {
                actual.0 == expected.0
                    && actual.1 == expected.1
                    && actual.2 == expected.2
                    && actual.3 == expected.3
                    && actual.4 == expected.4
                    && actual.5 == expected.5
            })
    {
        return Err(VaultError::DataDamaged);
    }

    let mut index_metadata = connection
        .prepare("PRAGMA index_list(vault_records)")
        .map_err(storage_error)?;
    let index_metadata = index_metadata
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .map_err(storage_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage_error)?;
    let expected_index = index_metadata
        .iter()
        .filter(|metadata| metadata.0 == "vault_records_updated_at_idx")
        .collect::<Vec<_>>();
    let primary_index = index_metadata
        .iter()
        .filter(|metadata| metadata.0 == "sqlite_autoindex_vault_records_1")
        .collect::<Vec<_>>();
    if index_metadata.len() != 2
        || expected_index.len() != 1
        || expected_index[0].1 != 0
        || expected_index[0].2 != "c"
        || expected_index[0].3 != 0
        || primary_index.len() != 1
        || primary_index[0].1 != 1
        || primary_index[0].2 != "pk"
        || primary_index[0].3 != 0
    {
        return Err(VaultError::DataDamaged);
    }

    let mut index_columns = connection
        .prepare("PRAGMA index_xinfo(vault_records_updated_at_idx)")
        .map_err(storage_error)?;
    let index_columns = index_columns
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(storage_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage_error)?
        .into_iter()
        .filter(|column| column.5 == 1)
        .collect::<Vec<_>>();
    let expected_index_columns = [
        (0, 5, Some("updated_at_ms".to_owned()), 1, "BINARY"),
        (1, 0, Some("id".to_owned()), 0, "BINARY"),
    ];
    if index_columns.len() != expected_index_columns.len()
        || !index_columns
            .iter()
            .zip(expected_index_columns)
            .all(|(actual, expected)| {
                actual.0 == expected.0
                    && actual.1 == expected.1
                    && actual.2 == expected.2
                    && actual.3 == expected.3
                    && actual.4 == expected.4
            })
    {
        return Err(VaultError::DataDamaged);
    }

    Ok(())
}

fn schema_object_matches(
    actual: &(String, String, String, Option<String>),
    object_type: &str,
    name: &str,
    table_name: &str,
    sql: Option<&str>,
) -> bool {
    actual.0 == object_type
        && actual.1 == name
        && actual.2 == table_name
        && actual.3.as_deref().map(canonical_schema_sql) == sql.map(canonical_schema_sql)
}

fn canonical_schema_sql(sql: &str) -> String {
    sql.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod error_mapping_tests {
    use super::*;

    fn sqlite_failure(code: i32) -> rusqlite::Error {
        rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(code), None)
    }

    #[test]
    fn sqlite_error_mapping_uses_the_underlying_cause() {
        for code in [
            rusqlite::ffi::SQLITE_CORRUPT,
            rusqlite::ffi::SQLITE_NOTADB,
            rusqlite::ffi::SQLITE_MISMATCH,
        ] {
            assert_eq!(storage_error(sqlite_failure(code)), VaultError::DataDamaged);
        }

        for code in [
            rusqlite::ffi::SQLITE_PERM,
            rusqlite::ffi::SQLITE_BUSY,
            rusqlite::ffi::SQLITE_LOCKED,
            rusqlite::ffi::SQLITE_READONLY,
            rusqlite::ffi::SQLITE_IOERR,
            rusqlite::ffi::SQLITE_FULL,
            rusqlite::ffi::SQLITE_CANTOPEN,
        ] {
            assert_eq!(
                storage_error(sqlite_failure(code)),
                VaultError::StorageUnavailable
            );
        }

        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("PRAGMA query_only = ON;").unwrap();
        let read_only_error = connection
            .execute_batch("CREATE TABLE forbidden (value TEXT);")
            .unwrap_err();
        assert_eq!(
            storage_error(read_only_error),
            VaultError::StorageUnavailable
        );
    }
}
