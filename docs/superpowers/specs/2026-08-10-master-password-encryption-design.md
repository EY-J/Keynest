# KeyNest Master Password and Encryption Design

**Date:** 2026-08-10
**Status:** Approved design awaiting written-spec review

## Objective

Require a master password before the existing KeyNest interface becomes available and establish the encryption boundary that all future vault data will use. The master password must protect data cryptographically, not merely hide the React interface.

## Scope

This feature includes first-run password creation, unlock on every application launch, manual locking, destructive reset when the password is forgotten, encrypted key metadata stored in the operating system's application-data directory, and automated coverage for the Rust and React behavior.

This feature does not yet implement password-vault records, password recovery, password changes, inactivity locking, lock-on-minimize, cloud synchronization, biometrics, or Windows Credential Manager integration.

## Confirmed Product Decisions

- KeyNest uses real local encryption owned by the Tauri Rust backend.
- A forgotten master password cannot be recovered.
- Resetting KeyNest permanently deletes its encrypted local contents.
- KeyNest locks on every application launch and when the user selects **Lock KeyNest**.
- Inactivity and focus-based locking are deferred.
- The minimum master-password length is 12 characters. No composition rules are imposed.
- The protected home and vault interface is not rendered until Rust reports an unlocked state.

## Threat Model

The design protects KeyNest data at rest when another person opens the application, browses the local filesystem, or copies the encrypted files. It also prevents a renderer-only bypass from accessing protected backend commands because Rust independently checks the lock state.

The design does not protect against malware, keyloggers, screen capture, an administrator inspecting process memory while KeyNest is unlocked, or an attacker who can modify the application binary. An attacker who copies the encrypted profile can guess passwords offline, so security primarily depends on a strong master password and a deliberately expensive password-derivation function. UI attempt delays are supplementary and are not represented as protection against offline attacks.

## Architecture

### Rust security boundary

The Tauri Rust backend owns:

- Master-password validation.
- Cryptographic key generation and derivation.
- Encryption and decryption of the vault data-encryption key.
- Authentication state and failed-attempt timing.
- Application-data path resolution and atomic file replacement.
- Resetting encrypted local state.
- Authorization checks for all current and future vault commands.

React owns only screen rendering, transient form state, and calls to the narrow authentication command interface. The plaintext data-encryption key never crosses the Tauri IPC boundary.

### Cryptographic construction

On first-time setup, Rust performs the following sequence:

