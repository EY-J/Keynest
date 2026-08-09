# KeyNest Master Password Encryption Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Require a real, Rust-backed encrypted master password before KeyNest renders protected content, with first-run setup, launch-time unlock, manual locking, and confirmed destructive reset.

**Architecture:** A focused Rust security package owns Argon2id derivation, XChaCha20-Poly1305 key wrapping, versioned profile storage, lock state, and authorization. Tauri exposes a narrow authentication command surface, while a React authentication gate renders setup, unlock, data-error, and reset states and only mounts the existing home page after Rust reports `unlocked`.

**Tech Stack:** Tauri 2, Rust 1.97, Argon2id, RustCrypto XChaCha20-Poly1305, zeroize, React 19, TypeScript 5.8, Vitest, Testing Library, Vite 7.

## Global Constraints

- Use Argon2id version 1.3 with 65,536 KiB memory, three iterations, four lanes, a 16-byte random salt, and a 32-byte output in production.
- Wrap a random 32-byte vault key with XChaCha20-Poly1305 using a random 24-byte nonce and associated data `keynest-profile-v1`.
- Never persist the master password, derived wrapping key, or plaintext vault key.
- Never send the plaintext vault key across Tauri IPC.
- Store the versioned profile only in Tauri's per-user application-data directory.
- Treat malformed, truncated, tampered, or unsupported metadata as closed/error states; never silently initialize over it.
- Require at least 12 Unicode characters for the master password in both React and Rust.
- Reset must require the exact backend confirmation `RESET` and remove only the two KeyNest-owned files `profile.json` and `vault.enc`.
- Every future protected Rust command must require `Unlocked` state; React gating is not authorization.
- Implement with red-green-refactor cycles and retain fresh failure/pass evidence for each task.

---

## File Structure

### Rust

- `src-tauri/src/security/mod.rs`: package exports and shared public types.
- `src-tauri/src/security/crypto.rs`: fixed production KDF parameters, entropy abstraction, key wrapping/unwrapping, and zeroizing key type.
- `src-tauri/src/security/storage.rs`: versioned profile schema validation, scoped paths, atomic first-write, load, and reset.
- `src-tauri/src/security/auth.rs`: authentication state machine, password policy, attempt timing, and service API.
- `src-tauri/src/lib.rs`: Tauri application setup, managed service initialization, IPC commands, blocking-crypto dispatch, and safe public errors.
- `src-tauri/Cargo.toml`: cryptographic, encoding, error, zeroization, and temporary-file dependencies.

### React

- `src/features/auth/types.ts`: frontend auth status and public error types.
- `src/features/auth/authClient.ts`: the only module that calls Tauri authentication commands.
- `src/features/auth/components/AuthGate.tsx`: startup state coordinator and protected-content render boundary.
- `src/features/auth/components/AuthLayout.tsx`: branded locked/setup shell with the shared title bar.
- `src/features/auth/components/PasswordField.tsx`: accessible password input and show/hide control.
- `src/features/auth/components/SetupScreen.tsx`: first-run password creation and validation.
- `src/features/auth/components/UnlockScreen.tsx`: returning-user unlock form and reset entry point.
- `src/features/auth/components/ResetDialog.tsx`: exact-`RESET` destructive confirmation.
- `src/features/auth/components/DataErrorScreen.tsx`: retry and reset-only damaged-data state.
- `src/shared/components/AppTitleBar.tsx`: title bar extracted from `HomePage` for reuse on protected and authentication screens.
- `src/app/App.tsx`: mounts `AuthGate` and passes its lock callback into protected content.
- `src/pages/HomePage.tsx`: accepts and forwards the lock callback.
- `src/shared/components/NavigationSidebar.tsx`: invokes the real lock callback.
- `src/App.css`: authentication UI and extracted-title-bar styling.
- `src/test/setup.ts`: jest-dom matchers and global DOM cleanup.
- `src/features/auth/components/AuthGate.test.tsx`: security-boundary and state-transition behavior.
- `src/features/auth/components/SetupScreen.test.tsx`: password-policy and confirmation behavior.
- `src/features/auth/components/UnlockScreen.test.tsx`: unlock failure, focus, throttling, and reset behavior.
- `src/app/App.test.tsx`: protected-home rendering and sidebar manual lock integration.
- `vite.config.ts`, `tsconfig.node.json`, `package.json`, `package-lock.json`: test runner configuration and dependencies.

