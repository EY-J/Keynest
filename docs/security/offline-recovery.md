# Offline Recovery Key verification (roadmap Phase 3)

Phase 3's architecture already exists. This phase adds regression evidence rather
than another recovery implementation or a profile migration.

## Existing architecture

- Rust `generate_recovery_key` reads 16 bytes from `OsEntropy` (`getrandom`) and
  encodes all 128 bits as a versioned `KN-R1-...` key. No password-derived,
  administrative, universal, email, or remote recovery credential exists.
- Setup wraps the same random Vault Key twice: once under the Master Password,
  once under the independent Recovery Key. Both wrappers get their own salt and
  nonce. AAD domains are `keynest-profile-v1` and `keynest-recovery-wrap-v1`.
- Recovery unwraps using the Recovery Key, makes a new master wrapper, generates
  a replacement Recovery Key and wrapper, and commits one replacement profile
  using `AtomicWriteFile`. The service changes its authoritative unlocked state
  and returns the replacement key only after persistence succeeds.
- Credential records are not re-encrypted. Recovery does not call the vault
  storage service; its only persistent replacement is `profile.json`.
- Setup/recovery/regeneration commands return a newly generated key for the
  temporary UI. The status command returns only `configured`. There is no command
  that retrieves a previously generated key from storage.
- `RecoveryKeyScreen` shows the key and requires a final-group acknowledgement.
  The setup, recovery, and settings parents clear the key when that flow finishes.
  Copy is an explicit button action through `copy_recovery_key`, not an automatic
  side effect of generating/displaying a key. No remote transmission is involved.
- Legacy v1 profiles without recovery remain readable and unlockable; settings
  displays `Not configured`. Enabling recovery requires the current password.

## Regression evidence

| Requirement | Evidence |
| --- | --- |
| Setup creates two independent wrappers for the same Vault Key | Auth setup test checks key equality, different salt/nonce, and no plaintext recovery key in profile bytes |
| At least 128 bits and explicit key version | Crypto generation/normalization tests and inspected OS entropy implementation |
| Domains cannot be interchanged | Crypto test attempts both master-to-recovery and recovery-to-master unwrap using the same secret |
| Wrong/tampered wrapper rejected | Crypto tests mutate salt, nonce, and ciphertext independently; command/auth tests verify unchanged profiles after wrong keys |
| Legacy installations still work | Legacy storage/auth tests prove load/unlock and unconfigured status |
| Vault Key and real records survive recovery | Command-level integration test creates an encrypted credential, recovers after service restart, compares vault bytes, and decrypts the original record |
| Old key invalidated; new key works | Integration test restarts again, rejects the old key, accepts the replacement, and verifies a second rotation |
| Failed commit retains a working recovery route | Injected replacement failure leaves the profile unchanged and locked; both old password and original recovery key still work |
| No plaintext persistence or automatic clipboard copy | Integration test scans profile/vault bytes for fixture secrets and verifies unrelated clipboard text remains unchanged |

New or expanded tests only; production behavior, IPC contracts, encryption, and
frontend presentation are unchanged in this phase. Run `cargo fmt --check`,
`cargo clippy --all-targets --all-features`, and `cargo test` from `src-tauri`.

## Limitations

The Recovery Key is a bearer secret: anyone possessing it and the encrypted
profile can recover that profile. Store it separately. Rotation invalidates an
old key for the current profile, not for an old copied profile/backup that still
contains the old wrapper. This is not rollback protection.

The readable key necessarily exists temporarily in UI/process memory while being
displayed. A crash or dismissal before saving a committed replacement can lose
that displayed key; it cannot be retrieved later. A user who can still unlock
with the new Master Password can generate another Recovery Key. Losing both
credentials has no backdoor recovery. No protection against compromised-system
malware or screen cameras is claimed.

Tests use temporary profiles, deterministic fixture entropy where appropriate,
and a fake clipboard; they do not modify a live vault. Frontend display/copy
behavior was verified by code inspection, not a manual desktop UI session.
