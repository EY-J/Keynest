# KeyNest

KeyNest is a local-first personal desktop application for Windows that brings useful digital tools, information, and everyday organization into one place.

## About KeyNest

KeyNest is an evolving personal space on the user's computer. It is designed to make frequently used information and tools easy to organize and access while keeping core functionality primarily on the device.

The Vault is one KeyNest feature, not the identity of the whole product. Notes is another current workspace, and future additions may extend beyond privacy- or security-focused tools. The final scope is intentionally open so KeyNest can grow into a broader personal desktop application without being locked into a single category.

## Current Capabilities

The capabilities below are implemented in the current repository.

### Vault

The Vault provides local credential management with:

- Encrypted credential storage
- Create, edit, reveal, copy, and delete workflows
- Search and tags
- Favorite credentials and a Favorites view
- Password generation
- Optional website associations

### Notes

The built-in Notes workspace supports:

- Creating and editing notes
- Encrypted local note storage
- Search and tags
- Favorite notes
- Moving deleted notes to Recently Deleted

### Recently Deleted

Recently Deleted is a shared recovery area for deleted credentials and notes. It supports restoring individual items, permanently deleting selected items, emptying the recovery area, and automatically removing items after 30 days.

### Account and Application Controls

KeyNest currently includes:

- Local Master Password setup, unlock, and password changes
- Offline Recovery Key creation, regeneration, and password recovery
- Manual locking and inactivity-based auto-lock
- Optional locking when Windows sleeps
- Configurable clearing of credential data copied through KeyNest
- Local display-name and profile-photo customization
- System, light, and dark appearance settings
- Optional launch at Windows startup
- An authenticated reset workflow for local KeyNest data

### Browser Autofill

The included Manifest V3 browser extension supports explicit, user-initiated credential filling in Chrome and Edge on Windows. It uses Chromium Native Messaging and a local Windows native host to communicate with a running, unlocked KeyNest desktop application.

Autofill matches approved HTTPS hosts and does not depend on a hosted KeyNest backend. It does not automatically submit forms, and the extension requires separate local setup and browser registration. See [browser-extension/README.md](browser-extension/README.md) for supported behavior, setup, and limitations.

## Direction

KeyNest is intentionally broader than a password manager or vault. Its current features are starting points for a personal desktop environment, and future tools may address organization, information, convenience, or other everyday needs.

Development is active, and no fixed final feature set is being claimed. Private Files currently appears in the interface as **Soon** and is not part of the implemented capabilities described above.

## Design Philosophy

KeyNest is guided by a few practical ideas:

- A personal desktop experience with simple, consistent organization
- Local ownership of application data
- Core functionality that remains useful without a remote account
- Privacy and security as foundations for the application
- Fast access to everyday tools and information
- Room to evolve without narrowing the product to one category

Core KeyNest functionality is designed to remain local-first and does not depend on a hosted KeyNest backend. This does not mean that every possible future feature must avoid the internet; optional integrations may be considered where they fit the product.

## Privacy and Security

Credential records and note contents are encrypted before being stored in local SQLite-backed application files. Master Password authentication, Recovery Key operations, protected data access, and credential clipboard operations are handled by the desktop application.

The local-first design reduces dependence on remote services, but it does not remove device-level risk. KeyNest cannot protect an already unlocked or compromised computer from malware, keyloggers, privileged operating-system users, process-memory inspection, or similar threats. Users are still responsible for securing their Windows account, device, backups, Master Password, and Recovery Key.

## Technology

KeyNest currently uses:

- React and TypeScript for the user interface
- CSS for application styling
- Vite for frontend development and builds
- Tauri for the Windows desktop application
- Rust for native logic and security-sensitive operations
- SQLite through Rust's `rusqlite` library for local Vault and Notes storage
- A JavaScript Manifest V3 extension and native host for browser autofill

## Application Architecture

The main desktop data flow is:

```text
User
  |
  v
React + TypeScript UI
  |
  v
Tauri IPC
  |
  v
Rust Backend
  |
  v
Local Application Storage
```

Browser autofill follows a separate local path:

```text
Website in Chrome or Edge
  |
  v
KeyNest Browser Extension
  |
  v
Chromium Native Messaging
  |
  v
KeyNest Native Host
  |
  v
Running KeyNest Desktop App
  |
  v
Vault
```

There is no hosted KeyNest service between the browser extension and the desktop application.

## Development

### Requirements

- Windows
- Node.js and npm
- Rust and Cargo
- The [Tauri prerequisites for Windows](https://v2.tauri.app/start/prerequisites/)

Install dependencies:

```powershell
npm.cmd install
```

Start the Tauri desktop application in development mode:

```powershell
npm.cmd run tauri -- dev
```

Available validation and frontend commands:

```powershell
npm.cmd test
npm.cmd run build
npm.cmd run preview
```

`preview` serves the built frontend for browser inspection; desktop capabilities still require Tauri. Browser autofill also requires the native host and extension registration documented in [docs/release/browser-autofill-dev-setup.md](docs/release/browser-autofill-dev-setup.md).

## Project Structure

```text
KeyNest/
|-- browser-extension/       # Chrome and Edge autofill extension
|   `-- src/                 # Extension popup, worker, and fill logic
|-- docs/                    # Architecture, security, and release notes
|-- public/                  # Static frontend assets and licenses
|-- scripts/                 # Autofill, development, and release utilities
|-- src/                     # React and TypeScript frontend
|   |-- app/                 # Application shell and navigation
|   |-- assets/              # Fonts, branding, and site icons
|   |-- components/          # Shared UI components and effects
|   |-- features/            # Auth, Vault, Notes, settings, and other features
|   |-- hooks/               # Shared React hooks
|   |-- shared/              # Shared frontend behavior
|   |-- styles/              # Global CSS
|   `-- utils/               # Frontend utilities
|-- src-tauri/               # Tauri configuration and Rust backend
|   |-- capabilities/        # Tauri permission configuration
|   `-- src/                 # IPC, security, storage, autofill, Vault, and Notes
|-- tests/                   # Frontend, policy, and integration tests
|-- package.json             # npm scripts and frontend dependencies
`-- vite.config.ts           # Vite configuration
```

## Project Status

KeyNest is under active development. Features, interfaces, storage formats, and project structure may change as the application evolves.

This README describes the present implementation rather than a final product boundary. The project is intentionally broader than the Vault, and future additions may include different kinds of personal desktop tools while preserving the local-first approach for core functionality.
