# Responsive Centered Authentication Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace KeyNest's modal-like authentication card with a centered, borderless content column that responds to both window width and height without changing authentication behavior.

**Architecture:** Keep `AuthLayout` as the single shared shell for setup, unlock, and local-data-error screens, but rename its inner wrapper to express page content rather than a card. Use CSS flex auto-margins for centered-when-it-fits behavior and natural top-aligned overflow, then layer exact compact and narrow media queries over the large-window base styles.

**Tech Stack:** React 19, TypeScript 5.8, CSS media queries, Vite 7, Vitest 4, Testing Library, Tauri 2

## Global Constraints

- Apply the borderless centered layout to first-time setup, unlock, and local-data-error screens through their existing shared `AuthLayout`.
- Keep `ResetDialog` visually and semantically modal.
- The content column uses `width: min(480px, 100%)` and does not stretch wider on large windows.
- Center the mark, eyebrow, heading, description, requirement copy, warnings, errors, and button text.
- Keep field labels and entered password text left-aligned.
- Compact mode activates at `@media (max-width: 1100px), (max-height: 760px)`.
- The defensive narrow layout activates at `@media (max-width: 520px)`.
- Inputs and actionable controls retain their current comfortable heights.
- If authentication content is taller than the available page height, it begins at the page's top padding and scrolls naturally instead of clipping.
- Do not change password validation, encryption, stored data, authentication state, cooldowns, focus behavior, reset behavior, or backend commands.
- Do not reset or delete local KeyNest data for visual verification.
- Do not reorganize the test suite; the postponed test-folder work remains out of scope.
- Preserve the existing uncommitted `src-tauri/tauri.conf.json` modification and never include it in these task commits.
- Do not add tests that assert raw CSS source text.

---

## File Structure

- Create `src/features/auth/components/AuthLayout.test.tsx`: verify the shared wrapper contract and ensure the normal authentication page is not a dialog.
- Modify `src/features/auth/components/AuthLayout.tsx`: rename the shared inner wrapper from `auth-card` to `auth-content` while preserving its semantics and props.
- Modify `src/App.css`: provide the borderless centered layout, alignment rules, compact width-or-height behavior, narrow defensive behavior, and natural overflow.
- Do not modify `SetupScreen.tsx`, `UnlockScreen.tsx`, `DataErrorScreen.tsx`, or `ResetDialog.tsx`; they already consume the shared layout or provide the intentionally modal reset flow.

### Task 1: Establish the Shared Page-Content Wrapper

**Files:**
- Create: `src/features/auth/components/AuthLayout.test.tsx`
- Modify: `src/features/auth/components/AuthLayout.tsx`

**Interfaces:**
- Consumes: `AuthLayoutProps` with `eyebrow`, `title`, `description`, and `children`.
- Produces: the existing labelled `<section>` with class name `auth-content`; all three authentication screens continue to consume the same default `AuthLayout` export.

- [ ] **Step 1: Write the failing wrapper-contract test**

Create `src/features/auth/components/AuthLayout.test.tsx` with:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import AuthLayout from "./AuthLayout";

