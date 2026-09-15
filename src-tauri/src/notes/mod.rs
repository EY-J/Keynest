mod crypto;
mod model;
mod service;

pub(crate) use model::{NoteInput, NoteRecord, NotesError};
pub(crate) use service::NotesService;

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::security::{CryptoError, EntropySource};

    use super::{NoteInput, NotesService};

    struct DeterministicEntropy(Mutex<u8>);

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

    fn input() -> NoteInput {
        NoteInput {
            title: "Project ideas".into(),
            content: "A private note\nwith line breaks.".into(),
            tags: vec!["Ideas".into()],
            favorite: false,
        }
    }

    #[test]
    fn notes_round_trip_through_encrypted_local_storage() {
        let temp = tempfile::tempdir().unwrap();
        let service = NotesService::new(
            temp.path().to_path_buf(),
            Arc::new(DeterministicEntropy(Mutex::new(1))),
        );
        let key = [7_u8; 32];
        let created = service.create(&key, input()).unwrap();
        assert_eq!(service.list(&key).unwrap(), vec![created.clone()]);

        let mut changed = input();
        changed.content = "Updated".into();
        changed.favorite = true;
        let updated = service.update(&key, &created.id, changed).unwrap();
        assert!(updated.favorite);
        assert_eq!(updated.content, "Updated");
        assert!(updated.updated_at_ms > created.updated_at_ms);

        service.delete(&key, &created.id).unwrap();
        assert!(service.list(&key).unwrap().is_empty());
        let bytes = std::fs::read(temp.path().join("notes.enc")).unwrap();
        assert!(!bytes
            .windows("Project ideas".len())
            .any(|window| window == b"Project ideas"));
        assert!(!bytes
            .windows("Updated".len())
            .any(|window| window == b"Updated"));

        let deleted = service.list_deleted(&key).unwrap();
        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0].title, "Project ideas");
        assert_eq!(
            deleted[0].item_type,
            crate::recently_deleted::DeletedItemType::Note
        );

        service.restore(&key, &created.id).unwrap();
        assert_eq!(service.list(&key).unwrap().len(), 1);
        assert!(service.list_deleted(&key).unwrap().is_empty());

        service.delete(&key, &created.id).unwrap();
        service.purge_deleted_before(&key, i64::MAX).unwrap();
        assert!(service.list_deleted(&key).unwrap().is_empty());
        assert!(service.restore(&key, &created.id).is_err());
    }

    #[test]
    fn version_one_notes_schema_migrates_to_soft_delete_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("notes.enc");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE notes (
                    id TEXT PRIMARY KEY NOT NULL,
                    format_version INTEGER NOT NULL,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE INDEX notes_updated_at_idx ON notes(updated_at_ms DESC, id ASC);
                PRAGMA user_version = 1;",
            )
            .unwrap();
        drop(connection);

        let service = NotesService::new(
            temp.path().to_path_buf(),
            Arc::new(DeterministicEntropy(Mutex::new(1))),
        );
        assert!(service.list(&[7_u8; 32]).unwrap().is_empty());
        let connection = rusqlite::Connection::open(path).unwrap();
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('notes') WHERE name = 'deleted_at_ms'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );
    }
}
