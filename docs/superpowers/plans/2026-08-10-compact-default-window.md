# Compact Default Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make KeyNest open at 1000 by 700 pixels, prevent resizing below that size, and use a compact presentation near the default viewport while preserving the spacious layout in larger windows.

**Architecture:** Keep native window limits in the existing Tauri JSON configuration and add one width-or-height media query to the existing stylesheet. Because these changes are configuration and visual styling, verify them through configuration parsing, the Tauri build, existing automated tests, production builds, and viewport inspection rather than brittle source-text tests.

**Tech Stack:** Tauri 2 configuration, React 19, CSS media queries, TypeScript 5.8, Vite 7, Vitest 4

## Global Constraints

- Default window size is exactly 1000 by 700 pixels.
- Minimum window size is exactly 1000 by 700 pixels.
- The window remains resizable to larger dimensions, centered, and decoration-free.
- Compact mode uses `@media (max-width: 1100px), (max-height: 760px)` so either constrained dimension activates it.
- Compact mode changes presentation only; it does not alter React structure, authentication, storage, encryption, or backend commands.
- Existing input and button dimensions remain unchanged.
- Existing 850-pixel and 520-pixel narrow-screen breakpoints remain in place.
- Do not add tests that assert raw JSON or CSS source text.

---

## File Structure

- Modify `src-tauri/tauri.conf.json`: set the supported native minimum dimensions.
- Modify `src/App.css`: add the compact desktop media query before the existing narrow-screen rules.
- No test file is created for raw configuration or stylesheet text; the existing suite provides regression coverage for application behavior.

### Task 1: Lock the Native Window to the Supported Minimum

**Files:**
- Modify: `src-tauri/tauri.conf.json`

**Interfaces:**
- Consumes: the first entry in Tauri's `app.windows` configuration.
- Produces: a main window with 1000-by-700 default and minimum dimensions, resizing enabled above the minimum, centering enabled, and decorations disabled.

- [ ] **Step 1: Update the native window configuration**

In `src-tauri/tauri.conf.json`, preserve all unrelated configuration and set the main window block to these values:

```json
"width": 1000,
"height": 700,
"minWidth": 1000,
"minHeight": 700,
"resizable": true,
"center": true,
"decorations": false
```

- [ ] **Step 2: Parse and inspect the effective JSON values**

Run:

```powershell
$config = Get-Content 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json
$window = $config.app.windows[0]
$window | Select-Object width, height, minWidth, minHeight, resizable, center, decorations
```

Expected values:

```text
width       1000
height       700
minWidth    1000
minHeight    700
resizable   True
center      True
decorations False
```

- [ ] **Step 3: Validate the configuration through a no-bundle Tauri build**

Run:

```powershell
npm.cmd run tauri -- build --debug --no-bundle
```

Expected: the Tauri CLI accepts the configuration and produces a debug desktop executable without bundling installers.

- [ ] **Step 4: Commit the native window constraint**

Run:

```powershell
git add src-tauri/tauri.conf.json
git commit -m "feat: enforce default KeyNest window size"
```

### Task 2: Add the Compact Desktop Presentation

**Files:**
- Modify: `src/App.css`

**Interfaces:**
- Consumes: existing home and authentication selectors in `src/App.css`.
- Produces: a compact media-query block before `@media (max-width: 850px)` while leaving interactive-control sizes and larger-window base rules unchanged.

- [ ] **Step 1: Add the compact media query**

Insert this block in `src/App.css` immediately before the existing `@media (max-width: 850px)` rule:

