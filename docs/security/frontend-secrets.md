# Phase 7: frontend secret boundaries

Vault lists, creation/update results and the new `get_vault_record_summary` detail endpoint contain metadata only. Opening a credential no longer calls the full-record endpoint. Rust may decrypt an entire encrypted record internally to build the summary; the full DTO wipes on drop and no decrypted vault cache is kept.

`copy_vault_password` accepts only a record ID, verifies unlocked state under the shared operation gate, decrypts the selected password and writes it directly to the Rust-managed clipboard. It returns no password. No `navigator.clipboard` path is introduced.

Explicit Reveal uses the existing selected-record endpoint, retains only the password in dialog-local state, and discards it on Hide/Close/Delete/Edit/record change. Explicit Edit is a justified plaintext exception: the existing editor needs the selected fields to edit them. Its DTO is discarded on cancel/close. The authenticated subtree unmounts on lock/navigation, and request generations discard late responses. Dialogs are keyed by record ID to prevent state reuse across records.

One-time Recovery Key display and typed master/credential passwords remain unavoidable temporary UI inputs/outputs; they are not persisted or placed in global stores. Settings/Home receive no credential plaintext. Existing IPC commands still enforce Rust authorization; frontend call patterns are not an XSS security boundary. JavaScript immutable strings cannot promise physical memory zeroization.

Tests cover password-free serialized Rust detail/list contracts, locked authorization, copy-by-ID, reveal/hide, edit/cancel, close, stale response rejection and AuthGate lock teardown. Hook-port tests do not claim actual WebView memory erasure or visual rendering verification.