---

### Task 1: Rust Cryptographic Primitive

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/security/mod.rs`
- Create: `src-tauri/src/security/crypto.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: `KdfParams::production()`, `WrappedVaultKey`, `VaultKey`, `wrap_new_vault_key(password, params, entropy)`, and `unwrap_vault_key(password, wrapped)`.
- `WrappedVaultKey` serializes KDF and AEAD metadata but never exposes plaintext keys.
- `VaultKey` wraps `Zeroizing<[u8; 32]>` and offers only crate-private byte access.

- [ ] **Step 1: Add compile-time dependencies without production behavior**

Add these dependency families to `src-tauri/Cargo.toml` and let Cargo lock exact compatible releases:

```toml
argon2 = "0.5"
base64 = "0.22"
chacha20poly1305 = "0.10"
getrandom = "0.3"
thiserror = "2"
zeroize = { version = "1", features = ["zeroize_derive"] }
tempfile = "3"
```

- [ ] **Step 2: Write failing crypto tests**

Create tests in `security/crypto.rs` using a deterministic `EntropySource` that fills the salt, vault key, and nonce with known non-secret test bytes:

```rust
#[test]
fn correct_password_unwraps_the_generated_vault_key() {
    let entropy = FixedEntropy::new();
    let (wrapped, original) = wrap_new_vault_key(
        "a secure master password",
        KdfParams::testing(),
        &entropy,
    ).unwrap();

    let unlocked = unwrap_vault_key("a secure master password", &wrapped).unwrap();
    assert_eq!(unlocked.expose_for_test(), original.expose_for_test());
}

#[test]
fn incorrect_password_cannot_unwrap_the_vault_key() {
    let entropy = FixedEntropy::new();
    let (wrapped, _) = wrap_new_vault_key(
        "a secure master password",
        KdfParams::testing(),
        &entropy,
    ).unwrap();

    assert_eq!(
        unwrap_vault_key("the wrong master password", &wrapped),
        Err(CryptoError::AuthenticationFailed),
    );
}

#[test]
fn tampered_ciphertext_fails_authentication() {
    let entropy = FixedEntropy::new();
    let (mut wrapped, _) = wrap_new_vault_key(
        "a secure master password",
        KdfParams::testing(),
        &entropy,
    ).unwrap();
    wrapped.ciphertext[0] ^= 1;

    assert_eq!(
        unwrap_vault_key("a secure master password", &wrapped),
        Err(CryptoError::AuthenticationFailed),
    );
}
```

- [ ] **Step 3: Run the focused test and verify RED**

Run: `cargo test security::crypto::tests --manifest-path src-tauri/Cargo.toml`

Expected: compilation fails because the requested crypto types and functions do not exist yet.

- [ ] **Step 4: Implement the minimal cryptographic module**

Implement:

```rust
pub(crate) const PROFILE_AAD: &[u8] = b"keynest-profile-v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct KdfParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl KdfParams {
    pub(crate) const fn production() -> Self {
        Self { memory_kib: 65_536, iterations: 3, parallelism: 4 }
    }

    #[cfg(test)]
    pub(crate) const fn testing() -> Self {
        Self { memory_kib: 32, iterations: 1, parallelism: 1 }
    }
}

#[derive(Zeroize, ZeroizeOnDrop)]
pub(crate) struct VaultKey([u8; 32]);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct WrappedVaultKey {
    pub params: KdfParams,
    pub salt: Vec<u8>,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}
```

Use `Argon2::new(Algorithm::Argon2id, Version::V0x13, ...)` with `hash_password_into`, `XChaCha20Poly1305`, and `Payload { msg, aad: PROFILE_AAD }`. Map all AEAD authentication failures to `CryptoError::AuthenticationFailed`. Zeroize the derived wrapping-key array on all paths.

- [ ] **Step 5: Run focused tests and verify GREEN**

Run: `cargo test security::crypto::tests --manifest-path src-tauri/Cargo.toml`

Expected: all crypto tests pass.

