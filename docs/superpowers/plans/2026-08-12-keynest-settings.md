# KeyNest Settings and Enforced Security Preferences Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a dedicated Settings page whose security preferences are validated and enforced by Rust, including mandatory auto-lock and clipboard clearing, safe master-password changes, Windows autostart, complete themes, About information, and protected reset flows.

**Architecture:** Keep React responsible for presentation, navigation, theme application, and throttled activity reports. Add focused Rust services for settings persistence, clipboard ownership, auto-lock coordination, startup registration, and authentication mutations; Rust remains the authority for lock state and never sends the vault key to React.

**Tech Stack:** React 19, TypeScript 5.8, Vitest 4, Testing Library, Vite 7, Tauri 2, Rust 2021, Argon2id, XChaCha20-Poly1305, `atomic-write-file`, official Tauri autostart and clipboard-manager plugins

**Design spec:** `docs/superpowers/specs/2026-08-12-keynest-settings-design.md`

## Global Constraints

- Preserve the user's uncommitted `src-tauri/tauri.conf.json` change that sets the default/minimum window to 1000 x 700; never stage that file in this feature.
- Settings is a dedicated authenticated page, not a modal; only destructive reset uses a dialog.
- Auto-lock is always enabled. Allowed seconds are exactly `60`, `300`, `900`, and `1800`; default is `300`; there is no Never value.
- Clipboard clearing is always enabled. Allowed seconds are exactly `10`, `30`, and `60`; default is `30`; there is no Never value.
- Sleep protection locks on Tauri `RunEvent::Resumed` before authenticated interaction continues.
- Master passwords require at least 12 Unicode characters; a successful change re-wraps the same vault key and leaves the current session unlocked.
- Launch at startup is off by default; an autostart launch uses `--autostart`, starts minimized, and remains locked.
- Theme values are exactly `system`, `dark`, and `light`; default is `system`.
- Settings reset requires the current password and exact `RESET KEYNEST`; locked recovery reset requires the phrase but cannot require the forgotten or damaged password.
- Never log, persist, place in URLs, or include in public errors any password, derived key, vault key, vault record, or clipboard secret.
- Backend mutations require `AuthStatus::Unlocked`; non-secret settings reads remain available while locked so authentication screens receive the chosen theme.
- Use strict versioned JSON, secure fallback defaults, Windows-safe atomic replacement, deterministic adapter-based tests, and fail-closed error handling.
- Do not add backup/restore, Windows Hello, cloud sync, accounts, analytics, update checking, a system tray, or placeholder vault-copy UI.
- Do not stage `.superpowers/` or unrelated working-tree changes in any commit.

---

## File Structure

### Rust backend

- Create `src-tauri/src/settings/{mod.rs,model.rs,storage.rs,service.rs}` for allowed values, snapshots, strict persistence, and synchronized mutations.
- Create `src-tauri/src/platform/{mod.rs,startup.rs}` for injectable autostart registration.
- Create `src-tauri/src/security/{clipboard.rs,locking.rs,auto_lock.rs}` for clipboard ownership and all lock triggers.
- Create `src-tauri/src/ipc.rs` for safe errors and Tauri commands moved out of `lib.rs`.
- Modify `src-tauri/src/security/{crypto.rs,storage.rs,auth.rs,mod.rs}` for existing-key wrapping, atomic profile replacement, password change, and reset rules.
- Modify `src-tauri/src/lib.rs` for plugin/service initialization, handlers, autostart minimization, and resume locking.
- Modify Rust/npm manifests, lockfiles, and `src-tauri/capabilities/default.json` for the approved official plugins and atomic writes.

### React frontend

- Create `src/features/settings/{types.ts,settingsClient.ts,SettingsProvider.tsx,ActivityReporter.tsx}` with matching colocated tests.
- Create `src/shared/components/AuthenticatedShell.tsx` and test for shared authenticated chrome and navigation.
- Create `src/pages/SettingsPage.tsx` and test for category navigation.
- Create `src/features/settings/components/{SecuritySettings.tsx,ChangeMasterPasswordForm.tsx,AuthenticatedResetDialog.tsx,GeneralSettings.tsx,AppearanceSettings.tsx,AboutSettings.tsx}` with focused tests.
- Modify the auth client/gate/reset components, sidebar, Home, App, and their tests for the new commands and transitions.
- Modify `src/App.css` for complete tokenized Dark/Light palettes and responsive Settings layout.

## Task 1: Versioned Settings Model and Atomic Persistence

**Files:**
- Create: `src-tauri/src/settings/mod.rs`
- Create: `src-tauri/src/settings/model.rs`
- Create: `src-tauri/src/settings/storage.rs`
- Create: `src-tauri/src/settings/service.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`

**Interfaces:**
- Consumes: Tauri's application-data directory.
- Produces: `SettingsValues`, `ThemePreference`, `SettingsSnapshot`, `SettingsStore::load/replace/reset`, and validated `SettingsService` mutations.

- [ ] **Step 1: Establish the passing baseline**

```powershell
npm.cmd test
npm.cmd run build
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: every command exits 0. Stop and report any baseline failure.

- [ ] **Step 2: Add Windows-safe atomic replacement support**

```powershell
cargo add atomic-write-file@0.3 --manifest-path src-tauri/Cargo.toml
```

Expected: Cargo adds `atomic-write-file = "0.3"` and updates `src-tauri/Cargo.lock`.

- [ ] **Step 3: Write failing model/storage tests**

```rust
#[test]
fn missing_settings_use_secure_defaults() {
    let temp = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(temp.path().to_path_buf());
    assert_eq!(store.load().unwrap(), SettingsLoad::Missing);
    assert_eq!(SettingsValues::default().auto_lock_seconds, 300);
    assert_eq!(SettingsValues::default().clipboard_clear_seconds, 30);
    assert_eq!(SettingsValues::default().theme, ThemePreference::System);
}

#[test]
fn invalid_security_values_are_damaged() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("settings.json"),
        br#"{"format_version":1,"auto_lock_seconds":0,"clipboard_clear_seconds":0,"theme":"dark"}"#,
    ).unwrap();
    let store = SettingsStore::new(temp.path().to_path_buf());
    assert_eq!(store.load().unwrap(), SettingsLoad::Damaged);
}

