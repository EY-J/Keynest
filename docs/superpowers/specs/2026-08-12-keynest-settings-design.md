# KeyNest Settings and Enforced Security Preferences Design

**Date:** 2026-08-12
**Status:** Approved design awaiting written-spec review

## Objective

Add a dedicated Settings experience to the unlocked KeyNest application and make its security preferences functional. Auto-locking and clipboard clearing must always remain enabled, master-password changes must preserve the existing vault key safely, and settings that affect security must be validated and enforced by the Rust backend.

## Scope

This feature includes:

- A dedicated, responsive Settings page with Security, General, Appearance, and About categories.
- Required inactivity auto-locking with selectable timeouts.
- Required clipboard clearing with selectable timeouts and ownership-safe clearing.
- Immediate locking on application resume after Windows sleep.
- Changing the master password by atomically re-wrapping the existing vault key.
- Optional Windows startup registration that launches KeyNest minimized and locked.
- Complete System, Dark, and Light themes.
- Version, privacy, and local data-location information.
- A strengthened in-app Danger Zone reset flow.
- Rust, React, build, and Windows/Tauri verification appropriate to the feature.

This feature does not add password-vault records, backup or restore, password recovery, cloud synchronization, Windows Hello, a system-tray process, analytics, or automatic updates. Clipboard security infrastructure is implemented now, but no placeholder copy action is added while vault records do not yet exist. Future commands that copy protected fields must use that infrastructure.

## Confirmed Product Decisions

- Settings is a dedicated page, not a modal.
- Settings uses a category list on the left and focused content on the right at the 1000 x 700 default/minimum window size.
- Security-sensitive settings are owned and enforced by Rust; React owns presentation and reports user activity.
- Auto-lock is mandatory. The default is 5 minutes and the allowed choices are 1, 5, 15, and 30 minutes. There is no **Never** option.
- KeyNest locks on the first application event after Windows resumes from sleep, before protected UI is usable.
- Clipboard clearing is mandatory. The default is 30 seconds and the allowed choices are 10, 30, and 60 seconds. There is no **Never** option.
- Changing the master password requires the current password, the new password, confirmation, and a minimum of 12 characters.
- A successful password change keeps the current session unlocked. The new password is required after the next lock.
- Launch at startup is optional and disabled by default. An autostart launch is minimized and locked.
- Theme choices are System, Dark, and Light. System is the default and follows Windows.
- The unlocked Settings Danger Zone requires the current password and the exact phrase `RESET KEYNEST`.
- There is no analytics or update checker in this version.

## Architecture

### Selected approach

KeyNest uses a React settings interface backed by Rust-owned security and settings services. This is preferred over frontend-only timers because the authenticated Rust state remains the authority and can lock even if the React activity reporter stops. A separate Windows background service is out of scope because it would add installation and lifecycle complexity without improving the current local-app threat model enough to justify it.

The high-level flow is:

```text
React Settings / activity listeners
                 |
                 | narrow Tauri commands
                 v
Rust settings, authentication, auto-lock, and clipboard services
                 |
                 | validated local persistence and OS integrations
                 v
KeyNest app-data files / Windows startup / system clipboard
                 |
                 | lock and settings events
                 v
React removes protected screens or refreshes displayed values
```

The existing rule remains unchanged: plaintext master passwords, derived key-encryption keys, and the plaintext vault key are never persisted. The plaintext vault key never crosses the Rust-to-React IPC boundary.

### Authenticated application shell

The current Home page owns the title bar, sidebar, and home content. This feature extracts the shared authenticated chrome into an `AuthenticatedShell`. The shell owns the current top-level destination (`home` or `settings`), sidebar open state, and active navigation state. Home and Settings render only their page content inside that shell.

No routing library is required for these two local destinations. Selecting Settings closes the overlay sidebar, marks Settings active, and renders the dedicated page. Selecting Home performs the corresponding reverse transition. Manual locking remains available from the shared sidebar.

### Rust services

The backend is split into responsibilities rather than accumulating behavior in `lib.rs`:

