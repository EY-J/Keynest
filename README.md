# KeyNest

KeyNest is a local-first Tauri application for keeping private information on a Windows PC or laptop. Its master-password boundary is implemented in Rust so the React interface cannot unlock protected backend data by itself.

## Local development

Requirements: Node.js, npm, Rust, and the platform prerequisites for Tauri 2.

```powershell
npm.cmd install
npm.cmd run tauri dev
```

Useful verification commands:

```powershell
npm.cmd test
npm.cmd run build
cargo test --manifest-path src-tauri\Cargo.toml
cargo build --manifest-path src-tauri\Cargo.toml
```

## Master-password behavior

- KeyNest requires the master password on every launch.
- The master password derives a key that unwraps the encrypted vault key; it is not stored on disk.
- The master password cannot be recovered.
- Reset permanently deletes KeyNest's encrypted local profile and vault.
- KeyNest protects stored data at rest but cannot protect an unlocked device from malware, keyloggers, or an administrator inspecting process memory.