#[test]
fn replacement_round_trips_allowed_values() {
    let temp = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(temp.path().to_path_buf());
    let values = SettingsValues {
        auto_lock_seconds: 900,
        clipboard_clear_seconds: 60,
        theme: ThemePreference::Light,
    };
    store.replace(values).unwrap();
    assert_eq!(store.load().unwrap(), SettingsLoad::Valid(values));
}
```

Also test unknown fields, version `2`, auto-lock `301`, clipboard `31`, and theme `"blue"`; each produces `SettingsLoad::Damaged`.

- [ ] **Step 4: Confirm the focused tests fail**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml settings::
```

Expected: FAIL because the settings module does not exist.

- [ ] **Step 5: Implement the exact domain contract**

```rust
pub(crate) const AUTO_LOCK_OPTIONS: [u64; 4] = [60, 300, 900, 1800];
pub(crate) const CLIPBOARD_CLEAR_OPTIONS: [u64; 3] = [10, 30, 60];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ThemePreference {
    #[default]
    System,
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SettingsValues {
    pub auto_lock_seconds: u64,
    pub clipboard_clear_seconds: u64,
    pub theme: ThemePreference,
}

impl Default for SettingsValues {
    fn default() -> Self {
        Self { auto_lock_seconds: 300, clipboard_clear_seconds: 30, theme: ThemePreference::System }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsSnapshot {
    pub auto_lock_seconds: u64,
    pub clipboard_clear_seconds: u64,
    pub theme: ThemePreference,
    pub launch_at_startup: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}
```

Use private `StoredSettings` with `#[serde(deny_unknown_fields)]` and `format_version: 1`. `SettingsLoad` is `Missing | Valid(SettingsValues) | Damaged`. Storage serializes pretty JSON, creates the parent, writes with `AtomicWriteFile`, calls `sync_all()`, then `commit()`.

`SettingsService` loads missing/damaged files as defaults. Only damaged data sets warning text `KeyNest restored secure settings defaults because the saved preferences were invalid.` Each setter validates, atomically persists, then updates mutex-protected memory; failed persistence leaves prior memory unchanged.

- [ ] **Step 6: Export and initialize without renderer commands**

```rust
mod model;
mod service;
mod storage;

pub(crate) use model::{SettingsSnapshot, SettingsValues, ThemePreference};
pub(crate) use service::{SettingsError, SettingsService};
pub(crate) use storage::SettingsStore;
```

Add `mod settings;` to `lib.rs`; construct the settings store from `app_data_dir.clone()` and manage the loaded service.

- [ ] **Step 7: Verify and commit**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml settings::
cargo test --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/settings src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat: add validated settings storage"
```

Expected: all Rust tests pass and only the named files are committed.

## Task 2: Atomic Master-Password Change

**Files:**
- Modify: `src-tauri/src/security/crypto.rs`
- Modify: `src-tauri/src/security/storage.rs`
- Modify: `src-tauri/src/security/auth.rs`
- Modify: `src-tauri/src/security/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: unlocked `AuthService`, `VaultKey`, KDF parameters, entropy, and `ProfileStore`.
- Produces: `wrap_existing_vault_key`, `ProfileStore::replace`, `AuthService::change_master_password`, and IPC `change_master_password(currentPassword, newPassword)`.

- [ ] **Step 1: Write failing crypto/auth tests**

```rust
#[test]
fn existing_vault_key_can_be_rewrapped_without_changing_it() {
    let entropy = FixedEntropy;
    let (_, vault_key) = wrap_new_vault_key(
        "old secure master password", KdfParams::testing(), &entropy
    ).unwrap();
    let wrapped = wrap_existing_vault_key(
        "new secure master password", vault_key.expose(), KdfParams::testing(), &entropy
    ).unwrap();
    let unwrapped = unwrap_vault_key("new secure master password", &wrapped).unwrap();
    assert_eq!(unwrapped.expose_for_test(), vault_key.expose_for_test());
}

#[test]
fn successful_password_change_preserves_key_and_session() {
    let fixture = AuthFixture::new();
    fixture.service.create_master_password("old secure master password").unwrap();
    let before = fixture.service.require_vault_key(|key| *key).unwrap();
    fixture.service.change_master_password(
        "old secure master password", "new secure master password"
    ).unwrap();
    assert_eq!(fixture.service.status(), AuthStatus::Unlocked);
    assert_eq!(fixture.service.require_vault_key(|key| *key).unwrap(), before);
    fixture.service.lock();
    assert_eq!(fixture.service.unlock("old secure master password"), Err(AuthError::InvalidCredentials));
    fixture.service.unlock("new secure master password").unwrap();
}
```

Add tests for short new password, incorrect current password, locked calls, and injected replacement failure preserving old profile/password and unlocked session.

- [ ] **Step 2: Confirm focused failure**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml password_change
```

Expected: FAIL because the interfaces are missing.

- [ ] **Step 3: Add reusable wrapping and profile replacement**

```rust
pub(crate) fn wrap_existing_vault_key(
    password: &str,
    vault_key: &[u8; 32],
    params: KdfParams,
    entropy: &dyn EntropySource,
) -> Result<WrappedVaultKey, CryptoError>
```

Generate a fresh 16-byte salt and 24-byte nonce, derive a zeroizing wrapping key, encrypt the supplied key with XChaCha20-Poly1305 plus `PROFILE_AAD`, and return encoded material. Refactor new-key creation to call this function.

Add `ProfileStore::replace(&StoredProfile)` with `AtomicWriteFile`, `sync_all`, and `commit`; keep `create` create-only. Test injected failure leaves original bytes unchanged.

- [ ] **Step 4: Implement the auth transaction**

```rust
pub(crate) fn change_master_password(
    &self,
    current_password: &str,
    new_password: &str,
) -> Result<(), AuthError>
```

Validate 12 Unicode characters, require `Unlocked`, unwrap using the current password, compare with the session key, re-wrap the same key, atomically replace storage, then replace only the in-memory profile. Map bad current password to `InvalidCredentials`; all failures keep the old profile and session.

- [ ] **Step 5: Add safe async IPC**

Accept `currentPassword` and `newPassword`, use `spawn_blocking`, zeroize both owned strings, return `unlocked`, and add public error tests for `password-too-short`, `invalid-credentials`, and `unauthorized`.

- [ ] **Step 6: Verify and commit**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/security src-tauri/src/lib.rs
git commit -m "feat: change master password atomically"
```

Expected: all Rust tests pass.

## Task 3: Strengthened Reset Contracts