- The existing authentication service continues to own `SetupRequired`, `Locked`, `Unlocked`, and `DataError` state.
- A settings store owns versioned, non-secret preferences and strict validation.
- An auto-lock supervisor owns the unlocked activity deadline and initiates locking.
- A clipboard service owns KeyNest-originated clipboard content and scheduled clearing.
- A startup adapter owns autostart enable, disable, and actual-state queries.

Authentication and settings services share only the narrow operations they require. Any command that mutates settings or performs a protected security action requires the Rust authentication state to be `Unlocked`. Reading non-secret settings is allowed while locked so the selected theme can apply to setup and unlock screens.

## Settings Storage and Contract

### Persisted format

Non-secret preferences are stored in a versioned `settings.json` under the same Tauri-managed per-user application-data directory as the encrypted profile. Its logical fields are:

```text
format_version
auto_lock_seconds
clipboard_clear_seconds
theme
```

Version 1 accepts only these exact values:

- `auto_lock_seconds`: `60`, `300`, `900`, or `1800`
- `clipboard_clear_seconds`: `10`, `30`, or `60`
- `theme`: `system`, `dark`, or `light`

Launch-at-startup state is read from the operating-system registration through the autostart adapter rather than duplicated as an unreliable Boolean in `settings.json`.

The settings document contains no passwords, vault keys, vault records, clipboard contents, or password-derived material. Writes use a temporary file in the same directory, flush and sync it, and atomically replace the prior valid settings file. The previous file survives a failed replacement.

Missing settings produce secure defaults. Malformed, unsupported, or invalid settings also fail closed to secure defaults and expose a safe warning to the UI; they do not change the authentication profile to `DataError` because preference damage is distinct from encrypted-profile damage. Invalid security values can never produce disabled auto-locking or disabled clipboard clearing.

### Command surface

The frontend uses a small typed command surface:

- `get_settings()` returns validated preferences plus the actual launch-at-startup state.
- `set_auto_lock_seconds(seconds)` validates, persists, applies the new deadline immediately, and returns the accepted settings.
- `set_clipboard_clear_seconds(seconds)` validates, persists, and returns the accepted settings.
- `set_theme(theme)` validates, persists, and returns the accepted settings.
- `set_launch_at_startup(enabled)` updates Windows registration and returns the actual resulting state.
- `record_activity()` reports trusted in-app keyboard, pointer, touch, wheel, or focus activity without accepting a caller-provided timestamp.
- `change_master_password(current_password, new_password)` performs the protected password-change transaction.
- `reset_keynest_authenticated(current_password, confirmation)` performs the Settings Danger Zone reset.
- `open_keynest_data_folder()` opens only KeyNest's resolved application-data directory.

Public errors use stable codes and safe messages. They distinguish invalid input, incorrect current password, unauthorized state, damaged encrypted data, settings persistence failure, startup-registration failure, and clipboard failure without exposing cryptographic details or filesystem internals.

## Security Behavior

### Required inactivity auto-lock

Unlocking or creating the master password starts a Rust-owned deadline using a monotonic clock. The default deadline is five minutes after the most recent accepted in-app activity. React installs activity listeners only while authenticated and throttles `record_activity()` calls so frequent pointer movement does not flood IPC. Rust records its own receipt time; React cannot move the deadline to an arbitrary future timestamp.

A Rust supervisor checks the deadline independently of React. When it expires, it calls the existing idempotent authentication lock operation, drops the zeroizing vault key, asks the clipboard service to clear any KeyNest-owned secret, and emits a `keynest://locked` event. `AuthGate` listens for this event and immediately removes the authenticated shell from the React tree.

Changing the timeout applies immediately against the most recent recorded activity. If selecting a shorter timeout means the deadline has already passed, KeyNest locks immediately after the setting is accepted. The timer is inactive in `SetupRequired`, `Locked`, and `DataError` states.

The design protects against a stalled or failed renderer because no further activity reports cause Rust to lock. A maliciously modified renderer or process-level attacker remains outside the existing threat model and could generate activity reports; the settings feature does not claim to defend against modified application code.

### Windows sleep and resume

