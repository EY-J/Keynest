# KeyNest

KeyNest is a local-first password vault for Windows. The React interface handles presentation, while Rust owns authentication, encryption, encrypted SQLite storage, clipboard protection, and automatic locking.

## Features

- Encrypted credential storage that stays on the local device
- Master-password setup, unlock, change, and authenticated reset flows
- Password generation and strength feedback
- Configurable automatic locking and clipboard clearing
- Light, dark, and system themes

## Local development

Requirements: Node.js, npm, Rust, and the platform prerequisites for Tauri 2.

```powershell
npm.cmd install
npm.cmd run tauri dev
```

## Verification

```powershell
npm.cmd run build
cargo test --manifest-path src-tauri\Cargo.toml
cargo build --manifest-path src-tauri\Cargo.toml
```

## Security model

- KeyNest requires the master password on every launch.
- The master password derives a key that unwraps the encrypted vault key; it is not stored on disk.
- The master password cannot be recovered.
- Reset permanently deletes KeyNest's encrypted local profile and vault.
- KeyNest protects stored data at rest but cannot protect an unlocked device from malware, keyloggers, or an administrator inspecting process memory.