**Files:**
- Modify: `src-tauri/src/security/auth.rs`
- Modify: `src-tauri/src/security/storage.rs`
- Modify: `src-tauri/src/settings/storage.rs`
- Modify: `src-tauri/src/settings/service.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: recovery reset, unlocked auth, profile/vault store, and settings store.
- Produces: exact-phrase validation, current-password validation, scoped `finish_reset`, and settings reset-to-default behavior for Task 6 orchestration.

- [ ] **Step 1: Write failing reset tests**

```rust
#[test]
fn authenticated_reset_requires_password_and_exact_phrase() {
    let fixture = AuthFixture::new();
    fixture.service.create_master_password("a secure master password").unwrap();
    assert_eq!(
        fixture.service.reset_keynest_authenticated("wrong master password", "RESET KEYNEST"),
        Err(AuthError::InvalidCredentials),
    );
    assert_eq!(
        fixture.service.reset_keynest_authenticated("a secure master password", "RESET"),
        Err(AuthError::InvalidResetConfirmation),
    );
    assert_eq!(fixture.service.status(), AuthStatus::Unlocked);
}
```

Update old phrase tests to `RESET KEYNEST`. Add a settings test proving `reset()` deletes `settings.json`, restores defaults in memory, and clears warning state.

- [ ] **Step 2: Confirm failure**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml reset
```

Expected: FAIL on the old phrase and missing protected reset.

- [ ] **Step 3: Split reset validation from scoped deletion**

Use `const RESET_CONFIRMATION: &str = "RESET KEYNEST";`. Add `validate_reset_confirmation(confirmation)` for the locked recovery path and `validate_authenticated_reset(current_password, confirmation)` for Settings. The latter requires `Unlocked`, validates the phrase, verifies the current password, and performs no deletion. Add `finish_reset()` as the only method that deletes the profile/vault and changes state to `SetupRequired`.

Keep the existing `reset_keynest(confirmation)` service method temporarily as a compatibility wrapper that calls phrase validation followed by `finish_reset()`. Task 6 replaces the command's direct use of that wrapper with cross-service orchestration before any new Settings reset command is registered.

`SettingsService::reset()` removes only `settings.json` and changes memory to defaults only after deletion succeeds.

- [ ] **Step 4: Verify the two-phase service contract**

Add tests proving validation failures leave all files and state untouched and that `finish_reset()` is not called implicitly by either validation method. Do not register `reset_keynest_authenticated` yet; Task 6 adds it only after autostart, clipboard, settings, and auth deletion are ordered correctly.

- [ ] **Step 5: Verify and commit**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml reset
cargo test --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/security src-tauri/src/settings src-tauri/src/lib.rs
git commit -m "feat: strengthen KeyNest reset confirmation"
```

Expected: all reset and regression tests pass.

## Task 4: Ownership-Safe Clipboard Service

**Files:**
- Create: `src-tauri/src/security/clipboard.rs`
- Modify: `src-tauri/src/security/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `package.json`
- Modify: `package-lock.json`

**Interfaces:**
- Consumes: official Tauri clipboard-manager Rust API and validated clipboard timeout.
- Produces: `ClipboardPort`, `TauriClipboardPort`, and cloneable `ClipboardService::copy_secret/set_timeout/clear_if_owned`.

- [ ] **Step 1: Install matching Tauri 2 dependencies**

```powershell
npm.cmd install @tauri-apps/plugin-clipboard-manager@^2
cargo add tauri-plugin-clipboard-manager@2 --manifest-path src-tauri/Cargo.toml
```

Expected: npm/Cargo manifests and locks contain Tauri 2 clipboard-manager packages.

- [ ] **Step 2: Write failing adapter tests**

```rust
#[test]
fn expiry_clears_only_matching_owned_content() {
    let port = Arc::new(FakeClipboard::default());
    let service = ClipboardService::new(port.clone(), Duration::from_secs(30));
    let generation = service.copy_secret_for_test("secret-one").unwrap();
    service.expire_generation_for_test(generation).unwrap();
    assert_eq!(port.text(), "");

    let generation = service.copy_secret_for_test("secret-two").unwrap();
    port.set_text("newer user value");
    service.expire_generation_for_test(generation).unwrap();
    assert_eq!(port.text(), "newer user value");
}

#[test]
fn stale_generation_cannot_clear_a_newer_copy() {
    let port = Arc::new(FakeClipboard::default());
    let service = ClipboardService::new(port.clone(), Duration::from_secs(30));
    let first = service.copy_secret_for_test("first secret").unwrap();
    let second = service.copy_secret_for_test("second secret").unwrap();
    service.expire_generation_for_test(first).unwrap();
    assert_eq!(port.text(), "second secret");
    service.expire_generation_for_test(second).unwrap();
    assert_eq!(port.text(), "");
}
```

Also test immediate lock clearing, best-effort process-exit clearing, changed clipboard preservation, timeout replacement, and port errors whose displayed form contains no secret.

- [ ] **Step 3: Confirm failure**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml clipboard
```

Expected: FAIL because the service is missing.

- [ ] **Step 4: Implement the ownership boundary**

```rust
pub(crate) trait ClipboardPort: Send + Sync {
    fn write_text(&self, value: &str) -> Result<(), ClipboardError>;
    fn read_text(&self) -> Result<String, ClipboardError>;
    fn clear(&self) -> Result<(), ClipboardError>;
}

#[derive(Clone)]
pub(crate) struct ClipboardService {
    inner: Arc<Mutex<ClipboardState>>,
    port: Arc<dyn ClipboardPort>,
}

struct ClipboardState {
    next_generation: u64,
    owned: Option<OwnedClipboard>,
    timeout: Duration,
}