- [ ] **Step 6: Commit the independently tested primitive**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/security src-tauri/src/lib.rs
git commit -m "feat: add vault key encryption primitive"
```

---

### Task 2: Versioned Profile Storage

**Files:**
- Create: `src-tauri/src/security/storage.rs`
- Modify: `src-tauri/src/security/mod.rs`

**Interfaces:**
- Consumes: `KdfParams`, `WrappedVaultKey` from Task 1.
- Produces: `StoredProfile::new(wrapped)`, `ProfileStore::new(app_data_dir, accepted_kdf)`, `load()`, `create(profile)`, and `reset()`.
- `load()` returns `Missing`, `Valid(StoredProfile)`, or a typed `StorageError`; malformed existing data never maps to `Missing`.
- Production constructs the store with `KdfParams::production()`; tests explicitly construct it with `KdfParams::testing()` so low-cost test profiles do not weaken production validation.

- [ ] **Step 1: Write failing storage tests**

Create real-filesystem tests with `tempfile::TempDir`:

```rust
#[test]
fn missing_profile_is_distinct_from_damaged_profile() {
    let temp = tempfile::tempdir().unwrap();
    let store = ProfileStore::new(temp.path().to_path_buf(), KdfParams::testing());
    assert_eq!(store.load().unwrap(), ProfileLoad::Missing);

    std::fs::create_dir_all(temp.path()).unwrap();
    std::fs::write(temp.path().join("profile.json"), b"not-json").unwrap();
    assert!(matches!(store.load(), Err(StorageError::DamagedProfile)));
}

#[test]
fn profile_creation_never_writes_plaintext_secrets() {
    let temp = tempfile::tempdir().unwrap();
    let store = ProfileStore::new(temp.path().to_path_buf(), KdfParams::testing());
    let profile = profile_fixture();
    store.create(&profile).unwrap();

    let bytes = std::fs::read(store.profile_path()).unwrap();
    assert!(!bytes.windows(b"a secure master password".len())
        .any(|window| window == b"a secure master password"));
    assert_eq!(store.load().unwrap(), ProfileLoad::Valid(profile));
}

#[test]
fn reset_deletes_only_keynest_owned_security_files() {
    let temp = tempfile::tempdir().unwrap();
    let store = ProfileStore::new(temp.path().to_path_buf(), KdfParams::testing());
    std::fs::create_dir_all(temp.path()).unwrap();
    std::fs::write(temp.path().join("profile.json"), b"profile").unwrap();
    std::fs::write(temp.path().join("vault.enc"), b"vault").unwrap();
    std::fs::write(temp.path().join("keep.txt"), b"keep").unwrap();

    store.reset().unwrap();

    assert!(!temp.path().join("profile.json").exists());
    assert!(!temp.path().join("vault.enc").exists());
    assert!(temp.path().join("keep.txt").exists());
}
```

- [ ] **Step 2: Run storage tests and verify RED**

Run: `cargo test security::storage::tests --manifest-path src-tauri/Cargo.toml`

Expected: compilation fails because `ProfileStore`, `StoredProfile`, and storage errors do not exist.

- [ ] **Step 3: Implement validated, scoped storage**

Use this profile envelope:

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct StoredProfile {
    pub format_version: u32,
    pub kdf_algorithm: String,
    pub key_wrap_algorithm: String,
    pub wrapped_key: WrappedVaultKey,
}
```

Version 1 accepts only `argon2id`, `xchacha20poly1305`, the store's injected accepted KDF parameters, 16-byte salt, 24-byte nonce, and 48-byte wrapped ciphertext. Production always injects `KdfParams::production()`; only `#[cfg(test)]` fixtures can access `KdfParams::testing()`. Serialize pretty JSON to a `NamedTempFile` in the same directory, call `flush` and `sync_all`, then use `persist_noclobber` to create `profile.json` without overwriting an existing profile. Remove the temporary file on failure. `reset()` targets explicit literal paths only.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run: `cargo test security::storage::tests --manifest-path src-tauri/Cargo.toml`

Expected: all storage tests pass.

- [ ] **Step 5: Commit profile storage**

```bash
git add src-tauri/src/security
git commit -m "feat: persist encrypted KeyNest profile"
```

---

### Task 3: Authentication State Machine and Tauri Commands