```css
@media (max-width: 1100px), (max-height: 760px) {
  .topbar {
    min-height: 76px;
  }

  .hero {
    padding: 60px 0 58px;
  }

  .hero h1 {
    font-size: clamp(2.8rem, 5vw, 3.1rem);
  }

  .hero-description {
    margin: 20px 0 26px;
    font-size: 0.96rem;
    line-height: 1.6;
  }

  .features {
    padding-bottom: 64px;
  }

  .section-heading {
    margin-bottom: 22px;
  }

  .section-heading h2 {
    font-size: 1.9rem;
  }

  .feature-card {
    min-height: 270px;
    padding: 22px;
  }

  .feature-icon {
    width: 46px;
    height: 46px;
    margin-bottom: 22px;
    font-size: 1.3rem;
  }

  .feature-card h3 {
    font-size: 1.16rem;
  }

  .feature-card p {
    margin: 12px 0 20px;
    font-size: 0.93rem;
    line-height: 1.55;
  }

  .auth-page {
    padding: 28px 24px;
  }

  .auth-card {
    padding: 34px;
  }

  .auth-mark {
    width: 50px;
    height: 50px;
    margin-bottom: 20px;
    font-size: 1.35rem;
  }

  .auth-card h1 {
    font-size: 2rem;
  }

  .auth-description {
    margin-bottom: 22px;
    font-size: 0.95rem;
    line-height: 1.55;
  }

  .auth-form {
    gap: 15px;
  }
}
```

- [ ] **Step 2: Confirm the production stylesheet keeps its responsive order**

Inspect the end of `src/App.css` and confirm the media queries occur in this order:

```text
@media (max-width: 1100px), (max-height: 760px)
@media (max-width: 850px)
@media (max-width: 520px)
```

Expected: the compact desktop block comes first, allowing the narrower existing rules to override it when applicable.

- [ ] **Step 3: Verify application tests and the production frontend build**

Run:

```powershell
npm.cmd test
npm.cmd run build
```

Expected: all existing frontend tests pass with pristine output, then TypeScript and Vite complete successfully.

- [ ] **Step 4: Commit the compact presentation**

Run:

```powershell
git add src/App.css
git commit -m "style: compact KeyNest default window"
```

### Task 3: Verify the Integrated Desktop Experience

**Files:**
- Verify only: `src-tauri/tauri.conf.json`
- Verify only: `src/App.css`
- Verify only: `src/tests/`

**Interfaces:**
- Consumes: the centralized test suite, native window constraint, and compact stylesheet.
- Produces: verified frontend, Rust/Tauri, configuration, and viewport behavior with no additional feature surface.

- [ ] **Step 1: Run the complete frontend test suite**

Run:

```powershell
npm.cmd test
```

Expected: all five frontend test files and 13 tests pass.

- [ ] **Step 2: Build the frontend production bundle**

Run:

```powershell
npm.cmd run build
```

Expected: TypeScript and Vite complete successfully.

- [ ] **Step 3: Run Rust formatting, tests, and lint checks**

Run:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: formatting is clean, all Rust tests pass, and Clippy reports no warnings.

- [ ] **Step 4: Revalidate the complete Tauri application**

Run:

```powershell
npm.cmd run tauri -- build --debug --no-bundle
```

Expected: the debug desktop executable builds successfully with the final frontend and configuration.

- [ ] **Step 5: Inspect compact and spacious browser layouts**

Check whether the existing development server responds:

```powershell
curl.exe -I --max-time 5 http://127.0.0.1:1420
```

If it does not respond, start it with:

```powershell
npm.cmd run dev -- --host 127.0.0.1
```

Use the in-app browser tooling, if operational, to inspect the home and authentication screens at 1000 by 700 pixels and again above 1100 by 760 pixels.

At 1000 by 700, confirm:

- The complete hero heading is visible at about 50 pixels with no overlap.
- Home supporting copy and feature-card content remain readable.
- Authentication titles, descriptions, and long error text fit and wrap inside the card.
- Inputs and buttons retain their existing dimensions.

At the larger viewport, confirm the original base typography and spacing return. If the in-app browser remains unavailable, report the tooling limitation explicitly and rely on the stylesheet review, production builds, and the user's native-app visual check without claiming screenshot verification.

- [ ] **Step 6: Confirm repository state contains only intended commits**

Run:

```powershell
git status --short
git log --oneline --decorate -5
```

Expected: no uncommitted implementation changes remain, and the recent history contains separate commits for test organization, window constraints, and compact styling.