struct OwnedClipboard {
    generation: u64,
    value: Zeroizing<String>,
}
```

`copy_secret` writes first, replaces ownership, increments generation, and starts one background expiry for that generation. Expiry removes ownership under the mutex, releases the mutex before port access, reads off the main thread, and clears only on exact equality. Superseded generations leave newer values untouched. `clear_if_owned` performs the same comparison and always drops zeroizing ownership.

Implement `TauriClipboardPort` through `tauri_plugin_clipboard_manager::ClipboardExt` on a cloned `AppHandle`. Do not expose clipboard plugin permissions to the renderer.

- [ ] **Step 5: Register plugin/service and verify**

Add `.plugin(tauri_plugin_clipboard_manager::init())`, construct the service in setup using saved timeout, and manage it. Add no copy IPC because vault records are out of scope.

```powershell
cargo test --manifest-path src-tauri/Cargo.toml clipboard
cargo test --manifest-path src-tauri/Cargo.toml
npm.cmd run build
git add src-tauri/src/security src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock package.json package-lock.json
git commit -m "feat: add secure clipboard ownership service"
```

Expected: tests pass without changing the developer's real clipboard.

## Task 5: Lock Coordinator, Auto-Lock, and Resume Protection

**Files:**
- Create: `src-tauri/src/security/locking.rs`
- Create: `src-tauri/src/security/auto_lock.rs`
- Modify: `src-tauri/src/security/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: auth, clipboard, saved timeout, Tauri emitter, and lifecycle events.
- Produces: `LockCoordinator::lock_and_emit`, `AutoLockService::arm/disarm/record_activity/set_timeout/lock_now`, command `record_activity`, and event `keynest://locked`.

- [ ] **Step 1: Write failing deterministic tests**

```rust
#[test]
fn deadline_expiry_locks_once() {
    let actions = Arc::new(FakeLockActions::default());
    let service = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(300));
    let start = Instant::now();
    service.arm_at_for_test(start);
    assert!(!service.expire_at_for_test(start + Duration::from_secs(299)));
    assert!(service.expire_at_for_test(start + Duration::from_secs(300)));
    assert_eq!(actions.lock_count(), 1);
}

#[test]
fn shortening_timeout_can_lock_immediately() {
    let actions = Arc::new(FakeLockActions::default());
    let service = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(300));
    let start = Instant::now();
    service.arm_at_for_test(start);
    service.record_activity_at_for_test(start + Duration::from_secs(100));
    service.set_timeout_at_for_test(Duration::from_secs(60), start + Duration::from_secs(170));
    assert_eq!(actions.lock_count(), 1);
}
```

Also prove disarmed state never locks, repeated triggers are idempotent, and resume lock calls clipboard cleanup before event emission.

