# Phase 6: secret lifetime audit

- Master-password/recovery inputs are wrapped in `Zeroizing<String>` as soon as the Rust command body runs, before dispatch to the blocking worker. Explicit clearing remains at completion; RAII also covers errors, dropped queued closures and unwinding.
- `RecoveryKeyResult` now wipes its plaintext field on drop and redacts Debug. Only the explicit one-time display IPC serializes it; its JSON contract is unchanged.
- VaultKey has no Debug/Clone and is zeroizing on drop. `AuthService::lock` replaces unlocked state with the encrypted profile only; no decrypted record cache exists in VaultService. The shared operation gate serializes active secret operations against locking.
- Password and recovery wrapping keys and decrypted key buffers use Zeroizing. Unwrapping now copies directly into a guarded key instead of an ordinary stack array.
- Argon2's zeroize feature is enabled. Its allocating convenience method uses an ordinary Vec, so KeyNest now supplies its own `Zeroizing<Vec<Block>>` working memory to `hash_password_into_with_memory`. Cost bounds and outputs are unchanged.
- Credential payload/input/detail structs already wipe on drop and redact/omit Debug. JSON encryption now writes directly into a guarded buffer, including partial serialization failures. Decryption buffers and backend clipboard values are guarded. Error enums contain static descriptions, not plaintext or underlying serialization errors.
- Secure notes and private files are not implemented; no plaintext handling was added for them.

Validation: type/explicit-wipe tests cover VaultKey, Argon2 workspace ownership and recovery response; existing credential DTO, failed crypto/auth, lock and concurrency tests cover authorization/lifetimes. Do not read freed memory to test zeroization: that would be undefined behavior.

Limits: Rust/Tauri IPC parsing/serialization, library-internal temporaries, allocator reallocations, register/stack copies, WebView/JS immutable strings and OS clipboard copies cannot all be reliably wiped here. A lock waits for an already-running protected operation; it does not cancel crypto mid-operation. Clipboard access failure can leave OS-owned text behind. This reduces unnecessary lifetime, not privileged malware access, live memory dumps, paging, or external backups.