describe("AuthLayout", () => {
  it("renders authentication as labelled page content instead of a dialog card", () => {
    render(
      <AuthLayout
        eyebrow="FIRST-TIME SETUP"
        title="Create your master password"
        description="Protect the encrypted vault on this device."
      >
        <div>Authentication controls</div>
      </AuthLayout>,
    );

    const content = screen
      .getByRole("heading", { name: "Create your master password" })
      .closest("section");

    expect(content).toHaveClass("auth-content");
    expect(content).not.toHaveClass("auth-card");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByText("Authentication controls")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the new test and confirm the intended failure**

Run:

```powershell
npm.cmd test -- src/features/auth/components/AuthLayout.test.tsx
```

Expected: one failing test because the section has `auth-card` instead of `auth-content`. The failure must not come from a render error or a missing heading.

- [ ] **Step 3: Rename the shared wrapper**

In `src/features/auth/components/AuthLayout.tsx`, preserve the existing element, `aria-labelledby`, content order, and props. Change only the class name:

```tsx
<section className="auth-content" aria-labelledby="auth-title">
  <div className="auth-mark" aria-hidden="true">
    K
  </div>
  <p className="auth-eyebrow">{eyebrow}</p>
  <h1 id="auth-title">{title}</h1>
  <p className="auth-description">{description}</p>
  {children}
</section>
```

- [ ] **Step 4: Run the focused auth tests**

Run:

```powershell
npm.cmd test -- src/features/auth/components/AuthLayout.test.tsx src/features/auth/components/SetupScreen.test.tsx src/features/auth/components/UnlockScreen.test.tsx
```

Expected: all seven tests across the three files pass. Existing setup validation, unlock/refocus behavior, and reset-confirmation behavior remain green.

- [ ] **Step 5: Commit the shared wrapper contract**

Run:

```powershell
git add src/features/auth/components/AuthLayout.tsx src/features/auth/components/AuthLayout.test.tsx
git commit -m "refactor: define auth page content wrapper"
```

Confirm `src-tauri/tauri.conf.json` was not staged or committed.

### Task 2: Implement the Borderless Responsive Presentation

**Files:**
- Modify: `src/App.css`
- Test: `src/features/auth/components/AuthLayout.test.tsx`
- Test: `src/features/auth/components/SetupScreen.test.tsx`
- Test: `src/features/auth/components/UnlockScreen.test.tsx`

**Interfaces:**
- Consumes: `.auth-shell`, `.auth-page`, `.auth-content`, `.auth-mark`, `.auth-form`, `.auth-field`, `.auth-warning`, `.auth-error`, `.data-error-actions`, and the existing reset-dialog selectors.
- Produces: a 480-pixel-capped content column that centers while it fits, naturally top-aligns and scrolls when it does not, and applies compact rules when width is at most 1100 pixels or height is at most 760 pixels.

- [ ] **Step 1: Replace the card layout with centered page content**

In `src/App.css`, replace the current `.auth-page` and `.auth-card` rules with:

```css
.auth-page {
  min-height: calc(100vh - 40px);
  display: flex;
  flex-direction: column;
  padding: 48px 24px;
}

.auth-content {
  width: min(480px, 100%);
  margin: auto;
  text-align: center;
}
```

The `margin: auto` rule is required on both block axes: when extra height exists it centers the group, and when content is taller than the page the auto margins resolve to zero so normal document scrolling remains available.

- [ ] **Step 2: Center display content while preserving form scanability**

Update the existing authentication rules to use `.auth-content` and these exact alignment declarations:

```css
.auth-mark {
  width: 58px;
  height: 58px;
  display: grid;
  place-items: center;
  margin: 0 auto 28px;
  border-radius: 17px;
  background: #54f5ae;
  color: #07100c;
  font-size: 1.55rem;
  font-weight: 900;
  box-shadow: 0 0 34px rgba(84, 245, 174, 0.2);
}

.auth-content h1,
.auth-content h2,
.auth-content p {
  margin-top: 0;
}

.auth-content h1 {
  margin-bottom: 13px;
  font-size: clamp(1.9rem, 5vw, 2.45rem);
  line-height: 1.08;
  letter-spacing: -0.035em;
}

.auth-field {
  display: grid;
  gap: 8px;
  text-align: left;
}

.auth-field input {
  text-align: left;
}

.data-error-actions .primary-button {
  width: 100%;
}
```

Keep `.auth-warning`, `.auth-error`, `.auth-requirement`, `.auth-reset-link`, and button text centered through inheritance from `.auth-content`. Do not apply `text-align: center` to `.auth-field`, its labels, or its input values.

- [ ] **Step 3: Preserve keyboard focus styling under the new wrapper name**

Replace the obsolete focus selector with:

```css
.auth-content button:focus-visible,
.app-titlebar button:focus-visible,
.navigation-sidebar button:focus-visible {
  outline: 2px solid #78ffc5;
  outline-offset: 2px;
}
```

Do not change any `ResetDialog` selector, backdrop, role, dimensions, or actions.

- [ ] **Step 4: Add the width-or-height compact rules**

Immediately before the existing `@media (max-width: 850px)` rule, add:

```css
@media (max-width: 1100px), (max-height: 760px) {
  .auth-page {
    padding: 28px 24px;
  }

  .auth-mark {
    width: 50px;
    height: 50px;
    margin-bottom: 20px;
    border-radius: 15px;
    font-size: 1.35rem;
  }

  .auth-content h1 {
    margin-bottom: 10px;
    font-size: 2rem;
  }

  .auth-description {
    margin-bottom: 22px;
    font-size: 0.95rem;
    line-height: 1.5;
  }

  .auth-form {
    gap: 14px;
  }

  .auth-warning {
    padding: 12px;
  }
}
```

Do not reduce the existing 50-pixel input height, 50-pixel primary-button height, 44-pixel reset-link height, or 40-pixel password-reveal target.

- [ ] **Step 5: Replace obsolete narrow card rules**

Inside the existing `@media (max-width: 520px)` block, keep the reset-dialog rules and replace the `.auth-card` rule with these authentication overrides:

```css
.auth-page {
  padding: 22px 14px;
}

.auth-mark {
  width: 46px;
  height: 46px;
  margin-bottom: 16px;
  border-radius: 14px;
  font-size: 1.25rem;
}

.auth-content h1 {
  font-size: 1.8rem;
}

.auth-description {
  margin-bottom: 18px;
}
```

After editing, `rg -n "auth-card" src` must return no matches.

- [ ] **Step 6: Run focused behavior checks and the production build**

Run:

```powershell
npm.cmd test -- src/features/auth/components/AuthLayout.test.tsx src/features/auth/components/SetupScreen.test.tsx src/features/auth/components/UnlockScreen.test.tsx
npm.cmd run build
```

Expected: all seven focused tests pass, then TypeScript and Vite complete successfully. The build must not report a stale `.auth-card` reference.

- [ ] **Step 7: Inspect the responsive layout in the running application**

Launch the existing application without clearing or resetting its local data:

```powershell
npm.cmd run tauri -- dev
```

Inspect the authentication screen available for the current local state at 1000 by 700 pixels and at a larger size above 1100 by 760 pixels. If a browser-hosted preview is available, also inspect at 520 by 700 pixels and 1000 by 600 pixels.

At 1000 by 700, confirm:

- The complete authentication group is visible or can be reached by normal page scrolling.
- The card border, card fill, rounded outer container, and card shadow are absent.
- The content group is centered and does not exceed 480 pixels.
- The heading, description, warning, validation/error copy, and button text are centered.
- Field labels and typed password text remain left-aligned.
- Inputs, reveal controls, and primary buttons retain their existing usable heights.

At a larger size, confirm the capped column remains centered and the spacious base sizing returns. At a short or narrow preview, confirm spacing tightens and vertical overflow scrolls instead of clipping. Because all three normal auth states share `AuthLayout`, inspect whichever state exists without deleting encrypted user data; rely on the focused component tests for the other state-specific behavior.

- [ ] **Step 8: Commit the responsive styles**

Run:

```powershell
git diff --check
git add src/App.css
git commit -m "style: center responsive authentication screens"
```

Confirm the commit contains only `src/App.css`; `src-tauri/tauri.conf.json` remains outside the commit.

### Task 3: Verify the Integrated Result

**Files:**
- Verify only: `src/features/auth/components/AuthLayout.tsx`
- Verify only: `src/features/auth/components/AuthLayout.test.tsx`
- Verify only: `src/App.css`
- Preserve: `src-tauri/tauri.conf.json`

**Interfaces:**
- Consumes: the final shared wrapper and responsive stylesheet from Tasks 1 and 2.
- Produces: evidence that the frontend and desktop build remain healthy and that only the pre-existing native configuration edit is left uncommitted.

- [ ] **Step 1: Run the complete frontend test suite**

Run:

```powershell
npm.cmd test
```

Expected: all six frontend test files and 14 tests pass, including the new `AuthLayout` contract test.

- [ ] **Step 2: Build the production frontend**

Run:

```powershell
npm.cmd run build
```

Expected: TypeScript compilation and the Vite production build complete successfully.

- [ ] **Step 3: Validate the desktop application without bundling installers**

Run:

```powershell
npm.cmd run tauri -- build --debug --no-bundle
```

Expected: Tauri accepts the frontend output and current window configuration and produces the debug desktop executable successfully.

- [ ] **Step 4: Confirm the final repository state**

Run:

```powershell
git status --short
git log --oneline --decorate -5
```

Expected: the two implementation commits are present. No auth implementation file is uncommitted; the pre-existing `src-tauri/tauri.conf.json` modification remains visible and untouched.