The process cannot execute while Windows is asleep, so the enforceable boundary is the first lifecycle event after resume. On Tauri's `RunEvent::Resumed`, Rust locks the authentication service, clears a still-owned clipboard secret, and emits `keynest://locked` before protected interaction resumes. This matches the product promise that returning from sleep always requires the master password. The Tauri API documents `RunEvent::Resumed` as the application event sent when its event loop resumes.

### Clipboard ownership and clearing

All future protected copy operations route through the Rust clipboard service rather than writing secrets directly from arbitrary React components. The service:

1. Writes the requested secret to the system clipboard.
2. Keeps the expected value only in zeroizing memory and assigns the copy operation a generation identifier.
3. Schedules expiration using the current validated 10-, 30-, or 60-second preference.
4. At expiration, reads the current clipboard text.
5. Clears the clipboard only when it still exactly matches the expected KeyNest value and the generation remains current.
6. Zeroizes and discards its owned value whether or not the clipboard was cleared.

A later KeyNest copy supersedes an earlier generation, so an old timer cannot clear a newly copied secret. If the user copies unrelated content after KeyNest, KeyNest leaves that newer content intact. Manual lock, inactivity lock, sleep/resume lock, reset, and process shutdown all request immediate ownership-safe clearing. Clipboard reads and clears use Tauri's official clipboard-manager plugin, whose API provides `readText`, `writeText`, and `clear` operations.

Clipboard failures do not expose the secret in logs. A failed read never authorizes blindly deleting unknown clipboard content; the service discards its in-memory copy and reports a safe failure where a foreground copy action can display it. Since vault-copy UI is not yet in scope, this release verifies the service independently and integrates it with lock/reset paths.

### Change master password

The inline Security form contains Current master password, New master password, and Confirm new master password fields. React validates confirmation and the 12-character minimum for immediate feedback; Rust independently requires `Unlocked`, validates the minimum, and verifies the current password cryptographically.

Rust does not re-encrypt every vault record. It performs the safer and cheaper key-wrapping transaction:

1. Verify the current password by deriving its key-encryption key and authenticating the stored wrapped vault key.
2. Confirm that the unwrapped result matches the vault key already held by the authenticated session.
3. Generate a fresh salt and nonce.
4. Derive a new key-encryption key from the new password using the existing approved Argon2id parameters.
5. Wrap the same random vault data-encryption key with XChaCha20-Poly1305 and the existing versioned authenticated context.
6. Serialize and validate the replacement profile.
7. Flush and sync a same-directory temporary file, then atomically replace `profile.json`.
8. Only after replacement succeeds, update the in-memory profile and zeroize transient passwords and derived material.

If verification, derivation, serialization, or replacement fails, the old profile and old password remain valid. Success clears all three React password fields, shows confirmation, and leaves the existing session unlocked. The new password is proven on the next unlock.

### Destructive reset

The unlocked Settings Danger Zone requires both the current master password and the exact case-sensitive phrase `RESET KEYNEST`. Rust verifies both requirements. Before deletion, KeyNest disables its Windows autostart registration; failure to do so aborts the reset before encrypted data is removed. A successful reset clears owned clipboard content, deletes KeyNest's encrypted profile, vault file, and settings file, drops all in-memory keys, and returns to `SetupRequired` with secure default settings.

The existing forgotten-password and damaged-data recovery paths remain available because a user who has forgotten the password must still be able to erase and reuse the local application, and a damaged profile may be unverifiable. Those locked recovery paths require the exact phrase `RESET KEYNEST` but cannot require the current password. This does not weaken confidentiality because reset grants no access to encrypted data; it only destroys KeyNest-owned files. The UI explains this distinction clearly.

## General, Appearance, and About

### Launch at startup

Launch at startup is off by default. The implementation uses Tauri's official autostart plugin and grants only its enable, disable, and state-query permissions. Registration includes a dedicated autostart argument. During setup, Rust detects that argument and minimizes the main window while leaving it available on the Windows taskbar; no system tray is introduced. Every process launch, including autostart, begins locked when a profile exists.

The OS registration is the source of truth. The Settings toggle changes only after the adapter confirms the resulting state. If Windows rejects the operation, the UI retains the previous value and shows an inline error. The official Tauri plugin supports enable, disable, and `isEnabled` operations and accepts launch arguments during initialization.