**Files:**
- Create: `src-tauri/src/security/auth.rs`
- Modify: `src-tauri/src/security/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `ProfileStore`, `StoredProfile`, `wrap_new_vault_key`, `unwrap_vault_key`.
- Produces: `AuthService::load`, `status`, `create_master_password`, `unlock_at`, `lock`, `reset_keynest`, and crate-private `require_vault_key`.
- Tauri commands serialize `AuthStatus` as `setup-required`, `locked`, `unlocked`, or `data-error` and errors as `{ code, message, retryAfterMs }`.

- [ ] **Step 1: Write failing state-machine tests**

Add tests covering the complete backend contract:

```rust
#[test]
fn first_run_create_lock_and_unlock_round_trip() {
    let fixture = AuthFixture::new();
    assert_eq!(fixture.service.status(), AuthStatus::SetupRequired);

    fixture.service.create_master_password("a secure master password").unwrap();
    assert_eq!(fixture.service.status(), AuthStatus::Unlocked);

    fixture.service.lock();
    assert_eq!(fixture.service.status(), AuthStatus::Locked);
    assert_eq!(
        fixture.service.unlock("wrong master password"),
        Err(AuthError::InvalidCredentials),
    );
    assert_eq!(fixture.service.status(), AuthStatus::Locked);

    fixture.service.unlock("a secure master password").unwrap();
    assert_eq!(fixture.service.status(), AuthStatus::Unlocked);
}

#[test]
fn short_password_is_rejected_by_rust() {
    let fixture = AuthFixture::new();
    assert_eq!(
        fixture.service.create_master_password("too short"),
        Err(AuthError::PasswordTooShort),
    );
    assert_eq!(fixture.service.status(), AuthStatus::SetupRequired);
}

#[test]
fn damaged_profile_loads_fail_closed() {
    let fixture = AuthFixture::with_profile_bytes(b"not-json");
    assert_eq!(fixture.service.status(), AuthStatus::DataError);
    assert_eq!(fixture.service.require_vault_key(|_| ()), Err(AuthError::Unauthorized));
}

#[test]
fn reset_requires_exact_confirmation() {
    let fixture = AuthFixture::new();
    fixture.service.create_master_password("a secure master password").unwrap();
    assert_eq!(fixture.service.reset_keynest("reset"), Err(AuthError::InvalidResetConfirmation));
    assert_eq!(fixture.service.status(), AuthStatus::Unlocked);

    fixture.service.reset_keynest("RESET").unwrap();
    assert_eq!(fixture.service.status(), AuthStatus::SetupRequired);
}
```

Add an injected test clock or `unlock_at(password, Instant)` coverage proving failures three and later enforce delays of 1, 2, 4, then 5 seconds, capped at five seconds, and success clears the counter.

- [ ] **Step 2: Run auth tests and verify RED**

Run: `cargo test security::auth::tests --manifest-path src-tauri/Cargo.toml`

Expected: compilation fails because the authentication service is missing.

- [ ] **Step 3: Implement the minimal synchronized service**

Model state as:

```rust
enum AuthState {
    SetupRequired,
    Locked(StoredProfile),
    Unlocked { profile: StoredProfile, vault_key: VaultKey },
    DataError,
}

#[derive(Clone)]
pub(crate) struct AuthService {
    inner: Arc<Mutex<AuthInner>>,
    store: ProfileStore,
    entropy: Arc<dyn EntropySource>,
    kdf_params: KdfParams,
}
```

Keep failed-attempt count and `next_allowed_at` inside `AuthInner`. The production constructor creates both the store and crypto service with the same `KdfParams::production()` value; the test fixture injects `KdfParams::testing()` into both. Do not hold the mutex during unnecessary filesystem work. Ensure every transition fails closed, especially create/write failure and reset failure.

- [ ] **Step 4: Run auth tests and verify GREEN**

Run: `cargo test security::auth::tests --manifest-path src-tauri/Cargo.toml`

Expected: all state-machine tests pass.

- [ ] **Step 5: Write failing command-contract tests**

Test conversion from each internal error to its stable public code/message and verify the status enum serialization:

```rust
#[test]
fn auth_status_uses_kebab_case_ipc_values() {
    assert_eq!(serde_json::to_string(&AuthStatus::SetupRequired).unwrap(), "\"setup-required\"");
    assert_eq!(serde_json::to_string(&AuthStatus::DataError).unwrap(), "\"data-error\"");
}

