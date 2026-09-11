# KeyNest

KeyNest is a local-first desktop application focused on privacy, security, and personal digital organization.

## About KeyNest

KeyNest is an evolving application designed to help people organize and access important personal information while keeping control of their data. Its development is guided by:

- Privacy
- Local data ownership
- Security
- Clear organization
- Ease of access

The features available today are part of that broader direction rather than the project's final scope.

## Current Capabilities

KeyNest currently includes:

- Encrypted local credential management with search, tags, favorites, and password generation
- Master-password authentication, recovery-key support, manual locking, and automatic locking
- Configurable clipboard clearing for copied credentials
- Local profile, appearance, startup, and security settings
- Browser autofill integration through the included extension and native host

## Current Development

KeyNest is under active development. Its capabilities and organization will continue to evolve as the project grows. This README describes the functionality currently implemented in the repository and does not treat planned work as complete.

## Technology

KeyNest currently uses:

- React and TypeScript for the user interface
- Vite for frontend development and builds
- Tauri for the desktop application framework
- Rust for native application logic and security-sensitive operations
- SQLite for local credential storage

## Local-First Approach

KeyNest is designed to keep application data and core functionality on the user's device where practical. Credential records are encrypted before being stored locally, and authentication and data access are handled by the desktop application rather than a hosted KeyNest service.

Local-first software still depends on the security of the device it runs on. KeyNest helps protect stored data at rest, but it cannot protect an unlocked device from threats such as malware, keyloggers, or an administrator inspecting process memory.

## Development

### Requirements

- Node.js and npm
- Rust
- The platform prerequisites for Tauri

Install dependencies and start the desktop application in development mode:

```powershell
npm.cmd install
npm.cmd run tauri -- dev
```

Other available project commands:

```powershell
npm.cmd test
npm.cmd run build
npm.cmd run preview
```

## Project Structure

```text
KeyNest/
|-- browser-extension/  # Browser autofill extension
|-- docs/               # Technical and security documentation
|-- public/             # Static frontend assets
|-- scripts/            # Development and release utilities
|-- src/                # React and TypeScript application
|-- src-tauri/          # Tauri and Rust desktop backend
|-- tests/              # Frontend and integration tests
|-- package.json        # npm scripts and dependencies
`-- README.md           # Project overview
```
