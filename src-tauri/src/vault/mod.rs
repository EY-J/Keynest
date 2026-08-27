mod crypto;
mod model;
mod service;

pub(crate) use model::{VaultError, VaultRecord, VaultRecordInput, VaultRecordSummary};
#[allow(unused_imports)]
pub(crate) use service::VaultService;

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::{Arc, Mutex},
        thread,
        time::Duration,
    };

    use crate::security::{CryptoError, EntropySource};
    use zeroize::Zeroize;

    use super::{VaultError, VaultRecord, VaultRecordInput, VaultService};

    struct DeterministicEntropy(Mutex<u8>);

    struct UnavailableEntropy;

    impl DeterministicEntropy {
        fn new() -> Self {
            Self(Mutex::new(1))
        }
    }

    impl EntropySource for DeterministicEntropy {
        fn fill(&self, destination: &mut [u8]) -> Result<(), CryptoError> {
            let mut next = self.0.lock().unwrap();
            for byte in destination {
                *byte = *next;
                *next = next.wrapping_add(1);
            }
            Ok(())
        }
    }

    impl EntropySource for UnavailableEntropy {
        fn fill(&self, _: &mut [u8]) -> Result<(), CryptoError> {
            Err(CryptoError::EntropyUnavailable)
        }
    }

    fn service(path: PathBuf) -> VaultService {
        VaultService::new(path, Arc::new(DeterministicEntropy::new()))
    }

    fn unavailable_entropy_service(path: PathBuf) -> VaultService {
        VaultService::new(path, Arc::new(UnavailableEntropy))
    }

    fn vault_key() -> [u8; 32] {
        [7; 32]
    }

    fn input() -> VaultRecordInput {
        VaultRecordInput {
            name: "Example account".to_owned(),
            username: "alex@example.test".to_owned(),
            password: "correct horse battery staple".to_owned(),
            website: Some("https://example.test".to_owned()),
            category: "Personal".to_owned(),
            tags: vec!["Important".to_owned()],
        }
    }

    fn secure_replace<T: Zeroize>(target: &mut T, replacement: T) {
        target.zeroize();
        *target = replacement;
    }

    macro_rules! input_with {
        ($($field:ident: $value:expr),+ $(,)?) => {{
            let mut value = input();
            $(secure_replace(&mut value.$field, $value);)+
            value
        }};
    }

    fn stored_shell(path: &std::path::Path, id: &str) -> (Vec<u8>, Vec<u8>) {
        let connection = rusqlite::Connection::open(path.join("vault.enc")).unwrap();
        connection
            .query_row(
                "SELECT nonce, ciphertext FROM vault_records WHERE id = ?1",
                rusqlite::params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
    }

    fn schema_snapshot(path: &std::path::Path) -> (i64, Vec<(String, String, Option<String>)>) {
        let connection = rusqlite::Connection::open(path.join("vault.enc")).unwrap();
        let version = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        let mut statement = connection
            .prepare("SELECT type, name, sql FROM sqlite_schema ORDER BY type, name")
            .unwrap();
        let objects = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        (version, objects)
    }

    #[test]
    fn secret_bearing_dtos_zeroize_and_redact_debug_output() {
        fn assert_zeroizing<T: zeroize::Zeroize + zeroize::ZeroizeOnDrop>() {}

        assert_zeroizing::<VaultRecordInput>();
        assert_zeroizing::<VaultRecord>();

        let temp = tempfile::tempdir().unwrap();
        let service = service(temp.path().to_path_buf());
        let key = vault_key();
        let secret_input = VaultRecordInput {
            name: "debug-secret-name".to_owned(),
            username: "debug-secret-username".to_owned(),
            password: "debug-secret-password".to_owned(),
            website: Some("debug-secret-website".to_owned()),
            category: "debug-secret-category".to_owned(),
            tags: vec!["debug-secret-tag".to_owned()],
        };
        let input_debug = format!("{secret_input:?}");

        assert!(input_debug.contains("[REDACTED]"));
        for secret in [
            "debug-secret-name",
            "debug-secret-username",
            "debug-secret-password",
            "debug-secret-website",
            "debug-secret-category",
            "debug-secret-tag",
        ] {
            assert!(!input_debug.contains(secret), "input Debug leaked {secret}");
        }

        let summary = service.create(&key, secret_input).unwrap();
        let record = service.get(&key, &summary.id).unwrap();
        let record_debug = format!("{record:?}");

        assert!(record_debug.contains("[REDACTED]"));
        for secret in [
            "debug-secret-name",
            "debug-secret-username",
            "debug-secret-password",
            "debug-secret-website",
            "debug-secret-category",
            "debug-secret-tag",
        ] {
            assert!(
                !record_debug.contains(secret),
                "record Debug leaked {secret}"
            );
        }
    }

    #[test]
    fn version_zero_with_application_objects_is_rejected_without_mutation() {
        let temp = tempfile::tempdir().unwrap();
        let vault_path = temp.path().join("vault.enc");
        let connection = rusqlite::Connection::open(&vault_path).unwrap();
        connection
            .execute_batch("CREATE TABLE foreign_application_data (value TEXT);")
            .unwrap();
        drop(connection);
        let bytes_before = fs::read(&vault_path).unwrap();
        let schema_before = schema_snapshot(temp.path());

        assert_eq!(
            service(temp.path().to_path_buf()).list(&vault_key()),
            Err(VaultError::DataDamaged)
        );

        assert_eq!(fs::read(&vault_path).unwrap(), bytes_before);
        assert_eq!(schema_snapshot(temp.path()), schema_before);
        assert_eq!(schema_before.0, 0);
    }

    #[test]
    fn version_zero_initialization_rolls_back_a_forced_mid_ddl_failure() {
        let temp = tempfile::tempdir().unwrap();
        let vault_path = temp.path().join("vault.enc");
        let connection = rusqlite::Connection::open(&vault_path).unwrap();
        connection.execute_batch("VACUUM;").unwrap();
        drop(connection);
        let bytes_before = fs::read(&vault_path).unwrap();
        let schema_before = schema_snapshot(temp.path());
        let service = service(temp.path().to_path_buf());

        assert_eq!(
            service.force_mid_schema_initialization_failure_for_test(),
            Err(VaultError::StorageUnavailable)
        );

        assert_eq!(fs::read(&vault_path).unwrap(), bytes_before);
        assert_eq!(schema_snapshot(temp.path()), schema_before);
        assert_eq!(schema_before, (0, Vec::new()));
    }

    #[test]
    fn validation_rejects_literal_field_boundaries() {
        let temp = tempfile::tempdir().unwrap();
        let service = service(temp.path().to_path_buf());
        let key = vault_key();

        let cases = [
            (
                "empty name",
                input_with!(name: String::new()),
                VaultError::InvalidName,
            ),
            (
                "201-character name",
                input_with!(name: "n".repeat(201)),
                VaultError::InvalidName,
            ),
            (
                "empty username",
                input_with!(username: String::new()),
                VaultError::InvalidUsername,
            ),
            (
                "501-character username",
                input_with!(username: "u".repeat(501)),
                VaultError::InvalidUsername,
            ),
            (
                "empty password",
                input_with!(password: String::new()),
                VaultError::InvalidPassword,
            ),
            (
                "4097-character password",
                input_with!(password: "p".repeat(4097)),
                VaultError::InvalidPassword,
            ),
            (
                "2049-character website",
                input_with!(website: Some("w".repeat(2049))),
                VaultError::InvalidWebsite,
            ),
            (
                "empty category",
                input_with!(category: String::new()),
                VaultError::InvalidCategory,
            ),
            (
                "101-character category",
                input_with!(category: "c".repeat(101)),
                VaultError::InvalidCategory,
            ),
            (
                "21 tags",
                input_with!(tags: (0..21).map(|tag| format!("tag-{tag}")).collect()),
                VaultError::InvalidTags,
            ),
            (
                "empty tag",
                input_with!(tags: vec!["   ".to_owned()]),
                VaultError::InvalidTags,
            ),
            (
                "51-character tag",
                input_with!(tags: vec!["t".repeat(51)]),
                VaultError::InvalidTags,
            ),
        ];

        for (description, invalid_input, expected_error) in cases {
            assert_eq!(
                service.create(&key, invalid_input),
                Err(expected_error),
                "{description} must be rejected"
            );
        }
    }

    #[test]
    fn tags_are_trimmed_and_case_insensitively_deduplicated() {
        let temp = tempfile::tempdir().unwrap();
        let service = service(temp.path().to_path_buf());
        let key = vault_key();
        let record = service
            .create(
                &key,
                input_with!(tags: vec![
                    " Work ".to_owned(),
                    "work".to_owned(),
                    "Personal".to_owned(),
                    " PERSONAL ".to_owned(),
                ]),
            )
            .unwrap();

        let loaded = service.get(&key, &record.id).unwrap();

        assert_eq!(loaded.tags, ["Work", "Personal"]);
    }

    #[test]
    fn crud_persists_across_services_and_updates_descending_order() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().to_path_buf();
        let key = vault_key();
        let first_service = service(path.clone());
        let first = first_service.create(&key, input()).unwrap();
        thread::sleep(Duration::from_millis(2));
        let second = first_service
            .create(&key, input_with!(name: "Second account".to_owned()))
            .unwrap();

        let second_service = service(path);
        assert_eq!(
            second_service
                .list(&key)
                .unwrap()
                .into_iter()
                .map(|record| record.id)
                .collect::<Vec<_>>(),
            vec![second.id.clone(), first.id.clone()]
        );

        thread::sleep(Duration::from_millis(2));
        let updated = second_service
            .update(
                &key,
                &first.id,
                input_with!(name: "Updated first account".to_owned()),
            )
            .unwrap();
        assert_eq!(updated.id, first.id);
        assert_eq!(
            second_service.get(&key, &first.id).unwrap().created_at_ms,
            first.created_at_ms
        );
        assert_eq!(
            second_service.list(&key).unwrap()[0].id,
            first.id,
            "an update must move a record to the front"
        );

        second_service.delete(&key, &second.id).unwrap();
        assert!(matches!(
            second_service.get(&key, &second.id),
            Err(VaultError::NotFound)
        ));
    }

    #[test]
    fn missing_records_have_a_stable_not_found_error() {
        let temp = tempfile::tempdir().unwrap();
        let service = service(temp.path().to_path_buf());
        let key = vault_key();
        let missing_id = "00000000000000000000000000000000";

        assert_eq!(service.get(&key, missing_id), Err(VaultError::NotFound));
        assert_eq!(
            service.update(&key, missing_id, input()),
            Err(VaultError::NotFound)
        );
        assert_eq!(service.delete(&key, missing_id), Err(VaultError::NotFound));
        assert_eq!(
            service.password_for_copy(&key, missing_id),
            Err(VaultError::NotFound)
        );
    }

    #[test]
    fn database_bytes_never_contain_literal_credential_fields() {
        let temp = tempfile::tempdir().unwrap();
        let service = service(temp.path().to_path_buf());
        let key = vault_key();
        let record = service
            .create(
                &key,
                VaultRecordInput {
                    name: "literal vault name".to_owned(),
                    username: "literal-vault-username".to_owned(),
                    password: "literal-vault-password".to_owned(),
                    website: Some("literal-vault-website".to_owned()),
                    category: "literal-vault-category".to_owned(),
                    tags: vec!["literal-vault-tag".to_owned()],
                },
            )
            .unwrap();
        let bytes = fs::read(temp.path().join("vault.enc")).unwrap();

        assert_eq!(record.name, "literal vault name");
        for literal in [
            "literal vault name",
            "literal-vault-username",
            "literal-vault-password",
            "literal-vault-website",
            "literal-vault-category",
            "literal-vault-tag",
        ] {
            assert!(
                !bytes
                    .windows(literal.len())
                    .any(|window| window == literal.as_bytes()),
                "database leaked {literal}"
            );
        }
    }

    #[test]
    fn equivalent_records_and_updates_use_fresh_encryption_material() {
        let temp = tempfile::tempdir().unwrap();
        let service = service(temp.path().to_path_buf());
        let key = vault_key();
        let first = service.create(&key, input()).unwrap();
        let second = service.create(&key, input()).unwrap();
        let first_shell = stored_shell(temp.path(), &first.id);
        let second_shell = stored_shell(temp.path(), &second.id);
        assert_ne!(first_shell.0, second_shell.0);
        assert_ne!(first_shell.1, second_shell.1);

        let before_update = first_shell.0;
        service.update(&key, &first.id, input()).unwrap();
        let after_update = stored_shell(temp.path(), &first.id).0;
        assert_ne!(before_update, after_update);
    }

    #[test]
    fn tampered_ciphertext_or_nonce_is_reported_as_damaged_data() {
        let temp = tempfile::tempdir().unwrap();
        let service = service(temp.path().to_path_buf());
        let key = vault_key();
        let ciphertext_record = service.create(&key, input()).unwrap();
        let nonce_record = service
            .create(&key, input_with!(name: "A second account".to_owned()))
            .unwrap();

        let connection = rusqlite::Connection::open(temp.path().join("vault.enc")).unwrap();
        let (_, mut ciphertext) = stored_shell(temp.path(), &ciphertext_record.id);
        ciphertext[0] ^= 1;
        connection
            .execute(
                "UPDATE vault_records SET ciphertext = ?1 WHERE id = ?2",
                rusqlite::params![ciphertext, ciphertext_record.id],
            )
            .unwrap();
        let (mut nonce, _) = stored_shell(temp.path(), &nonce_record.id);
        nonce[0] ^= 1;
        connection
            .execute(
                "UPDATE vault_records SET nonce = ?1 WHERE id = ?2",
                rusqlite::params![nonce, nonce_record.id],
            )
            .unwrap();
        drop(connection);

        assert_eq!(
            service.get(&key, &ciphertext_record.id),
            Err(VaultError::DataDamaged)
        );
        assert_eq!(
            service.get(&key, &nonce_record.id),
            Err(VaultError::DataDamaged)
        );
    }

    #[test]
    fn non_sqlite_vault_file_is_reported_as_damaged_data() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("vault.enc"), vec![b'X'; 4_096]).unwrap();

        assert_eq!(
            service(temp.path().to_path_buf()).list(&vault_key()),
            Err(VaultError::DataDamaged)
        );
    }

    #[test]
    fn invalid_stored_sqlite_column_types_are_reported_as_damaged_data() {
        let temp = tempfile::tempdir().unwrap();
        let service = service(temp.path().to_path_buf());
        let record = service.create(&vault_key(), input()).unwrap();
        let connection = rusqlite::Connection::open(temp.path().join("vault.enc")).unwrap();
        connection
            .execute(
                "UPDATE vault_records SET format_version = 'not-an-integer' WHERE id = ?1",
                rusqlite::params![record.id],
            )
            .unwrap();
        drop(connection);

        assert_eq!(service.list(&vault_key()), Err(VaultError::DataDamaged));
    }

    #[test]
    fn initialization_filesystem_failures_are_reported_as_storage_errors() {
        let temp = tempfile::tempdir().unwrap();
        let blocked_app_data = temp.path().join("not-a-directory");
        fs::write(&blocked_app_data, b"regular file").unwrap();

        assert_eq!(
            service(blocked_app_data).list(&vault_key()),
            Err(VaultError::StorageUnavailable)
        );
    }

    #[test]
    fn summaries_omit_passwords_and_copy_returns_only_the_selected_password() {
        let temp = tempfile::tempdir().unwrap();
        let service = service(temp.path().to_path_buf());
        let key = vault_key();
        let first = service.create(&key, input()).unwrap();
        let second = service
            .create(
                &key,
                input_with!(password: "other selected password".to_owned()),
            )
            .unwrap();

        let summary = serde_json::to_value(service.list(&key).unwrap()).unwrap();
        assert_eq!(summary[0].get("password"), None);
        assert_eq!(summary[1].get("password"), None);
        assert_eq!(
            service
                .password_for_copy(&key, &second.id)
                .unwrap()
                .as_str(),
            "other selected password"
        );
        assert_ne!(
            service.password_for_copy(&key, &first.id).unwrap().as_str(),
            "other selected password"
        );
    }

    #[test]
    fn password_whitespace_round_trips_and_copies_exactly() {
        let temp = tempfile::tempdir().unwrap();
        let service = service(temp.path().to_path_buf());
        let key = vault_key();
        let password = "  preserve these password spaces  ";
        let record = service
            .create(&key, input_with!(password: password.to_owned()))
            .unwrap();

        assert_eq!(service.get(&key, &record.id).unwrap().password, password);
        assert_eq!(
            service
                .password_for_copy(&key, &record.id)
                .unwrap()
                .as_str(),
            password
        );
    }

    #[test]
    fn update_authenticates_the_existing_record_before_writing() {
        let temp = tempfile::tempdir().unwrap();
        let service = service(temp.path().to_path_buf());
        let key = vault_key();
        let record = service.create(&key, input()).unwrap();
        let original_shell = stored_shell(temp.path(), &record.id);

        assert_eq!(
            service.update(&[8; 32], &record.id, input()),
            Err(VaultError::DataDamaged)
        );
        assert_eq!(stored_shell(temp.path(), &record.id), original_shell);
    }

    #[test]
    fn tampered_records_cannot_be_overwritten_by_update() {
        let temp = tempfile::tempdir().unwrap();
        let service = service(temp.path().to_path_buf());
        let key = vault_key();
        let record = service.create(&key, input()).unwrap();
        let connection = rusqlite::Connection::open(temp.path().join("vault.enc")).unwrap();
        let (_, mut ciphertext) = stored_shell(temp.path(), &record.id);
        ciphertext[0] ^= 1;
        connection
            .execute(
                "UPDATE vault_records SET ciphertext = ?1 WHERE id = ?2",
                rusqlite::params![ciphertext, record.id],
            )
            .unwrap();

        assert_eq!(
            service.update(&key, &record.id, input()),
            Err(VaultError::DataDamaged)
        );
    }

    #[test]
    fn malformed_or_unsupported_v1_schema_is_rejected_before_queries() {
        let temp = tempfile::tempdir().unwrap();
        let connection = rusqlite::Connection::open(temp.path().join("vault.enc")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE vault_records (
                    id TEXT PRIMARY KEY NOT NULL,
                    format_version INTEGER NOT NULL,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE INDEX vault_records_updated_at_idx
                    ON vault_records(updated_at_ms DESC, id ASC);
                CREATE TABLE unexpected_application_table (value TEXT);
                PRAGMA user_version = 1;",
            )
            .unwrap();
        drop(connection);
        let malformed_service = service(temp.path().to_path_buf());

        assert_eq!(
            malformed_service.list(&vault_key()),
            Err(VaultError::DataDamaged)
        );

        let unsupported = tempfile::tempdir().unwrap();
        let connection = rusqlite::Connection::open(unsupported.path().join("vault.enc")).unwrap();
        connection
            .execute_batch("PRAGMA user_version = 2;")
            .unwrap();
        drop(connection);
        assert_eq!(
            service(unsupported.path().to_path_buf()).list(&vault_key()),
            Err(VaultError::DataDamaged)
        );

        let malformed_columns = tempfile::tempdir().unwrap();
        let connection =
            rusqlite::Connection::open(malformed_columns.path().join("vault.enc")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE vault_records (
                    id TEXT PRIMARY KEY NOT NULL,
                    format_version INTEGER NOT NULL,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER
                );
                CREATE INDEX vault_records_updated_at_idx
                    ON vault_records(updated_at_ms DESC, id ASC);
                PRAGMA user_version = 1;",
            )
            .unwrap();
        drop(connection);
        assert_eq!(
            service(malformed_columns.path().to_path_buf()).list(&vault_key()),
            Err(VaultError::DataDamaged)
        );

        let malformed_index = tempfile::tempdir().unwrap();
        let index_service = service(malformed_index.path().to_path_buf());
        index_service.create(&vault_key(), input()).unwrap();
        let connection =
            rusqlite::Connection::open(malformed_index.path().join("vault.enc")).unwrap();
        connection
            .execute_batch(
                "DROP INDEX vault_records_updated_at_idx;
                 CREATE INDEX vault_records_updated_at_idx
                    ON vault_records(updated_at_ms ASC, id ASC);",
            )
            .unwrap();
        drop(connection);
        assert_eq!(
            index_service.list(&vault_key()),
            Err(VaultError::DataDamaged)
        );
    }

    #[test]
    fn schema_validation_rejects_reserved_prefix_generated_column_and_index_metadata_bypasses() {
        let reserved_prefix = tempfile::tempdir().unwrap();
        let connection =
            rusqlite::Connection::open(reserved_prefix.path().join("vault.enc")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE vault_records (
                    id TEXT PRIMARY KEY NOT NULL,
                    format_version INTEGER NOT NULL,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE INDEX vault_records_updated_at_idx
                    ON vault_records(updated_at_ms DESC, id ASC);
                CREATE TABLE sqliteX (value TEXT);
                PRAGMA user_version = 1;",
            )
            .unwrap();
        drop(connection);
        assert_eq!(
            service(reserved_prefix.path().to_path_buf()).list(&vault_key()),
            Err(VaultError::DataDamaged)
        );

        let generated_column = tempfile::tempdir().unwrap();
        let connection =
            rusqlite::Connection::open(generated_column.path().join("vault.enc")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE vault_records (
                    id TEXT PRIMARY KEY NOT NULL,
                    format_version INTEGER NOT NULL,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL,
                    generated_extra INTEGER GENERATED ALWAYS AS (length(id)) VIRTUAL
                );
                CREATE INDEX vault_records_updated_at_idx
                    ON vault_records(updated_at_ms DESC, id ASC);
                PRAGMA user_version = 1;",
            )
            .unwrap();
        drop(connection);
        assert_eq!(
            service(generated_column.path().to_path_buf()).list(&vault_key()),
            Err(VaultError::DataDamaged)
        );

        let altered_index = tempfile::tempdir().unwrap();
        let connection =
            rusqlite::Connection::open(altered_index.path().join("vault.enc")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE vault_records (
                    id TEXT PRIMARY KEY NOT NULL,
                    format_version INTEGER NOT NULL,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE INDEX vault_records_updated_at_idx
                    ON vault_records(updated_at_ms COLLATE NOCASE DESC, id COLLATE NOCASE ASC)
                    WHERE updated_at_ms > 0;
                PRAGMA user_version = 1;",
            )
            .unwrap();
        drop(connection);
        assert_eq!(
            service(altered_index.path().to_path_buf()).list(&vault_key()),
            Err(VaultError::DataDamaged)
        );
    }

    #[test]
    fn exact_schema_fingerprint_rejects_constraint_default_and_table_option_bypasses() {
        let table_shapes = [
            (
                "default expression",
                "CREATE TABLE vault_records (
                    id TEXT PRIMARY KEY NOT NULL,
                    format_version INTEGER NOT NULL DEFAULT 1,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                )",
            ),
            (
                "additional implicit unique index",
                "CREATE TABLE vault_records (
                    id TEXT PRIMARY KEY NOT NULL,
                    format_version INTEGER NOT NULL,
                    nonce BLOB NOT NULL UNIQUE,
                    ciphertext BLOB NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                )",
            ),
            (
                "check constraint",
                "CREATE TABLE vault_records (
                    id TEXT PRIMARY KEY NOT NULL,
                    format_version INTEGER NOT NULL,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL CHECK(length(ciphertext) > 0),
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                )",
            ),
            (
                "column collation",
                "CREATE TABLE vault_records (
                    id TEXT COLLATE NOCASE PRIMARY KEY NOT NULL,
                    format_version INTEGER NOT NULL,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                )",
            ),
            (
                "conflict policy",
                "CREATE TABLE vault_records (
                    id TEXT PRIMARY KEY ON CONFLICT REPLACE NOT NULL,
                    format_version INTEGER NOT NULL,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                )",
            ),
            (
                "strict table",
                "CREATE TABLE vault_records (
                    id TEXT PRIMARY KEY NOT NULL,
                    format_version INTEGER NOT NULL,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                ) STRICT",
            ),
            (
                "without-rowid table",
                "CREATE TABLE vault_records (
                    id TEXT PRIMARY KEY NOT NULL,
                    format_version INTEGER NOT NULL,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                ) WITHOUT ROWID",
            ),
        ];

        for (description, table_sql) in table_shapes {
            let temp = tempfile::tempdir().unwrap();
            let connection = rusqlite::Connection::open(temp.path().join("vault.enc")).unwrap();
            connection.execute_batch(table_sql).unwrap();
            connection
                .execute_batch(
                    "CREATE INDEX vault_records_updated_at_idx
                        ON vault_records(updated_at_ms DESC, id ASC);
                     PRAGMA user_version = 1;",
                )
                .unwrap();
            drop(connection);

            assert_eq!(
                service(temp.path().to_path_buf()).list(&vault_key()),
                Err(VaultError::DataDamaged),
                "{description} must not match the version-1 schema"
            );
        }
    }

    #[test]
    fn update_timestamp_is_strictly_newer_than_the_existing_record() {
        let temp = tempfile::tempdir().unwrap();
        let service = service(temp.path().to_path_buf());
        let key = vault_key();
        let record = service.create(&key, input()).unwrap();
        let future = 4_000_000_000_000_i64;
        let connection = rusqlite::Connection::open(temp.path().join("vault.enc")).unwrap();
        connection
            .execute(
                "UPDATE vault_records SET updated_at_ms = ?1 WHERE id = ?2",
                rusqlite::params![future, record.id],
            )
            .unwrap();
        drop(connection);

        assert_eq!(
            service
                .update(&key, &record.id, input())
                .unwrap()
                .updated_at_ms,
            future + 1
        );
    }

    #[test]
    fn invalid_encryption_metadata_and_rebound_aad_are_damaged() {
        let temp = tempfile::tempdir().unwrap();
        let first_service = service(temp.path().to_path_buf());
        let key = vault_key();
        let first = first_service.create(&key, input()).unwrap();
        let second = first_service
            .create(&key, input_with!(name: "Second account".to_owned()))
            .unwrap();
        let connection = rusqlite::Connection::open(temp.path().join("vault.enc")).unwrap();
        let second_shell = stored_shell(temp.path(), &second.id);
        connection
            .execute(
                "UPDATE vault_records SET nonce = ?1, ciphertext = ?2 WHERE id = ?3",
                rusqlite::params![second_shell.0, second_shell.1, first.id],
            )
            .unwrap();
        drop(connection);

        assert_eq!(
            first_service.get(&key, &first.id),
            Err(VaultError::DataDamaged)
        );

        let metadata = tempfile::tempdir().unwrap();
        let metadata_service = service(metadata.path().to_path_buf());
        let record = metadata_service.create(&key, input()).unwrap();
        let connection = rusqlite::Connection::open(metadata.path().join("vault.enc")).unwrap();
        connection
            .execute(
                "UPDATE vault_records SET format_version = 2 WHERE id = ?1",
                rusqlite::params![record.id],
            )
            .unwrap();
        drop(connection);
        assert_eq!(
            metadata_service.get(&key, &record.id),
            Err(VaultError::DataDamaged)
        );

        let nonce_record = metadata_service.create(&key, input()).unwrap();
        let connection = rusqlite::Connection::open(metadata.path().join("vault.enc")).unwrap();
        connection
            .execute(
                "UPDATE vault_records SET nonce = ?1 WHERE id = ?2",
                rusqlite::params![vec![0_u8; 23], nonce_record.id],
            )
            .unwrap();
        drop(connection);
        assert_eq!(
            metadata_service.get(&key, &nonce_record.id),
            Err(VaultError::DataDamaged)
        );
    }

    #[test]
    fn entropy_failure_prevents_record_creation() {
        let temp = tempfile::tempdir().unwrap();

        assert_eq!(
            unavailable_entropy_service(temp.path().to_path_buf()).create(&vault_key(), input()),
            Err(VaultError::EntropyUnavailable)
        );
    }
}