### Themes

Theme values are represented by CSS custom-property tokens rather than duplicated component-specific colors. System is the default. In System mode, `prefers-color-scheme` selects the Windows preference and responds to changes while the app is running. Dark and Light force their complete palettes. The root `color-scheme` property and focus, danger, warning, disabled, and overlay colors change consistently.

Settings are loaded before an authentication screen is committed, so setup, unlock, data-error, Home, and Settings use the same selected palette without displaying protected content early. Theme changes apply immediately after successful persistence. Both palettes meet readable contrast expectations and preserve visible keyboard focus.

### About

About displays the KeyNest mark, application version from Tauri package metadata, and concise statements that encrypted data remains on this device and that KeyNest cannot recover the master password. An **Open data folder** action opens the resolved KeyNest directory rather than accepting an arbitrary frontend path. There are no analytics, update-checking, account, or cloud controls.

## Settings User Experience

### Layout and responsiveness

Settings fills the content region of the authenticated shell and is not placed inside a modal. At the configured 1000 x 700 default/minimum window, it uses a stable two-column layout: a compact category navigation column and a centered detail column. Only the detail region scrolls when its content exceeds the available height; the title bar and navigation remain stable.

At defensive narrow viewport widths, including web test environments, the category navigation becomes a horizontally scrollable tab row and the content stacks beneath it. Controls remain touch-friendly, labels stay associated with their inputs, and no critical action depends on hover. The destructive confirmation is the only Settings modal because it benefits from an explicit interruption and focus trap.

### Category contents

- **Security:** auto-lock timeout, fixed lock-on-sleep status, clipboard timeout, change-master-password form, and Danger Zone reset.
- **General:** launch-at-startup toggle and explanation that startup is minimized and locked.
- **Appearance:** System, Dark, and Light choices with immediate preview after persistence.
- **About:** logo, version, privacy/no-recovery text, and Open data folder.

Selectors and toggles persist immediately. The UI waits for Rust success before committing the displayed value; failed requests retain the last confirmed value. Forms disable duplicate submission. Errors appear beside the affected control and a polite success status confirms password changes. Password values are never placed in component error text, browser storage, URLs, or logs and are cleared after completed submissions.

## Error Handling and Failure Semantics

- Unexpected backend responses fail closed. They cannot unlock protected content or disable required protections.
- A failed settings write leaves the previous file and in-memory setting active.
- A corrupted settings file falls back to secure defaults and displays a non-blocking warning; a corrupted encrypted profile still produces the existing blocking `DataError` state.
- A failed password change keeps the previous profile and session intact and returns a safe field-level or form-level error.
- Auto-lock is idempotent, so simultaneous timeout, resume, and manual-lock requests cannot reintroduce an unlocked state.
- Startup failures retain the actual OS registration state rather than showing the requested state optimistically.
- Clipboard comparison prevents KeyNest from erasing a user's newer clipboard value.
- Reset does not begin encrypted-data deletion until password/phrase validation and autostart disablement succeed.
- No sensitive value is logged by React, Rust, or tests.

## Dependencies and Permissions

Implementation adds the official Tauri autostart and clipboard-manager plugins to the Rust and JavaScript dependencies. Capabilities grant only the operations KeyNest uses. Existing opener functionality is reused for the fixed data-folder action; the frontend never supplies an arbitrary path to that command.

Relevant primary documentation:

