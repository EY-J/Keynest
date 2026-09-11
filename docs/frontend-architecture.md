# KeyNest Frontend Architecture

This guide explains where the KeyNest frontend starts, how it is organized,
and how it communicates with the secure Rust backend.

## Frontend directory map

```text
src/
├── app/
│   ├── App.tsx
│   ├── AppShell.tsx
│   └── components/
│       ├── AppTitleBar.tsx
│       └── NavigationSidebar.tsx
├── components/
│   ├── effects/
│   │   └── PixelBlast/
│   └── ui/
│       ├── Modal/
│       ├── BrandMark.tsx
│       ├── PasswordStrengthMeter.tsx
│       ├── Select.tsx
│       ├── ServiceLogo.tsx
│       └── buttons.css
├── features/
│   ├── auth/
│   │   ├── components/
│   │   ├── authClient.ts
│   │   └── types.ts
│   ├── autofill/
│   │   ├── components/
│   │   └── hostApprovalClient.ts
│   ├── home/
│   │   └── HomePage.tsx
│   ├── settings/
│   │   ├── components/
│   │   ├── SettingsPage.tsx
│   │   ├── SettingsProvider.tsx
│   │   ├── settingsClient.ts
│   │   └── types.ts
│   └── vault/
│       ├── components/
│       ├── VaultPage.tsx
│       ├── credentialFavorites.ts
│       ├── passwordGenerator.ts
│       ├── types.ts
│       └── vaultClient.ts
├── hooks/
│   └── useScrollActivity.ts
├── utils/
│   └── serviceIdentity.ts
├── shared/
│   └── security/
├── assets/
│   ├── fonts/
│   └── site-icons/
├── styles/
│   └── globals.css
└── main.tsx
```

The separate `browser-extension/` directory contains the browser Autofill UI
and Native Messaging client. It is not part of the React application bundle.

## Where the app starts

`main.tsx` is the browser entry point. It imports shared button styles, finds
the HTML element with the `root` ID, and asks React to render `App` in Strict
Mode.

`app/App.tsx` sets up app-wide behavior. It loads settings, checks the current
authentication state, reports user activity for automatic locking, and shows
the authenticated app only after the Rust backend confirms that KeyNest is
unlocked.

`app/AppShell.tsx` owns navigation after unlock. It displays the title bar and
sidebar, chooses the Home, Vault, Favorites, or Settings page, stores the local
favorites preference, and mounts the host-approval modal.

## Features and pages

Feature-specific code stays inside the feature that owns it:

- `features/vault/` contains the Vault page, credential list/card views,
  credential form, details modal, favorite preference helper, types, and
  client.
- `features/settings/` contains the Settings page, settings sections, provider,
  types, and client.
- `features/auth/` contains setup, unlock, recovery, reset, and authentication
  state components plus the authentication client.
- `features/autofill/` contains the desktop host-approval UI and its client.
- `features/home/` contains the Home page.

The Add and Edit Credential flows both use `CredentialForm.tsx`. The parent
decides whether the form starts empty or receives an existing credential.

## Reusable UI, styles, and assets

Reusable visual building blocks live in `components/ui/`. Examples include the
shared native dialog wrapper, dropdown Select, service logo, brand mark, and
password-strength meter. A component should move here only when more than one
feature genuinely uses it.

The decorative PixelBlast effect is isolated in `components/effects/` because
it is not business logic or a normal form control.

Global variables, layout rules, responsive rules, and existing feature style
sections live in `styles/globals.css`. Small styles that belong to one component
stay beside that component. Fonts, the KeyNest mark, and bundled site logos live
in `assets/`.

Shared security-related frontend helpers remain in `shared/security/` to make
their special purpose obvious. Rust still owns the actual authentication,
encryption, secure storage, clipboard protection, and lock enforcement.

## Frontend clients and Tauri

The files named `authClient.ts`, `vaultClient.ts`, `settingsClient.ts`, and
`hostApprovalClient.ts` are the boundary between React and Rust. Components
call readable methods such as `vaultClient.listCredentials()`. Those methods
use Tauri `invoke()` with the existing Rust command names.

## How KeyNest Frontend Works

```text
User interacts with the React UI
                ↓
React calls a feature client
                ↓
The client uses Tauri invoke()
                ↓
Rust performs the secure operation
                ↓
The result returns to React
                ↓
React updates the visible UI
```

Sensitive work stays in Rust. React displays results and manages temporary UI
state; it does not replace the Rust security boundary.

## How to find something to edit

1. Start with the screen name. Vault work begins in `features/vault/VaultPage.tsx`;
   Settings work begins in `features/settings/SettingsPage.tsx`.
2. Look in that feature's `components/` directory for the visible section or
   modal.
3. Look in the feature client when the component needs data from Rust.
4. Look in `components/ui/` only for controls reused across features.
5. Look in `styles/globals.css` for existing page-level CSS class names, or next
   to the component when it has its own stylesheet.
6. Follow `main.tsx` → `App.tsx` → `AppShell.tsx` when explaining the overall
   application flow.

## Frontend checks

Run these commands after a frontend change:

```powershell
npm.cmd test
npm.cmd run build
```

`npm test` includes the React regression tests and browser-extension Autofill
tests. The production build also performs the TypeScript type check.