- [ ] **Step 2: Confirm failure**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml auto_lock
cargo test --manifest-path src-tauri/Cargo.toml locking
```

Expected: FAIL because both services are missing.

- [ ] **Step 3: Implement one lock coordinator**

```rust
pub(crate) trait LockEventSink: Send + Sync {
    fn emit_locked(&self) -> Result<(), LockError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LockOutcome {
    pub status: AuthStatus,
    pub transitioned: bool,
}

#[derive(Clone)]
pub(crate) struct LockCoordinator {
    auth: AuthService,
    clipboard: ClipboardService,
    events: Arc<dyn LockEventSink>,
}

impl LockCoordinator {
    pub(crate) fn lock_and_emit(&self) -> Result<AuthStatus, LockError> {
        let outcome = self.auth.lock();
        let clipboard_result = self.clipboard.clear_if_owned();
        if outcome.transitioned {
            self.events.emit_locked()?;
        }
        clipboard_result?;
        Ok(outcome.status)
    }
}
```

Change `AuthService::lock()` to return `LockOutcome` while holding its existing state mutex; `transitioned` is true only for `Unlocked -> Locked`. Update its existing tests and the manual-lock command to return `outcome.status`. The Tauri sink emits `keynest://locked` without payload, so simultaneous/repeated triggers cannot emit duplicate lock transitions or restore state.

- [ ] **Step 4: Implement the monotonic supervisor**

Use `Arc<(Mutex<AutoLockState>, Condvar)>` with `armed`, `last_activity`, `timeout`, and `shutdown`. One supervisor thread waits until the deadline or condition notification. `arm()` uses `Instant::now()`, `record_activity()` changes only an armed deadline, `set_timeout()` recalculates from last activity and locks immediately if elapsed, `disarm()` removes the deadline, and shutdown wakes the thread. Production durations come only from validated settings; tests use injected-time helpers and never sleep a real minute.

- [ ] **Step 5: Wire transitions and lifecycle**

Successful create/unlock arms; manual lock coordinates and disarms; timeout/resume call `lock_now`; reset disarms. `record_activity` requires `Unlocked` and accepts no timestamp. On `RunEvent::Exit`, request `ClipboardService::clear_if_owned()` before plugin cleanup; this is best effort because the operating system can still terminate a process without delivering a graceful exit event.

Change `run()` to build the app and call `app.run`. On `RunEvent::Resumed`, retrieve `AutoLockService` and call `lock_now()`. Locked/setup resume is harmless.

- [ ] **Step 6: Verify and commit**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml auto_lock
cargo test --manifest-path src-tauri/Cargo.toml locking
cargo test --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/security src-tauri/src/lib.rs
git commit -m "feat: enforce inactivity and resume locking"
```

Expected: all deterministic lock tests pass.

## Task 6: Autostart Adapter and Complete Settings IPC

**Files:**
- Create: `src-tauri/src/platform/mod.rs`
- Create: `src-tauri/src/platform/startup.rs`
- Create: `src-tauri/src/ipc.rs`
- Modify: `src-tauri/src/settings/{model.rs,service.rs}`
- Modify: `src-tauri/src/security/{auto_lock.rs,clipboard.rs}`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `package.json`, `package-lock.json`
- Modify: `src-tauri/capabilities/default.json`

**Interfaces:**
- Consumes: settings, auto-lock, clipboard, auth, coordinator, app-data path, and official autostart/opener APIs.
- Produces: all approved commands, `StartupService`, safe `PublicIpcError`, fixed data-folder opening, and complete reset orchestration.

- [ ] **Step 1: Install Tauri 2 autostart dependencies**

```powershell
npm.cmd install @tauri-apps/plugin-autostart@^2
cargo add tauri-plugin-autostart@2 --manifest-path src-tauri/Cargo.toml
```

Expected: npm/Cargo manifests and locks contain the official plugin.

- [ ] **Step 2: Write failing startup/IPC tests**

```rust
#[test]
fn reset_aborts_before_deletion_when_autostart_disable_fails() {
    let fixture = CommandFixture::with_startup(FakeStartup::enabled_with_disable_failure());
    fixture.create_unlocked_profile();
    let error = fixture.reset_authenticated(
        "a secure master password", "RESET KEYNEST"
    ).unwrap_err();
    assert_eq!(error.code, "startup-error");
    assert!(fixture.profile_exists());
    assert!(fixture.settings_exist());
}
```

Add tests for enable/disable/query failures, snapshot camelCase serialization, invalid timeout/theme values, unauthorized mutations, fixed folder path, and safe messages.

- [ ] **Step 3: Confirm failure**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml startup
cargo test --manifest-path src-tauri/Cargo.toml command_tests
```

Expected: FAIL because full orchestration is missing.

- [ ] **Step 4: Implement startup behind an adapter**

```rust
pub(crate) trait StartupRegistration: Send + Sync {
    fn is_enabled(&self) -> Result<bool, StartupError>;
    fn enable(&self) -> Result<(), StartupError>;
    fn disable(&self) -> Result<(), StartupError>;
}

#[derive(Clone)]
pub(crate) struct StartupService {
    registration: Arc<dyn StartupRegistration>,
}
```

`set_enabled` performs enable/disable, then returns actual queried state. The Tauri implementation uses `ManagerExt`. Initialize with `MacosLauncher::LaunchAgent` and `Some(vec!["--autostart"])`. In setup, detect that exact argument and minimize `main`; do not hide or add a tray.

- [ ] **Step 5: Move and complete IPC**

Move existing commands/error mapping to `ipc.rs` as `PublicIpcError`, preserving current auth codes. Add exact commands:

```text
get_settings()
set_auto_lock_seconds(seconds)
set_clipboard_clear_seconds(seconds)
set_theme(theme)
set_launch_at_startup(enabled)
record_activity()
change_master_password(currentPassword, newPassword)
reset_keynest(confirmation)
reset_keynest_authenticated(currentPassword, confirmation)
open_keynest_data_folder()
```

`get_settings` merges stored values/warning with actual startup state. Mutations require unlocked auth. Persist before updating running timeout services. Folder opening accepts no path, creates the fixed app-data directory if missing, and calls `tauri_plugin_opener::open_path(app_data_dir, None::<&str>)`.

Authenticated reset order is: validate password/phrase without deletion, disable autostart, clear owned clipboard, reset settings, call `AuthService::finish_reset()` to delete profile/vault, disarm timer, return setup-required. Recovery reset omits password validation but uses the remaining order. If settings cleanup fails, encrypted profile/vault data remains; if final encrypted-data deletion fails, settings have already fallen back to secure defaults and the backend remains non-setup so the user can retry.

- [ ] **Step 6: Restrict capabilities**

Add only:

```json
"autostart:allow-enable",
"autostart:allow-disable",
"autostart:allow-is-enabled"
```

Do not add clipboard permissions.

- [ ] **Step 7: Verify and commit**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
npm.cmd run build
git add src-tauri/src/platform src-tauri/src/ipc.rs src-tauri/src/settings src-tauri/src/security src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/capabilities/default.json package.json package-lock.json
git commit -m "feat: expose enforced settings commands"
```

Expected: all checks pass and `src-tauri/tauri.conf.json` remains unstaged.

## Task 7: Frontend Settings Client and Theme Bootstrap

**Files:**
- Create: `src/features/settings/types.ts`
- Create: `src/features/settings/settingsClient.ts`
- Create: `src/features/settings/settingsClient.test.ts`
- Create: `src/features/settings/SettingsProvider.tsx`
- Create: `src/features/settings/SettingsProvider.test.tsx`
- Modify: `src/app/App.tsx`
- Modify: `src/App.css`

**Interfaces:**
- Consumes: settings IPC and `matchMedia('(prefers-color-scheme: dark)')`.
- Produces: `SettingsProvider`, `useSettings`, typed client, confirmed mutation helpers, and root `data-theme`.

- [ ] **Step 1: Write failing client tests and exact types**

```ts
export type AutoLockSeconds = 60 | 300 | 900 | 1800;
export type ClipboardClearSeconds = 10 | 30 | 60;
export type ThemePreference = "system" | "dark" | "light";
export type SettingsSnapshot = {
  autoLockSeconds: AutoLockSeconds;
  clipboardClearSeconds: ClipboardClearSeconds;
  theme: ThemePreference;
  launchAtStartup: boolean;
  warning?: string;
};
```

```ts
await settingsClient.getSettings();
await settingsClient.setAutoLockSeconds(900);
await settingsClient.setClipboardClearSeconds(60);
await settingsClient.setTheme("light");
await settingsClient.setLaunchAtStartup(true);
await settingsClient.recordActivity();
await settingsClient.openDataFolder();
expect(invokeMock.mock.calls).toEqual([
  ["get_settings"],
  ["set_auto_lock_seconds", { seconds: 900 }],
  ["set_clipboard_clear_seconds", { seconds: 60 }],
  ["set_theme", { theme: "light" }],
  ["set_launch_at_startup", { enabled: true }],
  ["record_activity"],
  ["open_keynest_data_folder"],
]);
```

- [ ] **Step 2: Write failing provider/theme tests**

Prove children wait for initial settings, System follows mocked Windows dark/light changes, forced themes ignore media changes, state updates only after backend success, rejection retains prior state, and `resetToDefaults` restores `300`, `30`, `system`, `false`. Prove a backend damage warning is retained in context for the authenticated shell to display.

- [ ] **Step 3: Confirm failure**

```powershell
npm.cmd test -- src/features/settings/settingsClient.test.ts src/features/settings/SettingsProvider.test.tsx
```

Expected: FAIL because the frontend module is missing.

- [ ] **Step 4: Implement typed client/provider**

Normalize records into `SettingsClientError`. Mutations return full snapshots; activity/folder return void. The context exposes:

```ts
type SettingsContextValue = {
  settings: SettingsSnapshot;
  setAutoLockSeconds(value: AutoLockSeconds): Promise<void>;
  setClipboardClearSeconds(value: ClipboardClearSeconds): Promise<void>;
  setTheme(value: ThemePreference): Promise<void>;
  setLaunchAtStartup(enabled: boolean): Promise<void>;
  resetToDefaults(): void;
  reload(): Promise<void>;
};
```

Before load, render the branded checking layout. Failure uses secure defaults plus `KeyNest could not load saved preferences. Secure defaults are active.` Resolve System with `matchMedia`, set root `data-theme` to only dark/light, set `colorScheme`, and clean up listeners.

- [ ] **Step 5: Add initial palette tokens and wrap App**

```css
:root,
:root[data-theme="dark"] {
  color-scheme: dark;
  --kn-bg: #080d0b;
  --kn-surface: #0d1511;
  --kn-titlebar: #1b1d1c;
  --kn-text: #f1fff7;
  --kn-muted: #8fa39a;
  --kn-accent: #54f5ae;
  --kn-accent-contrast: #07100c;
  --kn-danger: #ee8b8b;
  --kn-warning: #f0ca87;
}

:root[data-theme="light"] {
  color-scheme: light;
  --kn-bg: #f4faf6;
  --kn-surface: #ffffff;
  --kn-titlebar: #e8f0eb;
  --kn-text: #102119;
  --kn-muted: #587064;
  --kn-accent: #087a4b;
  --kn-accent-contrast: #ffffff;
  --kn-danger: #a83434;
  --kn-warning: #805b10;
}
```

Use root text/background tokens now; Task 11 completes conversion.

- [ ] **Step 6: Verify and commit**

```powershell
npm.cmd test -- src/features/settings
npm.cmd run build
git add src/features/settings src/app/App.tsx src/App.css
git commit -m "feat: bootstrap persisted KeyNest themes"
```

Expected: client/provider tests and build pass.

## Task 8: Authenticated Shell and Settings Navigation

**Files:**
- Create: `src/shared/components/AuthenticatedShell.tsx`
- Create: `src/shared/components/AuthenticatedShell.test.tsx`
- Create: `src/pages/SettingsPage.tsx`
- Create: `src/pages/SettingsPage.test.tsx`
- Modify: `src/shared/components/NavigationSidebar.tsx`
- Modify: `src/pages/HomePage.tsx`
- Modify: `src/pages/HomePage.test.tsx`
- Modify: `src/app/App.tsx`
- Modify: `src/app/App.test.tsx`

**Interfaces:**
- Consumes: title bar, sidebar, Home content, and AuthGate lock callback.
- Produces: `AuthenticatedDestination = "home" | "settings"`, shared shell, active navigation, and category tabs.

- [ ] **Step 1: Write failing shell tests**

```ts
const user = userEvent.setup();
render(
  <AuthenticatedShell
    onLockKeynest={vi.fn().mockResolvedValue(undefined)}
    onOpenPasswordVault={vi.fn()}
  />,
);
expect(screen.getByRole("heading", { name: /secure nest/i })).toBeInTheDocument();
await user.click(screen.getByRole("button", { name: "Open navigation" }));
await user.click(screen.getByRole("button", { name: "Settings" }));
expect(screen.getByRole("heading", { name: "Settings" })).toBeInTheDocument();
expect(screen.getByRole("button", { name: "Settings" })).toHaveClass("active");
expect(screen.getByRole("complementary")).toHaveAttribute("aria-hidden", "true");
```

Also test Home return, Escape/backdrop close, lock callback, Password Vault callback, and a non-blocking settings warning banner when `settings.warning` is present.

- [ ] **Step 2: Write failing category tests**

Test Security, General, Appearance, and About buttons. Security starts selected; clicking About updates `aria-selected` and shows About panel. Use real category headings/copy while later tasks insert controls.

- [ ] **Step 3: Confirm failure**

```powershell
npm.cmd test -- src/shared/components/AuthenticatedShell.test.tsx src/pages/SettingsPage.test.tsx src/app/App.test.tsx
```

Expected: FAIL because navigation is not wired.

- [ ] **Step 4: Extract authenticated chrome**

Move sidebar state, Escape listener, title bar, sidebar, and backdrop from Home into `AuthenticatedShell`. Home accepts only `onOpenPasswordVault` and renders topbar/main/footer. Read `settings.warning` through `useSettings()` and render it once as a safe `role="status"` banner above authenticated page content; never include a raw backend error or filesystem path.

Use exact sidebar props:

```ts
type NavigationSidebarProps = {
  isOpen: boolean;
  activeDestination: "home" | "settings";
  onClose(): void;
  onNavigate(destination: "home" | "settings"): void;
  onOpenPasswordVault(): void;
  onLockKeynest(): Promise<void>;
};
```

Home/Settings navigate, close, and apply active state only when selected.

- [ ] **Step 5: Implement dedicated Settings structure**

Own `SettingsCategory = "security" | "general" | "appearance" | "about"`. Render `<nav aria-label="Settings categories">`, buttons with `role="tab"`, `aria-selected`, and `aria-controls`, plus one matching `role="tabpanel"`.

- [ ] **Step 6: Compose App, verify, and commit**

Inside unlocked AuthGate content render the shell, preserving Password Vault alert and manual lock. Update App integration tests for Settings/Home transitions.

```powershell
npm.cmd test -- src/shared/components src/pages src/app/App.test.tsx
npm.cmd run build
git add src/shared/components src/pages src/app/App.tsx src/app/App.test.tsx
git commit -m "feat: add dedicated settings navigation"
```

Expected: shell/category tests and build pass.

## Task 9: Security Timeouts, Lock Events, and Activity Reporting

**Files:**
- Create: `src/features/settings/ActivityReporter.tsx`
- Create: `src/features/settings/ActivityReporter.test.tsx`
- Create: `src/features/settings/components/SecuritySettings.tsx`
- Create: `src/features/settings/components/SecuritySettings.test.tsx`
- Modify: `src/features/auth/components/AuthGate.tsx`
- Modify: `src/features/auth/components/AuthGate.test.tsx`
- Modify: `src/pages/SettingsPage.tsx`
- Modify: `src/app/App.tsx`

**Interfaces:**
- Consumes: settings context, activity IPC, `keynest://locked`, and AuthGate state.
- Produces: required timeout controls, fixed sleep status, throttled reporting, and backend-triggered UI locking.

- [ ] **Step 1: Write failing exact-option tests**

```ts
expect(screen.getByRole("option", { name: "1 minute" })).toHaveValue("60");
expect(screen.getByRole("option", { name: "5 minutes" })).toHaveValue("300");
expect(screen.getByRole("option", { name: "15 minutes" })).toHaveValue("900");
expect(screen.getByRole("option", { name: "30 minutes" })).toHaveValue("1800");
expect(screen.queryByRole("option", { name: /never/i })).not.toBeInTheDocument();
```

Assert clipboard options are exactly 10, 30, 60 seconds. Prove pending mutations retain the last confirmed value; rejected mutations retain it and show inline alert.

- [ ] **Step 2: Write failing activity/event tests**

With fake timers, fire pointerdown/keydown/wheel/touchstart/focus repeatedly within five seconds and expect one `record_activity`; advance 5,000 ms and expect another. Unmount and prove no calls. Mock `listen`, invoke the captured locked callback, assert protected content is removed, and assert unlisten runs.

- [ ] **Step 3: Confirm failure**

```powershell
npm.cmd test -- src/features/settings/ActivityReporter.test.tsx src/features/settings/components/SecuritySettings.test.tsx src/features/auth/components/AuthGate.test.tsx
```

Expected: FAIL because controls/reporting/listener are missing.

- [ ] **Step 4: Implement security controls**

Use controlled selects bound to confirmed context. Disable only the saving control and render its error next to it. Render **Lock when Windows sleeps** as fixed enabled text with no toggle.

- [ ] **Step 5: Implement throttled activity**

Mount reporter only while unlocked. Register pointerdown, keydown, wheel, touchstart, and focus; use passive listeners where supported. Send immediately on first event, ignore for 5,000 ms using `lastSentAt`, and remove every listener. Ignore only unauthorized errors caused by simultaneous lock; route other safe failures to App's error banner.

- [ ] **Step 6: Listen for backend lock**

Use `listen("keynest://locked", () => { setLockError(""); setStatus("locked"); })`. If listener registration fails while authenticated, immediately call the existing Rust `lock()` command; render `data-error` if that lock cannot be confirmed. Never keep protected content mounted without a working backend-lock notification channel. Clean up the unlisten function.

- [ ] **Step 7: Verify and commit**

```powershell
npm.cmd test -- src/features/settings src/features/auth/components/AuthGate.test.tsx
npm.cmd run build
git add src/features/settings src/features/auth/components/AuthGate.tsx src/features/auth/components/AuthGate.test.tsx src/pages/SettingsPage.tsx src/app/App.tsx
git commit -m "feat: connect enforced security preferences"
```

Expected: exact options, confirmed mutations, throttling, and lock events pass.

## Task 10: Password Change and Both Reset Experiences

**Files:**
- Create: `src/features/settings/components/ChangeMasterPasswordForm.tsx`
- Create: `src/features/settings/components/ChangeMasterPasswordForm.test.tsx`
- Create: `src/features/settings/components/AuthenticatedResetDialog.tsx`
- Create: `src/features/settings/components/AuthenticatedResetDialog.test.tsx`
- Modify: `src/features/settings/components/SecuritySettings.tsx`
- Modify: `src/features/auth/authClient.ts`
- Modify: `src/features/auth/authClient.test.ts`
- Modify: `src/features/auth/components/AuthGate.tsx`
- Modify: `src/features/auth/components/AuthGate.test.tsx`
- Modify: `src/features/auth/components/ResetDialog.tsx`
- Modify: `src/features/auth/components/UnlockScreen.test.tsx`
- Modify: `src/app/App.tsx`

**Interfaces:**
- Consumes: password/reset IPC, AuthGate transition, settings reset callback, and `PasswordField`.
- Produces: safe password change, protected Settings reset, updated recovery phrase, and setup transition.

- [ ] **Step 1: Extend exact auth client tests**

```ts
await authClient.changeMasterPassword("current password value", "new password value");
await authClient.resetKeynest("RESET KEYNEST");
await authClient.resetKeynestAuthenticated("current password value", "RESET KEYNEST");
expect(invokeMock.mock.calls).toContainEqual([
  "change_master_password",
  { currentPassword: "current password value", newPassword: "new password value" },
]);
expect(invokeMock.mock.calls).toContainEqual([
  "reset_keynest_authenticated",
  { currentPassword: "current password value", confirmation: "RESET KEYNEST" },
]);
```

Change the existing recovery reset expectation to `RESET KEYNEST`.

- [ ] **Step 2: Write failing form/dialog tests**

Password tests cover empty, fewer than 12 Unicode characters, mismatch, pending disablement, bad-current message, success field clearing, success text, and no lock call. Reset tests prove both password and exact phrase are required; lowercase and `RESET` remain disabled; success clears fields, closes, resets context defaults, and shows first-time setup.

- [ ] **Step 3: Confirm failure**

```powershell
npm.cmd test -- src/features/auth/authClient.test.ts src/features/settings/components/ChangeMasterPasswordForm.test.tsx src/features/settings/components/AuthenticatedResetDialog.test.tsx src/features/auth/components/UnlockScreen.test.tsx
```

Expected: FAIL because client methods/components are missing.

- [ ] **Step 4: Implement password change**

Use Current/New/Confirm local state. Validate `Array.from(newPassword).length >= 12` and exact confirmation; Rust remains authoritative. Submit current/new only. Clear all fields after a completed request. Success text is `Master password changed. Your new password will be required the next time KeyNest locks.` Keep session unlocked.

- [ ] **Step 5: Implement reset flows**

Authenticated dialog uses `PasswordField`, phrase input, focus trap, and Escape only while idle. Extend AuthGate children with:

```ts
resetAuthenticated(
  currentPassword: string,
  confirmation: "RESET KEYNEST",
): Promise<void>;
```

After either authenticated or locked-recovery reset returns setup-required, call `onResetComplete`, then set AuthGate status. App passes `SettingsProvider.resetToDefaults`. Update locked `ResetDialog` label/gate/submission to `RESET KEYNEST`, without a password field; explain that it erases but does not unlock data.

- [ ] **Step 6: Verify and commit**

```powershell
npm.cmd test -- src/features/auth src/features/settings/components src/app/App.test.tsx
npm.cmd run build
git add src/features/auth src/features/settings/components src/app/App.tsx src/app/App.test.tsx
git commit -m "feat: add password change and protected reset"
```

Expected: password/reset tests and build pass.

## Task 11: General, Appearance, About, Themes, and Responsive Layout

**Files:**
- Create: `src/features/settings/components/GeneralSettings.tsx`
- Create: `src/features/settings/components/AppearanceSettings.tsx`
- Create: `src/features/settings/components/AboutSettings.tsx`
- Create: `src/features/settings/components/SettingsSections.test.tsx`
- Modify: `src/pages/SettingsPage.tsx`
- Modify: `src/pages/SettingsPage.test.tsx`
- Modify: `src/App.css`

**Interfaces:**
- Consumes: settings context, data-folder client, `getVersion`, BrandMark, and theme tokens.
- Produces: functional General/Appearance/About sections, complete palettes, and desktop/narrow Settings layout.

- [ ] **Step 1: Write failing section tests**

Test all of these:

- General shows actual launch-at-startup state, explains minimized/locked startup, waits for confirmation, and retains old value plus alert on failure.
- Appearance exposes exactly System, Dark, and Light radios and checks the confirmed preference.
- About renders BrandMark, mocked version `0.1.0`, local-device/no-recovery copy, and calls `open_keynest_data_folder` without arguments.
- Category tabs support ArrowLeft/ArrowRight roving keyboard navigation and visible focus.

- [ ] **Step 2: Confirm failure**

```powershell
npm.cmd test -- src/features/settings/components/SettingsSections.test.tsx src/pages/SettingsPage.test.tsx
```

Expected: FAIL because sections are missing.

- [ ] **Step 3: Implement the three sections**

General uses controlled checkbox plus confirmed `setLaunchAtStartup`. Appearance uses labeled radio group plus confirmed `setTheme`. About calls `getVersion()` on mount, shows `Version unavailable` on failure, and invokes only `settingsClient.openDataFolder()`.

Use exact privacy copy:

```text
Your encrypted KeyNest data stays on this device. KeyNest has no account service and cannot recover a forgotten master password.
```

- [ ] **Step 4: Complete theme token conversion**

Replace hard-coded dark text/background/border pairs throughout title bar, sidebar, auth screens, Home, dialogs, banners, inputs, buttons, cards, footer, and Settings with named variables. Add:

```css
:root,
:root[data-theme="dark"] {
  --kn-surface-raised: #111815;
  --kn-border: rgba(255, 255, 255, 0.09);
  --kn-border-strong: rgba(84, 245, 174, 0.28);
  --kn-overlay: rgba(0, 0, 0, 0.76);
  --kn-accent-soft: rgba(84, 245, 174, 0.09);
  --kn-danger-soft: rgba(238, 139, 139, 0.08);
  --kn-warning-soft: rgba(239, 182, 85, 0.08);
}

:root[data-theme="light"] {
  --kn-surface-raised: #ffffff;
  --kn-border: rgba(16, 33, 25, 0.13);
  --kn-border-strong: rgba(8, 122, 75, 0.35);
  --kn-overlay: rgba(16, 33, 25, 0.46);
  --kn-accent-soft: rgba(8, 122, 75, 0.09);
  --kn-danger-soft: rgba(168, 52, 52, 0.08);
  --kn-warning-soft: rgba(128, 91, 16, 0.09);
}
```

Focus outlines remain at least 2 px and use `--kn-accent`.

- [ ] **Step 5: Implement the approved responsive layout**

```css
.settings-page {
  min-height: calc(100vh - 40px);
  display: grid;
  grid-template-columns: 210px minmax(0, 720px);
  justify-content: center;
  gap: 32px;
  padding: 48px 32px;
  background: var(--kn-bg);
}

.settings-category-nav {
  align-self: start;
  position: sticky;
  top: 32px;
}

.settings-panel {
  min-width: 0;
  color: var(--kn-text);
}
```

At 760 px or narrower, switch to one column, make category nav horizontally scrollable, and keep tabs at least 44 px high. At heights of 760 px or less, reduce vertical padding to 24 px. Authenticated content scrolls while the title bar remains fixed.

- [ ] **Step 6: Verify and commit**

```powershell
npm.cmd test -- src/features/settings src/pages/SettingsPage.test.tsx src/shared/components/AuthenticatedShell.test.tsx
npm.cmd test
npm.cmd run build
git add src/features/settings/components src/pages/SettingsPage.tsx src/pages/SettingsPage.test.tsx src/App.css
git commit -m "feat: complete responsive settings experience"
```

Expected: focused/full frontend tests and production build pass.

## Task 12: Full Verification and Windows Manual QA

**Files:**
- Modify only when verification exposes a settings-scoped defect: files named in Tasks 1-11
- Do not modify: `src-tauri/tauri.conf.json`

**Interfaces:**
- Consumes: the complete settings feature.
- Produces: passing automated evidence and Windows manual-QA results.

- [ ] **Step 1: Check Rust formatting**

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

Expected: exits 0. If it reports differences, run `cargo fmt --manifest-path src-tauri/Cargo.toml`, inspect the diff, and retain only settings-scoped formatting.

- [ ] **Step 2: Run the full matrix**

```powershell
npm.cmd test
npm.cmd run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected: every command exits 0 and no whitespace errors appear.

- [ ] **Step 3: Audit secrets and forbidden values**

```powershell
rg -n -i "never|password.*log|console\.log|println!|clipboard.*console" src src-tauri/src
rg -n "60|300|900|1800|10|30" src/features/settings src-tauri/src/settings
```

Expected: no Never option exists; ordinary explanatory uses of “never” are reviewed manually; no password/clipboard logging exists; allowed constants match the approved sets.

- [ ] **Step 4: Verify workspace isolation**

```powershell
git status --short
git diff -- src-tauri/tauri.conf.json
git log --oneline -12
```

Expected: the user's 1000 x 700 change remains uncommitted, `.superpowers/` remains untracked, and feature commits contain neither.

- [ ] **Step 5: Run Windows/Tauri smoke checks**

```powershell
npm.cmd run tauri dev
```

Verify in order:

1. Unlock and navigate Home -> Settings -> all four categories at 1000 x 700 and a larger size.
2. Select Light, Dark, System; inspect setup/unlock, Home, Settings, error, and reset states for contrast/focus.
3. Select each auto-lock option; use 1 minute, interact once before expiry, then stop and confirm Unlock replaces Settings.
4. Sleep Windows while unlocked; resume and confirm Welcome back is the first protected state.
5. Change master password; remain unlocked, then lock, reject old password, and accept new password.
6. Exercise the clipboard service with a controlled backend test value; verify unchanged value clears and newer user value remains.
7. Enable startup; confirm actual toggle, launch with `--autostart`, restore minimized taskbar window to Welcome back, then disable startup.
8. Open the data folder and confirm it is the fixed KeyNest application-data directory.
9. Reject reset with wrong password/phrase, cancel with Escape, then reset and confirm setup plus System/5-minute/30-second defaults.
10. Confirm no unrelated local file is removed and no secret appears in terminal/developer-console output.

- [ ] **Step 6: Fix only evidence-backed defects**

For every defect: add a failing regression test in the owning test file, run it to observe failure, make the smallest correction, rerun the focused test, then rerun Step 2.

- [ ] **Step 7: Commit corrections only if necessary**

```powershell
git add src src-tauri/src src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/capabilities/default.json package.json package-lock.json
git commit -m "fix: resolve settings verification findings"
```

If verification changed nothing, do not create an empty commit. Record all automated and manual results in the final handoff.
