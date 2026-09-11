# Shared modal system

## Popup audit

| Popup / state | Owner | Backdrop / Escape |
| --- | --- | --- |
| Change Master Password | ChangeMasterPasswordForm | Both allowed unless submitting/closing |
| Forgot Master Password choices; Recover with Recovery Key | RecoverPasswordDialog | No backdrop; Escape unless submitting |
| Locked/data-error destructive reset | ResetDialog | No backdrop; Escape unless submitting |
| Authenticated destructive reset | AuthenticatedResetDialog | No backdrop; Escape unless submitting |
| Recovery management, creation/regeneration, replacement-key display | RecoverySettings | No backdrop; Escape only before replacement-key display and when not submitting |
| Add Credential | VaultPage / VaultModal | Both allowed unless saving/closing |
| View / Reveal / Edit / Delete Credential | VaultRecordDialog / VaultModal | Both allowed unless pending/closing |

Edit/delete/reveal are states of the existing credential dialog, not separate overlays. Recovery choice/form and regeneration/key acknowledgement likewise remain in one dialog. Setup/post-recovery RecoveryKeyScreen is a full-page auth screen, not a popup; unchanged. Navigation sidebar, inline errors, password generator and native select menus are not application modals; unchanged. No other application popup was found by the final dialog/modal/overlay/backdrop audit.

## Shared implementation

`src/components/ui/Modal/Modal.tsx` contains the native dialog wrapper, delayed-close hook and Lucide close button. `modal.css` owns all modal positioning, backdrop and animation tokens. VaultModal is now only a sizing/prop adapter. Native `showModal()` supplies the top layer and background inertness without a portal or arbitrary z-index; all focus trapping and opener/fallback restoration is centralized.

- Entry: 280ms, cubic-bezier(0.16, 1, 0.3, 1), opacity plus -18px/0.97 to neutral.
- Exit: 180ms, cubic-bezier(0.4, 0, 1, 1), opacity plus -10px/0.98.
- Backdrop: rgba(0,0,0,0.35), 2px blur, 200ms entrance / 160ms exit. Full-window native backdrop; no separate interactive title-bar hole.
- Reduced motion: 10ms opacity only, no modal translation/scale or button motion.
- Position: top center, approximately title-bar height plus 11vh (77px below a 40px bar at 1000x700). Short windows use 56px top. Widths remain 440px reset, 520px ordinary form/recovery, 620px vault; constrained to viewport gutters. Height is constrained with internal scrolling.
- Native dialog opens first, then a one-shot timer focuses the preferred/first meaningful control with preventScroll. Tab/Shift+Tab wrap; Escape never invokes browser default immediate close. Focus returns after dismissal if opener/fallback is still connected.
- Scroll locking is reference-counted; prior root overflow is restored. Settings content/category navigation, page and body scrolling are locked through a temporary class while dialog content remains scrollable.
- Close requests retain the owner's component/state through the exit. Closing content becomes inert. Surface animationend finishes it; a CSS-duration-derived timer covers cancelled/missing events and reduced motion. Backdrop events cannot finish early. Pointer-down and click must both be outside before backdrop dismissal.
- Pending dismissals remain blocked. One-time recovery display cannot be dismissed accidentally. No shared store contains form values. Existing post-request clearing stays in place; remaining state clears at ordinary close. Forced auth lock/reset/navigation teardown remains immediate and cancels delayed actions: security teardown must never wait for decoration.

Removed Change Master Password's separate animation/focus/timer code and keyframes, reset/vault backdrop positioning and blur rules, repeated Escape/focus traps, and the old text-glyph vault close button. Dialog-specific content layout, theme tokens, warnings, labels, validation, API calls and confirmation requirements remain. Rust, Tauri, auth gates, encryption, clipboard, routing and settings structure were not modified by this task.

## Changed files

- Shared modal: `src/components/ui/Modal/{Modal.tsx,modal.css}` with coverage in `tests/{modal.test.mjs,modalIntegration.test.mjs}`.
- Consumers: Settings, Auth/Recovery, Vault, and host-approval components use the shared modal; page-level rules remain in `src/styles/globals.css`.
- Updated test ports: `tests/{changeMasterPassword.test.mjs,componentHarness.mjs,vaultFixture.mjs}`.

## Validation

`npm.cmd run build` passed. `node --experimental-vm-modules --test tests/*.test.mjs`: 51 passed. Existing Vite large-chunk and Node experimental-VM warnings remain. Tests exercise real shared component handlers with native-dialog/hook ports; they do not prove actual WebView focus, rendering or inertness.

Manual verification is still pending for **every row above**: the browser skill connection and its troubleshooting call failed before execution (missing sandboxPolicy metadata). No standalone automation workaround was used, and no real credentials or user vault were accessed. In a debug Tauri build with disposable fixtures, check entry/top positioning, exit before disappearance, backdrop rules, Tab/Shift+Tab/Escape, opener restoration, background focus/scroll exclusion, internal scrolling, cleanup/reopen, pending safeguards and reduced motion at 1000x700 and narrow/short windows. Include recovery-to-reset transitions, recovery acknowledgement, successful create/edit/delete, lock during an open modal and error retry. No claim of completed visual/native acceptance.