1. Validate that the UTF-8 master password contains at least 12 characters.
2. Generate a unique random 128-bit salt using the operating system's cryptographically secure random source.
3. Derive a 256-bit key-encryption key from the master password with Argon2id using 64 MiB of memory, three iterations, and four lanes. These are the second recommended parameters in [RFC 9106](https://www.rfc-editor.org/rfc/rfc9106.html) for memory-constrained environments.
4. Generate an independent random 256-bit vault data-encryption key.
5. Generate a unique random 192-bit nonce and encrypt the vault key with XChaCha20-Poly1305, using the fixed versioned KeyNest context string `keynest-profile-v1` as authenticated associated data. The selected RustCrypto construction uses a 256-bit key and 192-bit nonce as documented by the [RustCrypto XChaCha20-Poly1305 API](https://docs.rs/chacha20poly1305/latest/chacha20poly1305/type.XChaCha20Poly1305.html).
6. Persist the version, Argon2id algorithm and parameters, encoded salt, wrapping algorithm, encoded nonce, and wrapped vault-key ciphertext.
7. Retain the plaintext vault key only in zeroizing Rust memory and enter the unlocked state.

The persisted Argon2id parameters are used for subsequent unlocks so the format can be upgraded later. Version 1 requires a 64 MiB memory cost, three iterations, four lanes, a 128-bit salt, and a 256-bit output. Tests may inject cheaper parameters but production code must reject unsupported or weaker version-1 parameters rather than silently downgrading them.

On unlock, Rust derives the key-encryption key from the submitted password and stored parameters, then authenticates and decrypts the wrapped vault key. Successful authenticated decryption is the password check; no separate reusable password hash or verifier is stored. Incorrect passwords and authentication failures return the same public error.

The submitted password, derived key-encryption key, and vault key are zeroized in Rust when no longer needed. React clears password fields immediately after each completed request and never persists them in browser storage. Because the password is entered in a webview, it necessarily exists transiently in renderer memory during submission; this limitation is included in the threat model.

### Persisted profile format

The profile metadata is a versioned structured document with these logical fields:

```text
format_version
kdf.algorithm
kdf.salt
kdf.memory_kib
kdf.iterations
kdf.parallelism
key_wrap.algorithm
key_wrap.nonce
key_wrap.ciphertext
```

Binary values are encoded for structured storage. The document contains neither the plaintext master password, a plaintext vault key, nor future plaintext vault records. Future encrypted vault data uses the unwrapped data-encryption key and a separate nonce; it must not derive record-encryption keys directly from the master password.

The profile is stored under Tauri's OS-managed per-user application-data directory. Creation and replacement use a file in the same directory followed by an atomic rename/replace operation. A partial or invalid profile is treated as damaged data and is never silently overwritten.

### In-memory authentication state

Rust maintains one process-wide state machine:

```text
SetupRequired | Locked | Unlocked(vault_key) | DataError
```

Only `Unlocked` contains the zeroizing vault key. Launching the application with an existing valid profile begins in `Locked`. Closing the process discards all in-memory keys. Calling the lock command drops the key immediately and returns to `Locked`.

Every current or future command that handles protected data must check Rust's state and return an unauthorized error unless it is `Unlocked`. React gating alone is never considered authorization.

## Backend Command Contract

The frontend uses a small command surface:

- `get_auth_status()` returns `setup-required`, `locked`, `unlocked`, or `data-error`.
- `create_master_password(password)` validates policy, creates the encrypted profile, and unlocks the process. It fails if a valid profile already exists.
- `unlock(password)` unlocks a valid profile or returns a generic invalid-password response. It never reveals whether a failure came from a guessed password or authenticated-decryption failure.
- `lock()` discards the in-memory vault key. Calling it while already locked is safe and idempotent.
- `reset_keynest(confirmation)` accepts only the exact confirmation string `RESET`, removes KeyNest's encrypted local profile and vault files, clears all in-memory keys, and returns to `SetupRequired`.

Internal errors distinguish invalid input, invalid credentials, throttling, damaged data, unauthorized access, and I/O failure. The IPC response exposes only messages that are useful and safe for the corresponding screen.

## Attempt Handling

Incorrect unlock attempts are counted only in process memory. After three consecutive failures, new attempts receive an increasing delay, capped at five seconds. A successful unlock clears the count. Restarting the application resets the delay, which is acceptable because the delay is only a usability-safe deterrent against casual rapid guessing; Argon2id and password strength are the actual defenses against offline guessing.

## User Experience

### Startup states

`App` resolves the Rust authentication status before rendering protected content and shows one of five states:

- **Checking:** a brief branded loading state while the backend inspects local metadata.
- **Setup required:** first-run master-password creation.
- **Locked:** the returning-user unlock screen.
- **Unlocked:** the existing home interface.
- **Data error:** a damaged-data screen with retry and destructive reset actions.

The current custom title bar remains available on authentication screens so the application can still be moved, minimized, maximized, and closed. Navigation, home content, and future vault content are absent from the React tree until unlock succeeds.

### First-time setup

The setup screen contains:

- Master password and confirmation fields.
- Show/hide controls for both fields.
- Clear inline feedback for the 12-character minimum and password mismatch.
- A visible warning that KeyNest cannot recover a forgotten password.
- A primary **Create Master Password** action.

The frontend provides immediate validation, while Rust independently enforces the minimum length. Confirmation is checked in React; only the validated password is sent to Rust. Submitting successfully clears both fields and renders the existing home screen.

### Unlock

The unlock screen contains one master-password field, a show/hide control, and a primary **Unlock KeyNest** action. Pressing Enter submits the form. Incorrect passwords clear the field, retain focus, and show a generic error. During a throttling delay, submission is disabled and the user receives a brief wait message.

### Manual lock

The existing sidebar **Lock KeyNest** button receives a real callback. Activating it closes the sidebar, calls Rust's lock command, removes protected content from the React tree, and renders the locked screen. No confirmation is required because locking is non-destructive.

### Forgotten password and reset

The unlock and damaged-data screens expose **Forgot password? Reset KeyNest**. The reset dialog:

- States that encrypted vault contents cannot be recovered.
- Requires the user to type the exact word `RESET`.
- Keeps the destructive action disabled until the text matches.
- Calls the backend confirmation-enforcing reset command.
- Returns to first-time setup after success.

Reset removes only KeyNest-owned encrypted profile and vault files from its application-data location. It does not delete unrelated application-data contents.

## Error Handling

- Incorrect passwords never disclose cryptographic details.
- Damaged or tampered profile metadata never falls back to first-time setup automatically.
- Authentication and reset controls prevent duplicate submissions while a request is pending.
- I/O failures preserve existing data where possible and show a retryable message.
- A setup failure leaves the app in `SetupRequired` unless a complete valid profile was atomically written.
- A reset failure remains on the confirmation/error state and reports that local data could not be removed.
- Unexpected backend responses fail closed: protected content remains unrendered.

## Code Organization

Rust security code will be split by responsibility rather than placed in the current template `lib.rs`:

- An authentication module for the state machine and command-facing service.
- A crypto module for key derivation, wrapping, and zeroization.
- A storage module for profile parsing, validation, app-data paths, and atomic writes.
- Small serializable command response and error types shared by the commands.

React code will separate the startup state coordinator from presentational setup, unlock, reset, and damaged-data components. A frontend authentication client will contain the Tauri `invoke` calls so components can be tested without embedding IPC details throughout the UI.

## Verification Strategy

Implementation follows test-driven development.

### Rust tests

- A new profile can be created and produces an unlocked state.
- The correct password unlocks an existing profile.
- An incorrect password does not unlock or expose the vault key.
- Locking drops the unlocked state and is idempotent.
- Reset requires exact confirmation, removes only scoped KeyNest files, and returns to setup.
- Tampering with salt, parameters, nonce, or ciphertext fails closed.
- Missing, malformed, truncated, and unsupported-version metadata produce `DataError`, not `SetupRequired`.
- Persisted bytes contain neither the master password nor the plaintext vault key.
- Atomic-write failure does not replace a previously valid profile.
- Production KDF parameters cannot be downgraded through untrusted profile values below the accepted security floor.

Cryptographic tests use deterministic random input and reduced Argon2id cost only through test-only dependency injection. Production entropy always comes from the operating system.

### React tests

A React test runner and DOM testing utilities will be added because the project currently has no frontend test framework. Tests cover:

- Checking, setup, locked, unlocked, and data-error rendering.
- Setup length and confirmation validation.
- Successful creation and unlock transitions.
- Incorrect-password clearing, focus, and feedback.
- Manual locking removes protected content.
- Reset remains disabled until `RESET` is entered and returns to setup after success.
- Backend and unexpected errors fail closed.

### Build and manual verification

- Run the complete frontend test suite.
- Run the complete Rust test suite.
- Run the TypeScript and Vite production build.
- Run a Tauri development or packaged-app smoke test covering first launch, application restart, wrong password, correct password, manual lock, and destructive reset.

## Acceptance Criteria

- A fresh installation cannot show the home screen until a valid master password has been created.
- Every later process launch begins locked and cannot show protected content until the correct password is entered.
- The persisted profile contains no plaintext password or vault key.
- Incorrect passwords, tampered metadata, and malformed files fail closed.
- The sidebar lock action discards the backend key and returns to the unlock screen.
- A forgotten password has no recovery path; confirmed reset deletes only KeyNest's encrypted local data and returns to setup.
- Frontend gating and backend authorization are both enforced.
- Automated Rust and React tests cover the security and state transitions, and both production builds complete successfully.
