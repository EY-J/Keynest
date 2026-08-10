# Centralized Frontend Tests Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move every frontend test and its shared setup into a mirrored `src/tests/` tree without changing application or test behavior.

**Architecture:** Preserve the existing Vitest/JSDOM suite and relocate files according to the production tree they test. Update only moved-file imports and Vitest's setup path; do not introduce aliases, dependencies, helpers, or new test semantics.

**Tech Stack:** React 19, TypeScript 5.8, Vitest 4, Testing Library, Vite 7

## Global Constraints

- All `.test.ts` and `.test.tsx` files must live beneath `src/tests/`.
- `src/tests/` mirrors tested production paths beneath `src/`.
- Shared setup must live at `src/tests/setup.ts`.
- Test names, assertions, mocks, fixtures, and production files remain unchanged.
- Imports use ordinary relative paths; no alias, dependency, barrel file, or custom resolver is added.
- The old `src/test/` directory must not remain.

---

## File Structure

- Move `src/test/setup.ts` to `src/tests/setup.ts`: shared Testing Library cleanup.
- Move `src/app/App.test.tsx` to `src/tests/app/App.test.tsx`: application integration tests.
- Move `src/features/auth/authClient.test.ts` to `src/tests/features/auth/authClient.test.ts`: Tauri authentication client tests.
- Move the three component tests from `src/features/auth/components/` to `src/tests/features/auth/components/`: authentication UI tests.
- Modify `vite.config.ts`: point `setupFiles` at `./src/tests/setup.ts`.

### Task 1: Move the Frontend Test Suite

**Files:**
- Move: `src/test/setup.ts` -> `src/tests/setup.ts`
- Move: `src/app/App.test.tsx` -> `src/tests/app/App.test.tsx`
- Move: `src/features/auth/authClient.test.ts` -> `src/tests/features/auth/authClient.test.ts`
- Move: `src/features/auth/components/AuthGate.test.tsx` -> `src/tests/features/auth/components/AuthGate.test.tsx`
- Move: `src/features/auth/components/SetupScreen.test.tsx` -> `src/tests/features/auth/components/SetupScreen.test.tsx`
- Move: `src/features/auth/components/UnlockScreen.test.tsx` -> `src/tests/features/auth/components/UnlockScreen.test.tsx`
- Modify: `vite.config.ts`

**Interfaces:**
- Consumes: Vitest's default `*.test.ts`/`*.test.tsx` discovery and the existing `src/test/setup.ts` cleanup behavior.
- Produces: the same five test files and 13 test cases under `src/tests/`, with Vitest loading `src/tests/setup.ts`.

- [ ] **Step 1: Establish the passing test baseline**

Run:

```powershell
npm.cmd test
```

Expected: PASS with five test files and 13 passing tests. Stop and report if the baseline does not pass.

- [ ] **Step 2: Create the mirrored test directories**

Run:

```powershell
New-Item -ItemType Directory -Force 'src/tests/app'
New-Item -ItemType Directory -Force 'src/tests/features/auth/components'
```

Expected: both directory paths exist.

- [ ] **Step 3: Move setup and all five tests with Git-aware moves**

Run:

```powershell
git mv src/test/setup.ts src/tests/setup.ts
git mv src/app/App.test.tsx src/tests/app/App.test.tsx
git mv src/features/auth/authClient.test.ts src/tests/features/auth/authClient.test.ts
git mv src/features/auth/components/AuthGate.test.tsx src/tests/features/auth/components/AuthGate.test.tsx
git mv src/features/auth/components/SetupScreen.test.tsx src/tests/features/auth/components/SetupScreen.test.tsx
git mv src/features/auth/components/UnlockScreen.test.tsx src/tests/features/auth/components/UnlockScreen.test.tsx
```

Expected: Git records six renames and no test files remain at the old locations.

- [ ] **Step 4: Update production-module imports in the moved tests**

Use these exact imports while leaving all test bodies unchanged.

In `src/tests/app/App.test.tsx`:

```ts
import { authClient } from "../../features/auth/authClient";
import App from "../../app/App";
```

In `src/tests/features/auth/authClient.test.ts`:

```ts
import { authClient } from "../../../features/auth/authClient";
```

In `src/tests/features/auth/components/AuthGate.test.tsx`:

```ts
import { authClient } from "../../../../features/auth/authClient";
import type { AuthStatus } from "../../../../features/auth/types";
import AuthGate from "../../../../features/auth/components/AuthGate";
```

In `src/tests/features/auth/components/SetupScreen.test.tsx`:

```ts
import { authClient } from "../../../../features/auth/authClient";
import SetupScreen from "../../../../features/auth/components/SetupScreen";
```

In `src/tests/features/auth/components/UnlockScreen.test.tsx`:

```ts
import { authClient } from "../../../../features/auth/authClient";
import { AuthClientError } from "../../../../features/auth/types";
import ResetDialog from "../../../../features/auth/components/ResetDialog";
import UnlockScreen from "../../../../features/auth/components/UnlockScreen";
```

- [ ] **Step 5: Update Vitest's setup path**

In `vite.config.ts`, replace the existing `setupFiles` value with:

```ts
setupFiles: ["./src/tests/setup.ts"],
```

- [ ] **Step 6: Verify unchanged test behavior and TypeScript resolution**

Run:

```powershell
npm.cmd test
npm.cmd run build
```

Expected: the same five files and 13 tests pass, then TypeScript and Vite complete successfully.

- [ ] **Step 7: Verify the directory contract**

Run:

```powershell
$testFiles = Get-ChildItem src -Recurse -File | Where-Object { $_.Name -match '\.test\.(ts|tsx)$' }
$outsideTests = $testFiles | Where-Object { $_.FullName -notlike '*\src\tests\*' }
$testFiles.FullName
"OutsideTests=$($outsideTests.Count)"
"OldSetupDirectory=$(Test-Path 'src/test')"
```

Expected: five paths are listed beneath `src\tests\`, `OutsideTests=0`, and `OldSetupDirectory=False`.

- [ ] **Step 8: Commit the structural refactor**

Run:

```powershell
git add -A src vite.config.ts
git commit -m "test: centralize frontend test suite"
```

Expected: one commit containing only the six moves, import adjustments, and setup-path update.