- [Tauri Autostart plugin](https://v2.tauri.app/plugin/autostart/)
- [Tauri Clipboard Manager API](https://v2.tauri.app/reference/javascript/clipboard-manager/)
- [Tauri `RunEvent` lifecycle documentation](https://docs.rs/tauri/latest/tauri/enum.RunEvent.html)
- [Tauri core window theme permissions](https://v2.tauri.app/reference/acl/core-permissions/)

Exact dependency versions are resolved during implementation from the project's Tauri 2-compatible lockfile rather than hard-coded in this design.

## Verification Strategy

Implementation follows test-driven development.

### Rust tests

- Missing settings load secure defaults.
- Every allowed timeout and theme round-trips through persistence.
- Unknown fields, unsupported versions, invalid timeout values, and `never`-equivalent values fall back to secure defaults.
- Atomic settings-write failure preserves the previous valid document and active values.
- Activity reports reset the monotonic deadline; inactivity locks at the selected duration.
- Shortening a timeout locks immediately when the new deadline is already elapsed.
- Repeated or simultaneous lock triggers remain idempotent.
- A simulated resume event locks and requests clipboard clearing before emitting the frontend event.
- Clipboard expiration clears matching content, preserves changed content, ignores stale generations, and clears matching content on lock.
- Password change rejects locked state, short new passwords, and incorrect current passwords.
- Successful password change keeps the vault key unchanged, makes the new password work after lock, and makes the old password fail.
- Injected replacement failure leaves the old profile/password usable and the current session unlocked.
- Authenticated reset requires both the current password and exact phrase.
- Locked recovery reset requires the exact phrase and never decrypts or exposes vault data.
- Reset is aborted if autostart disablement fails; successful reset removes only scoped KeyNest files and returns secure defaults.
- Startup adapter errors preserve the prior actual state.

Time, entropy, clipboard, startup registration, and lifecycle signals use injectable adapters in tests so tests are deterministic and do not modify the developer's real Windows settings or clipboard.

### React tests

- Sidebar navigation opens Settings, marks it active, closes the overlay, and returns Home correctly.
- All four categories render and support keyboard navigation.
- Controls initialize from confirmed backend values.
- Allowed auto-lock and clipboard options contain no Never choice.
- Immediate settings remain unchanged until the backend confirms and retain their previous value on failure.
- Auto-lock events remove authenticated content and render Unlock.
- Activity reporting is installed only while unlocked and is throttled.
- Password-change validation, submission, field clearing, success, and safe errors work as designed.
- Danger Zone requires password plus `RESET KEYNEST` before enabling reset.
- System, Dark, and Light themes apply to authentication and authenticated screens and persist across a remount.
- System-theme media-query changes update System mode but do not override forced Dark or Light.
- Startup and About controls expose correct accessible names and errors.
- The desktop and narrow responsive layouts do not hide or overflow critical controls.

### Build and manual Windows verification

- Run the complete frontend test suite.
- Run the complete Rust test suite.
- Run the TypeScript and Vite production build.
- Run Tauri development or packaged-app smoke tests at 1000 x 700 and a larger window.
- Verify each theme across setup, unlock, Home, Settings, reset confirmation, and error states.
- Enable startup, sign out/restart Windows as appropriate, and confirm KeyNest starts minimized and locked; then disable it and confirm registration is gone.
- Copy a controlled test value through the clipboard service, verify timed clearing, and verify that newer clipboard content is preserved.
- Put Windows to sleep while KeyNest is unlocked and confirm the first visible post-resume state is locked.
- Change the master password, confirm the current session remains unlocked, then lock and verify only the new password works.
- Reset from Settings and confirm autostart, encrypted local data, and preferences are removed without affecting unrelated files.

## Acceptance Criteria

- Settings is reachable from the unlocked sidebar as a dedicated, responsive page with Security, General, Appearance, and About categories.
- Rust always enforces one of the allowed inactivity timeouts; no persisted or UI state can disable auto-lock.
- Returning from Windows sleep requires the master password before protected content is usable.
- Rust always enforces one of the allowed clipboard timeouts, clears only still-owned content, and preserves newer user clipboard content.
- Changing the master password atomically re-wraps the same vault key, preserves the old profile on failure, and leaves the successful current session unlocked.
- Launch at startup is off by default, reflects actual Windows registration, and starts KeyNest minimized and locked when enabled.
- System, Dark, and Light themes work consistently across every app state and persist locally.
- About accurately displays version, local-first/no-recovery information, and the fixed KeyNest data-folder action.
- The Settings reset requires the current password plus `RESET KEYNEST`; locked recovery reset remains destructive-only and exposes no data.
- Backend and frontend tests cover security defaults, failure semantics, navigation, persistence, themes, and responsive behavior, and all production builds complete successfully.