#[test]
fn invalid_credentials_expose_only_a_generic_message() {
    let public = PublicAuthError::from(AuthError::InvalidCredentials);
    assert_eq!(public.code, "invalid-credentials");
    assert_eq!(public.message, "The master password is incorrect.");
}
```

- [ ] **Step 6: Run command tests and verify RED**

Run: `cargo test command_tests --manifest-path src-tauri/Cargo.toml`

Expected: compilation fails because the public DTOs and command registration do not exist.

- [ ] **Step 7: Wire Tauri setup and commands**

In `lib.rs`, resolve `app.path().app_data_dir()`, create and manage `AuthService`, and register:

```rust
#[tauri::command]
fn get_auth_status(auth: State<'_, AuthService>) -> AuthStatus;

#[tauri::command]
async fn create_master_password(
    mut password: String,
    auth: State<'_, AuthService>,
) -> Result<AuthStatus, PublicAuthError>;

#[tauri::command]
async fn unlock(
    mut password: String,
    auth: State<'_, AuthService>,
) -> Result<AuthStatus, PublicAuthError>;

#[tauri::command]
fn lock(auth: State<'_, AuthService>) -> AuthStatus;

#[tauri::command]
fn reset_keynest(
    confirmation: String,
    auth: State<'_, AuthService>,
) -> Result<AuthStatus, PublicAuthError>;
```

Clone the service into `tauri::async_runtime::spawn_blocking` for Argon2 work and zeroize each mutable password inside the blocking closure before returning. Remove the template `greet` command.

- [ ] **Step 8: Run all Rust tests and verify GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: all crypto, storage, auth, and command-contract tests pass without warnings.

- [ ] **Step 9: Commit the Rust authentication boundary**

```bash
git add src-tauri
git commit -m "feat: enforce encrypted master password in Tauri"
```

---

### Task 4: Frontend Test Harness and Authentication Client

**Files:**
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `vite.config.ts`
- Modify: `tsconfig.node.json`
- Create: `src/test/setup.ts`
- Create: `src/features/auth/types.ts`
- Create: `src/features/auth/authClient.ts`
- Create: `src/features/auth/authClient.test.ts`

**Interfaces:**
- Produces: `AuthStatus`, `AuthClientError`, and `authClient` methods matching the Rust commands exactly.
- Later UI tasks consume only `authClient`; they do not import Tauri `invoke` directly.

- [ ] **Step 1: Install the test dependencies**

Run:

```powershell
npm.cmd install --save-dev vitest jsdom @testing-library/react @testing-library/user-event @testing-library/jest-dom
```

Add scripts:

```json
"test": "vitest run",
"test:watch": "vitest"
```

Configure Vite `test.environment` as `jsdom`, `test.setupFiles` as `./src/test/setup.ts`, and global cleanup in the setup file.

- [ ] **Step 2: Write a failing authentication-client contract test**

Mock only `@tauri-apps/api/core` at the IPC boundary:

```ts
it("uses the exact backend commands and arguments", async () => {
  invokeMock.mockResolvedValue("unlocked");

  await authClient.createMasterPassword("a secure master password");
  await authClient.unlock("a secure master password");
  await authClient.lock();
  await authClient.resetKeynest("RESET");

  expect(invokeMock.mock.calls).toEqual([
    ["create_master_password", { password: "a secure master password" }],
    ["unlock", { password: "a secure master password" }],
    ["lock"],
    ["reset_keynest", { confirmation: "RESET" }],
  ]);
});
```

- [ ] **Step 3: Run the client test and verify RED**

Run: `npm.cmd test -- src/features/auth/authClient.test.ts`

Expected: failure because the authentication client does not exist.

- [ ] **Step 4: Implement the minimal typed client**

Implement exact methods:

```ts
export type AuthStatus =
  | "setup-required"
  | "locked"
  | "unlocked"
  | "data-error";

export const authClient = {
  getStatus: () => invoke<AuthStatus>("get_auth_status"),
  createMasterPassword: (password: string) =>
    invoke<AuthStatus>("create_master_password", { password }),
  unlock: (password: string) => invoke<AuthStatus>("unlock", { password }),
  lock: () => invoke<AuthStatus>("lock"),
  resetKeynest: (confirmation: string) =>
    invoke<AuthStatus>("reset_keynest", { confirmation }),
};
```

Normalize rejected structured Tauri values into `AuthClientError` without discarding `code`, safe `message`, or optional `retryAfterMs`.

- [ ] **Step 5: Run the client test and verify GREEN**

Run: `npm.cmd test -- src/features/auth/authClient.test.ts`

Expected: the client contract test passes.

- [ ] **Step 6: Commit the frontend testing boundary**

```bash
git add package.json package-lock.json vite.config.ts tsconfig.node.json src/test src/features/auth
git commit -m "test: add authentication client harness"
```

---

### Task 5: Setup, Unlock, Reset, and Data-Error Screens

**Files:**
- Create: `src/features/auth/components/AuthLayout.tsx`
- Create: `src/features/auth/components/PasswordField.tsx`
- Create: `src/features/auth/components/SetupScreen.tsx`
- Create: `src/features/auth/components/UnlockScreen.tsx`
- Create: `src/features/auth/components/ResetDialog.tsx`
- Create: `src/features/auth/components/DataErrorScreen.tsx`
- Create: `src/features/auth/components/SetupScreen.test.tsx`
- Create: `src/features/auth/components/UnlockScreen.test.tsx`
- Create: `src/shared/components/AppTitleBar.tsx`
- Modify: `src/pages/HomePage.tsx`

**Interfaces:**
- `SetupScreen({ onCreated })` calls the auth client and reports `unlocked` only after success.
- `UnlockScreen({ onUnlocked, onReset })` owns incorrect-password feedback and exposes reset.
- `ResetDialog({ isOpen, onClose, onReset })` invokes reset only for exact `RESET`.
- `DataErrorScreen({ onRetry, onReset })` offers no route to protected content.

- [ ] **Step 1: Write failing setup-screen tests**

Cover the policy before implementation:

```tsx
it("requires twelve characters and a matching confirmation", async () => {
  render(<SetupScreen onCreated={onCreated} />);
  await user.type(screen.getByLabelText("Master password"), "short");
  await user.type(screen.getByLabelText("Confirm master password"), "different");
  await user.click(screen.getByRole("button", { name: "Create Master Password" }));

  expect(screen.getByText("Use at least 12 characters.")).toBeInTheDocument();
  expect(authClient.createMasterPassword).not.toHaveBeenCalled();
});

it("clears both fields after successful creation", async () => {
  authClient.createMasterPassword.mockResolvedValue("unlocked");
  render(<SetupScreen onCreated={onCreated} />);
  await user.type(screen.getByLabelText("Master password"), "a secure master password");
  await user.type(screen.getByLabelText("Confirm master password"), "a secure master password");
  await user.click(screen.getByRole("button", { name: "Create Master Password" }));

  expect(onCreated).toHaveBeenCalled();
  expect(screen.getByLabelText("Master password")).toHaveValue("");
  expect(screen.getByLabelText("Confirm master password")).toHaveValue("");
});
```

- [ ] **Step 2: Run setup tests and verify RED**

Run: `npm.cmd test -- src/features/auth/components/SetupScreen.test.tsx`

Expected: failure because `SetupScreen` does not exist.

- [ ] **Step 3: Implement setup and shared password controls**

Use semantic `<form>`, labeled inputs, `aria-live` errors, show/hide buttons with changing accessible names, pending-state disabled controls, and the non-recovery warning. Count password length with `Array.from(password).length` so React and Rust both use Unicode characters rather than UTF-16 code units/bytes.

- [ ] **Step 4: Run setup tests and verify GREEN**

Run: `npm.cmd test -- src/features/auth/components/SetupScreen.test.tsx`

Expected: setup tests pass.

- [ ] **Step 5: Write failing unlock and reset tests**

```tsx
it("clears and refocuses the password after invalid credentials", async () => {
  authClient.unlock.mockRejectedValue(
    new AuthClientError("invalid-credentials", "The master password is incorrect."),
  );
  render(<UnlockScreen onUnlocked={onUnlocked} onReset={onReset} />);
  const field = screen.getByLabelText("Master password");
  await user.type(field, "wrong master password");
  await user.keyboard("{Enter}");

  expect(field).toHaveValue("");
  expect(field).toHaveFocus();
  expect(screen.getByText("The master password is incorrect.")).toBeInTheDocument();
});

it("requires exact RESET before deleting local data", async () => {
  render(<ResetDialog isOpen onClose={onClose} onReset={onReset} />);
  const resetButton = screen.getByRole("button", { name: "Reset KeyNest" });
  expect(resetButton).toBeDisabled();
  await user.type(screen.getByLabelText("Type RESET to confirm"), "RESET");
  expect(resetButton).toBeEnabled();
  await user.click(resetButton);
  expect(onReset).toHaveBeenCalledWith("RESET");
});
```

- [ ] **Step 6: Run unlock tests and verify RED**

Run: `npm.cmd test -- src/features/auth/components/UnlockScreen.test.tsx`

Expected: failure because unlock and reset components do not exist.

- [ ] **Step 7: Implement unlock, reset, damaged-data, and shared auth layout**

Extract the existing title-bar JSX into `AppTitleBar` without behavior changes, then render it inside both `HomePage` and `AuthLayout`. Keep reset as an accessible modal (`role="dialog"`, `aria-modal="true"`) and return focus to the reset trigger when dismissed. A data-error screen offers only retry and reset.

- [ ] **Step 8: Run component tests and verify GREEN**

Run: `npm.cmd test -- src/features/auth/components`

Expected: all auth screen tests pass.

- [ ] **Step 9: Commit the authentication screens**

```bash
git add src/features/auth/components src/shared/components/AppTitleBar.tsx src/pages/HomePage.tsx
git commit -m "feat: add master password screens"
```

---

### Task 6: Authentication Gate and Manual Lock Integration

**Files:**
- Create: `src/features/auth/components/AuthGate.tsx`
- Create: `src/features/auth/components/AuthGate.test.tsx`
- Modify: `src/app/App.tsx`
- Create: `src/app/App.test.tsx`
- Modify: `src/pages/HomePage.tsx`
- Modify: `src/shared/components/NavigationSidebar.tsx`

**Interfaces:**
- `AuthGate` accepts `children: (controls: { lock: () => Promise<void> }) => ReactNode`.
- `HomePage` gains `onLockKeynest: () => Promise<void>` and passes it to the sidebar.
- The sidebar closes first, then awaits the real backend lock callback.

- [ ] **Step 1: Write failing gate tests**

```tsx
it("does not mount protected content until Rust reports unlocked", async () => {
  authClient.getStatus.mockReturnValue(statusPromise.promise);
  render(
    <AuthGate>{() => <div>Protected home</div>}</AuthGate>,
  );
  expect(screen.queryByText("Protected home")).not.toBeInTheDocument();

  statusPromise.resolve("locked");
  expect(await screen.findByRole("heading", { name: "Welcome back" })).toBeInTheDocument();
  expect(screen.queryByText("Protected home")).not.toBeInTheDocument();
});

it("fails closed when status lookup rejects", async () => {
  authClient.getStatus.mockRejectedValue(new Error("IPC unavailable"));
  render(<AuthGate>{() => <div>Protected home</div>}</AuthGate>);
  expect(await screen.findByText("KeyNest could not verify your local data.")).toBeInTheDocument();
  expect(screen.queryByText("Protected home")).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Run gate tests and verify RED**

Run: `npm.cmd test -- src/features/auth/components/AuthGate.test.tsx`

Expected: failure because `AuthGate` does not exist.

- [ ] **Step 3: Implement the startup coordinator**

Use an exhaustive status switch. The initial state is `checking`; unknown or rejected results become `data-error`. Setup/unlock callbacks transition only when the backend returns `unlocked`. Reset transitions only when the backend returns `setup-required`. The `lock` callback unmounts protected content only after Rust returns `locked`; if IPC fails, show a blocking error and do not claim the key was discarded.

- [ ] **Step 4: Run gate tests and verify GREEN**

Run: `npm.cmd test -- src/features/auth/components/AuthGate.test.tsx`

Expected: all gate tests pass.

- [ ] **Step 5: Write the failing app integration test**

Mock only the auth client and Tauri window functions, render `App`, unlock it, open the navigation, click **Lock KeyNest**, and assert:

```tsx
expect(authClient.lock).toHaveBeenCalledOnce();
expect(await screen.findByRole("heading", { name: "Welcome back" })).toBeInTheDocument();
expect(screen.queryByText("Keep your important information")).not.toBeInTheDocument();
```

- [ ] **Step 6: Run the app integration test and verify RED**

Run: `npm.cmd test -- src/app/App.test.tsx`

Expected: failure because the app is not gated and the sidebar lock button has no callback.

- [ ] **Step 7: Wire `App`, `HomePage`, and `NavigationSidebar`**

Replace direct `HomePage` rendering with:

```tsx
<AuthGate>
  {({ lock }) => (
    <HomePage
      onOpenPasswordVault={openPasswordVault}
      onLockKeynest={lock}
    />
  )}
</AuthGate>
```

Update the sidebar prop and lock button handler, closing navigation before invoking `onLockKeynest`.

- [ ] **Step 8: Run app and frontend tests and verify GREEN**

Run: `npm.cmd test`

Expected: all frontend tests pass with no React act warnings or unhandled rejections.

- [ ] **Step 9: Commit the enforced frontend gate**

```bash
git add src/app src/features/auth src/pages/HomePage.tsx src/shared/components/NavigationSidebar.tsx
git commit -m "feat: gate KeyNest behind master password"
```

---

### Task 7: Authentication Styling and Full Verification

**Files:**
- Modify: `src/App.css`
- Modify: `README.md`

**Interfaces:**
- No new logic interfaces. Styling must preserve keyboard focus, responsive layout, custom title-bar controls, pending/disabled states, and destructive-action contrast.

- [ ] **Step 1: Add authentication styles**

Add focused classes for the full-window auth background, centered auth card, lock emblem, password-field wrapper, reveal button, inline errors, non-recovery warning, reset link, modal/backdrop, damaged-data actions, and loading indicator. Reuse the current dark green KeyNest palette and ensure visible `:focus-visible` outlines. At widths below 520px, reduce card padding and keep all controls at least 44px high.

- [ ] **Step 2: Document the local security behavior**

Replace the template README with concise setup/run instructions and these user-facing guarantees:

```markdown
- KeyNest requires the master password on every launch.
- The master password cannot be recovered.
- Reset permanently deletes KeyNest's encrypted local profile and vault.
- KeyNest protects stored data at rest but cannot protect an unlocked device from malware or keyloggers.
```

- [ ] **Step 3: Run formatting checks**

Run: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`

If it reports differences, run `cargo fmt --manifest-path src-tauri/Cargo.toml`, then rerun the check.

- [ ] **Step 4: Run the complete frontend suite**

Run: `npm.cmd test`

Expected: all test files and tests pass, with no warnings.

- [ ] **Step 5: Run the complete Rust suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: all tests pass with no ignored security tests or warnings.

- [ ] **Step 6: Run production builds**

Run: `npm.cmd run build`

Expected: TypeScript and Vite finish with exit code 0.

Run: `cargo build --manifest-path src-tauri/Cargo.toml`

Expected: the Tauri Rust application builds with exit code 0.

- [ ] **Step 7: Inspect persisted-data and authorization requirements**

Re-read the approved spec and confirm in code/tests that plaintext secrets are not serialized, malformed metadata fails closed, reset targets only explicit filenames, no vault key is returned from a command, and protected content is absent before backend unlock.

- [ ] **Step 8: Run a browser/Tauri smoke test**

Start the development app, then verify first-run setup, wrong password, correct password, manual lock, process restart lock, reset confirmation, and responsive layout. If native Tauri launch is unavailable, verify the complete renderer flow in the in-app browser with a deterministic development mock and explicitly report the native gap.

- [ ] **Step 9: Commit the verified feature**

```bash
git add src/App.css README.md
git commit -m "docs: explain KeyNest master password security"
```

---

## Plan Self-Review Results

- Every approved spec requirement maps to a task and an explicit verification step.
- The Rust and TypeScript status names match exactly across tasks.
- The IPC command names and camel-cased Tauri arguments match the frontend client contract.
- No task grants React access to the vault key or treats React state as authorization.
- Destructive reset is scoped to two literal KeyNest filenames and exact `RESET` confirmation.
- The implementation deliberately excludes recovery, password change, inactivity locking, focus locking, and vault-record CRUD.
