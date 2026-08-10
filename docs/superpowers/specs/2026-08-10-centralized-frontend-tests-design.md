# Centralized Frontend Tests Design

## Goal

Keep KeyNest production directories easy to scan by moving every frontend test and its shared setup into one mirrored `src/tests/` tree without changing test behavior or application behavior.

## Directory Structure

The frontend test suite will use this structure:

```text
src/tests/
|-- setup.ts
|-- app/
|   `-- App.test.tsx
`-- features/
    `-- auth/
        |-- authClient.test.ts
        `-- components/
            |-- AuthGate.test.tsx
            |-- SetupScreen.test.tsx
            `-- UnlockScreen.test.tsx
```

The directory beneath `src/tests/` mirrors each tested production file's location beneath `src/`. New frontend tests should follow the same convention.

## File Moves and Imports

All five existing frontend test files will move; none will be duplicated or left beside production code. `src/test/setup.ts` will move to `src/tests/setup.ts`, and the Vitest `setupFiles` entry in `vite.config.ts` will change to `./src/tests/setup.ts`.

Imports inside the moved tests will use ordinary relative paths to their production modules. This cleanup will not introduce a source alias, dependency, barrel file, or custom test resolver.

Test names, assertions, mocks, fixtures, and production files will remain unchanged. Git-aware moves should preserve file history where practical.

## Behavior and Error Handling

This is a structural refactor only. Vitest will continue discovering files through its default `*.test.ts` and `*.test.tsx` patterns, and the existing JSDOM environment and cleanup behavior will remain in effect.

Incorrect paths will surface as import or setup-resolution failures when the suite runs. The implementation will fix path errors directly rather than add fallback behavior.

## Relationship to the Compact Window Work

The test reorganization will be completed and verified as its own commit before the compact-window configuration and styling changes. The compact-window work will not add source-text tests for JSON or CSS. Those presentation changes will instead be verified through the existing automated suite, production builds, Tauri configuration validation, and visual inspection when browser tooling is operational.

## Verification

- Run the complete frontend suite before moving files and record the passing baseline.
- Run the complete frontend suite after the move and confirm the same tests pass.
- Run the TypeScript/Vite production build to confirm all moved imports and the updated setup path resolve.
- Confirm no `.test.ts` or `.test.tsx` files remain outside `src/tests/`.
- Confirm the old `src/test/` directory no longer remains.

## Success Criteria

All frontend tests and shared setup live under the mirrored `src/tests/` tree, all existing tests still pass, the production build succeeds, and no production behavior or test semantics change.
